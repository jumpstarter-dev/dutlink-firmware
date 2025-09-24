#![no_std]
#![no_main]

// use panic_halt as _;
use panic_semihosting as _;

mod control;
mod dfu;
mod storage;
mod usbserial;
mod ctlpins;
mod powermeter;
mod filter;
mod version;
mod config;

// dispatchers are free Hardware IRQs we don't use that rtic will use to dispatch
// software tasks, we are not using EXT interrupts, so we can use those
#[rtic::app(device = stm32f4xx_hal::pac, peripherals = true, dispatchers = [EXTI0, EXTI1, EXTI2])]
mod app {

    use stm32f4xx_hal::{
        gpio,
        gpio::{Input, Output, PushPull},
        otg_fs::{UsbBus, UsbBusType, USB},
        pac,
        prelude::*,
        timer,
        serial::{config::Config, Tx, Rx, Serial},
        adc::{config::{AdcConfig, Dma, SampleTime, Scan, Sequence, Resolution}, Adc},
        dma::{config::DmaConfig, PeripheralToMemory, Stream0, StreamsTuple, Transfer},
        pac::{ADC1, DMA2},
    };

    use heapless::spsc::{Consumer, Producer, Queue};
    use usb_device::{class_prelude::*, prelude::*};

    use usbd_serial::SerialPort;

    use crate::{control::ControlClass, dfu::{get_serial_str, new_dfu_bootloader, DFUBootloaderRuntime}};
    use crate::storage::*;
    use crate::usbserial::*;
    use crate::ctlpins;
    use crate::powermeter::*;
    use crate::version;
    use crate::config::*;

    type LedCmdType = gpio::PC15<Output<PushPull>>;
    type StorageSwitchType = StorageSwitch<gpio::PA15<Output<PushPull>>, gpio::PB3<Output<PushPull>>,
                                           gpio::PB5<Output<PushPull>>, gpio::PB4<Output<PushPull>>>;
    type CTLPinsType = ctlpins::CTLPins<gpio::PA4<Output<PushPull>>>;
    type DMATransfer = Transfer<Stream0<DMA2>, 0, Adc<ADC1>, PeripheralToMemory, &'static mut [u16; 2]>;

    const DUT_BUF_SIZE: usize = 1024;
    // Resources shared between tasks
    #[shared]
    struct Shared {
        timer: timer::CounterMs<pac::TIM2>,
        usb_dev: UsbDevice<'static, UsbBusType>,
        usb_serial: USBSerialType,
        dfu: DFUBootloaderRuntime,
        ctl: ControlClass,

        led_tx: gpio::PC13<Output<PushPull>>,
        led_rx: gpio::PC14<Output<PushPull>>,
        led_cmd: LedCmdType,

        storage: StorageSwitchType,

        adc_dma_transfer: DMATransfer,

        ctl_pins: CTLPinsType,

        power_meter: MAVPowerMeter,

        config: ConfigArea,
    }

    // Local resources to specific tasks (cannot be shared)
    #[local]
    struct Local {
        _button: gpio::PA0<Input>,
        usart_rx: Rx<pac::USART1>,
        usart_tx: Tx<pac::USART1>,
        to_dut_serial: Producer<'static, u8, DUT_BUF_SIZE>,          // queue of characters to send to the DUT
        to_dut_serial_consumer: Consumer<'static, u8, DUT_BUF_SIZE>, // consumer side of the queue
        to_host_serial: Producer<'static, u8, DUT_BUF_SIZE>,          // queue of characters to send to the DUT
        to_host_serial_consumer: Consumer<'static, u8, DUT_BUF_SIZE>, // consumer side of the queue
        adc_buffer: Option<&'static mut [u16; 2]>,
    }

    #[init(local = [q_to_dut: Queue<u8, DUT_BUF_SIZE> = Queue::new(), q_from_dut: Queue<u8, DUT_BUF_SIZE> = Queue::new()])]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        static mut USB_BUS: Option<UsbBusAllocator<UsbBusType>,> = None;
        static mut EP_MEMORY: [u32; 1024] = [0; 1024];

        let dp = ctx.device;
        let rcc = dp.RCC.constrain();
        let clocks = rcc
            .cfgr
            .use_hse(25.MHz())
            .sysclk(48.MHz())
            .require_pll48clk()
            .freeze();

        // Configure the on-board LED (PC13, blue)
        let gpioa = dp.GPIOA.split();
        let gpiob = dp.GPIOB.split();
        let gpioc = dp.GPIOC.split();

        let mut led_tx = gpioc.pc13.into_push_pull_output();
        let mut led_rx = gpioc.pc14.into_push_pull_output();
        let mut led_cmd = gpioc.pc15.into_push_pull_output();

        led_tx.set_high();
        led_rx.set_high();
        led_cmd.set_high();

        let _button = gpioa.pa0.into_pull_up_input();

        let ctl_pins = ctlpins::CTLPins::new(gpioa.pa5.into_dynamic(),          // ctl_a
                                             gpioa.pa6.into_dynamic(),          // ctl_b
                                             gpioa.pa7.into_dynamic(),          // ctl_c
                                             gpioa.pa8.into_dynamic(),          // ctl_d
                                             gpioa.pa9.into_dynamic(),          // reset
                                             gpioa.pa4.into_push_pull_output()  // power enable
                                            );

        let pins = (gpiob.pb6, gpiob.pb7);
        let usart = Serial::new(
            dp.USART1,
            pins, // (tx, rx)
            Config::default().baudrate(115_200.bps()).wordlength_8(),
            &clocks,
        ).unwrap().with_u8_data();

        let (usart_tx, mut usart_rx) = usart.split();

        usart_rx.listen();


        let current_sense = gpioa.pa1.into_analog();
        let vout_sense = gpioa.pa2.into_analog();
        let dma = StreamsTuple::new(dp.DMA2);
        let config = DmaConfig::default()
                    .transfer_complete_interrupt(true)
                    .memory_increment(true)
                    .double_buffer(false);

        let adc_config = AdcConfig::default()
                        .dma(Dma::Continuous)
                        .scan(Scan::Enabled)
                        .resolution(Resolution::Twelve);

        let mut adc = Adc::adc1(dp.ADC1, true, adc_config);

        adc.configure_channel(&current_sense, Sequence::One, SampleTime::Cycles_480);
        adc.configure_channel(&vout_sense, Sequence::Two, SampleTime::Cycles_480);
        adc.enable_temperature_and_vref();
        let power_meter = MAVPowerMeter::new();

        let first_buffer = cortex_m::singleton!(: [u16; 2] = [0; 2]).unwrap();
        let adc_buffer = Some(cortex_m::singleton!(: [u16; 2] = [0; 2]).unwrap());
        // Give the first buffer to the DMA. The second buffer is held in an Option in `local.buffer` until the transfer is complete
        let adc_dma_transfer = Transfer::init_peripheral_to_memory(dma.0, adc, first_buffer, None, config);

        let mut storage = StorageSwitch::new(
            gpioa.pa15.into_push_pull_output(), //OEn
            gpiob.pb3.into_push_pull_output(), //SEL
            gpiob.pb5.into_push_pull_output(), //PW_DUT
            gpiob.pb4.into_push_pull_output(), //PW_HOST
        );

        storage.power_off();


        // setup a timer for the periodic 10ms task
        let mut timer = dp.TIM2.counter_ms(&clocks);
        timer.start(10.millis()).unwrap(); //100Hz
        timer.listen(timer::Event::Update);

        // Pull the D+ pin down to send a RESET condition to the USB bus.
        let mut usb_dp = gpioa.pa12.into_push_pull_output();
        usb_dp.set_low();
        cortex_m::asm::delay(1024 * 50);

        let usb_periph = USB::new(
            (dp.OTG_FS_GLOBAL, dp.OTG_FS_DEVICE, dp.OTG_FS_PWRCLK),
            (gpioa.pa11.into_alternate(), usb_dp.into_alternate()),
            &clocks,
        );

        unsafe {
            USB_BUS = Some(UsbBus::new(usb_periph, &mut EP_MEMORY));
        }
        /* I tried creating a 2nd serial port which only works on STM32F412 , 411 has not enough
           endpoints, but it didn't work well, the library probably needs some debugging */
        let mut usb_serial = new_usb_serial! (unsafe { USB_BUS.as_ref().unwrap() });
        let dfu = new_dfu_bootloader(unsafe { USB_BUS.as_ref().unwrap() });
        let ctl = ControlClass::new(unsafe { USB_BUS.as_ref().unwrap() });

        usb_serial.reset();

        let usb_dev = UsbDeviceBuilder::new(
            unsafe { USB_BUS.as_ref().unwrap() },
            UsbVidPid(0x2b23, 0x1012),
        )
        .strings(&[
            StringDescriptors::default()
            .manufacturer("Red Hat Inc.")
            .product("Dutlink")
            .serial_number(get_serial_str())
        ]).unwrap()
        .device_release(version::usb_version_bcd_device())
        .self_powered(false)
        .max_power(250).unwrap()
        .max_packet_size_0(64).unwrap()
        .build();



        let (to_dut_serial, to_dut_serial_consumer) = ctx.local.q_to_dut.split();
        let (to_host_serial, to_host_serial_consumer) = ctx.local.q_from_dut.split();

        let config = ConfigArea::new(stm32f4xx_hal::flash::LockedFlash::new(dp.FLASH));

        (
            Shared {
                timer,
                usb_dev,
                usb_serial,
                dfu,
                ctl,
                led_tx,
                led_rx,
                led_cmd,
                storage,
                adc_dma_transfer,
                ctl_pins,
                power_meter,
                config,
            },
            Local {
                _button,
                usart_tx,
                usart_rx,
                to_dut_serial,
                to_dut_serial_consumer,
                to_host_serial,
                to_host_serial_consumer,
                adc_buffer,
            },
            // Move the monotonic timer to the RTIC run-time, this enables
            // scheduling
            init::Monotonics(),
        )
    }

    #[task(binds = USART1, priority=1, local = [usart_rx, to_host_serial], shared = [led_rx])]
    fn usart_task(cx: usart_task::Context){
        let usart_rx = cx.local.usart_rx;
        let mut led_rx = cx.shared.led_rx;
        let to_host_serial = cx.local.to_host_serial;

        led_rx.lock(|led_rx| {
            while usart_rx.is_rx_not_empty() {
                led_rx.set_low();
                match usart_rx.read() {
                    Ok(b) => {
                        to_host_serial.enqueue(b).ok(); // this could over-run but it's ok the only solution would be a bigger buffer
                    },
                    Err(_e) => {
                        break;
                    }
                }
            }
            usart_rx.clear_idle_interrupt();
        });


        if to_host_serial.len() > 0 {
           console_monitor_task::spawn().ok();
        }
    }

    #[task(local=[to_host_serial_consumer], shared=[usb_serial])]
    fn console_monitor_task(mut cx: console_monitor_task::Context) {
        let to_host_serial_consumer = cx.local.to_host_serial_consumer;
        let usb_serial = &mut cx.shared.usb_serial;

        // Forward data from physical serial port to USB serial port
        if to_host_serial_consumer.len() > 0 {
            usb_serial.lock(|usb_serial| {
                let mut buf = [0u8; DUT_BUF_SIZE];
                let mut count = 0;
                loop {
                    match to_host_serial_consumer.dequeue() {
                        Some(c) => {
                            buf[count] = c;
                            count += 1;
                            if count >= buf.len() {
                                break;
                            }
                        },
                        None => {
                            break;
                        }
                    }
                }
                if count > 0 {
                    usb_serial.write(&buf[..count]).ok();
                }
            });
        }
    }

    #[task(binds = OTG_FS, shared = [usb_dev, usb_serial, dfu, ctl, led_cmd, storage, ctl_pins, power_meter, config], local=[to_dut_serial])]
    fn usb_task(mut cx: usb_task::Context) {
        let usb_dev         = &mut cx.shared.usb_dev;
        let usb_serial      = &mut cx.shared.usb_serial;
        let dfu             = &mut cx.shared.dfu;
        let ctl             = &mut cx.shared.ctl;
        let led_cmd         = &mut cx.shared.led_cmd;
        let storage         = &mut cx.shared.storage;
        let to_dut_serial   = cx.local.to_dut_serial;

        let ctl_pins        = &mut cx.shared.ctl_pins;
        let power_meter     = &mut cx.shared.power_meter;
        let config          = &mut cx.shared.config;

        (usb_dev, usb_serial, dfu, ctl, led_cmd, storage, ctl_pins, power_meter, config).lock(
            |usb_dev, usb_serial, dfu, ctl, _led_cmd, storage, ctl_pins, power_meter, config| {
            if !usb_dev.poll(&mut [usb_serial, dfu, ctl]) {
                return;
            }

            ctl.post_poll(config, ctl_pins, storage, power_meter);

            // Forward USB serial data to physical serial port
            let available_to_dut = to_dut_serial.capacity() - to_dut_serial.len();
            if available_to_dut > 0 {
                let mut buf = [0u8; DUT_BUF_SIZE];
                match usb_serial.read(&mut buf[..available_to_dut]) {
                    Ok(count) => {
                        for b in &buf[..count] {
                            to_dut_serial.enqueue(*b).ok();
                        }
                    },
                    Err(_e) => {}
                }
            }
        });
    }

    #[task(binds = TIM2, shared=[timer, dfu,  led_rx, led_tx, led_cmd, adc_dma_transfer])]
    fn periodic_10ms(mut ctx: periodic_10ms::Context) {

        ctx.shared.dfu.lock(|dfu| dfu.tick(10));

        // clear all leds set in other tasts
        ctx.shared.led_rx.lock(|led_rx| led_rx.set_high());
        ctx.shared.led_tx.lock(|led_tx| led_tx.set_high());
        ctx.shared.led_cmd.lock(|led_cmd| led_cmd.set_high());

        ctx.shared.adc_dma_transfer.lock(|transfer| {
            transfer.start(|adc| {
                adc.start_conversion();
            });
        });

        ctx.shared
            .timer
            .lock(|tim| tim.clear_flags(timer::Flag::Update));
    }

    #[task(binds = DMA2_STREAM0, shared=[adc_dma_transfer, power_meter], local=[adc_buffer])]
    fn adc_dma(mut cx:adc_dma::Context){
        let adc_dma_transfer = &mut cx.shared.adc_dma_transfer;
        let adc_buffer = &mut cx.local.adc_buffer;
        let power_meter = &mut cx.shared.power_meter;


        let buffer = adc_dma_transfer.lock(|transfer| {
            let (buffer, _) = transfer
                               .next_transfer(adc_buffer.take().unwrap())
                               .unwrap();
            buffer
        });

        // get the ADC readings for the current and the output voltage
        let current_raw = buffer[0];
        let vout_raw = buffer[1];

        // leave the previous buffer ready again for next transfer
        *cx.local.adc_buffer = Some(buffer);

        // calculate current in amps
        let current_V = (current_raw as f32 - 2048.0) * 3.3 / 4096.0;
        let current_A = -current_V / 0.264;

        // calculate vin voltage in volts
        // we get vout from the voltage divider, in 12 bits, 3.3V is 4096
        let vout_sense_V = (vout_raw as f32) * 3.3 / 4096.0;
        // we do the reverse calculation to figure out the input voltage
        let R8 = 2400.0; // R8 is the top resistor in the voltage divider
        let R9 = 470.0; // R9 is the bottom resistor in the voltage divider
        let vin = vout_sense_V * (R8 + R9) / R9;

        power_meter.lock(|power_meter| {
            power_meter.feed_voltage(vin);
            power_meter.feed_current(current_A);
        });

    }


    // Background task, runs whenever no other tasks are running
    #[idle(local=[to_dut_serial_consumer, usart_tx], shared=[led_tx])]
    fn idle(mut ctx: idle::Context) -> ! {
        // Forward data from USB serial to physical serial port
        let to_dut_serial_consumer = &mut ctx.local.to_dut_serial_consumer;

        loop {
            // Go to sleep, wake up on interrupt
            cortex_m::asm::wfi();

            // Is there any data to be sent to the device under test over USART?
            if to_dut_serial_consumer.len() == 0 {
                continue;
            }
            loop {
                match to_dut_serial_consumer.dequeue() {
                    Some(c) => {
                        // Transparent forwarding - no escape character processing
                        let usart_tx = &mut ctx.local.usart_tx;
                        let led_tx = &mut ctx.shared.led_tx;
                        led_tx.lock(|led_tx| led_tx.set_low());

                        loop {
                            if usart_tx.is_tx_empty() {
                                    break;
                            }
                        }

                        usart_tx.write(c).ok();

                    },
                    None => {
                        break;
                    }
                }
            }
        }
    }



}
