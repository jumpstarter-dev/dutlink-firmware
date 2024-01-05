//! # DFU bootloader for use with fwupd and STM32F411CEU6
//!
//! It is adapted from the example https://github.com/vitalyvb/usbd-dfu-example
//! writen on top of the https://github.com/vitalyvb/usbd-dfu stack.
//!
//! The address map is updated so the bootloader space is not exposed
//! to the host, and the start address begins right after the bootloader
//! at 0x08008000, this works better for fwupd.
//!
//! There are two modes of operation: minimal and DFU.
//!
//! After reset, bootloader starts in a minimal mode,
//! its goal is to determine if bootloader must switch
//! to DFU mode, and if not, try to jump to a main
//! firmware.
//!
//! In minimal mode, following items are checked:
//!
//! > * Magic value in RAM (can be used by the firmware application to
//!     force the bootloader to enter DFU mode).
//!
//! > * KEY BUTTON (PA0) state.
//!
//! > * The first few bytes of a firmware (should look like a proper stack pointer).
//!
//! When DFU mode is active, LED on PC13 blinks every 2 seconds.
//! Required peripherals and USB are enabled, host
//! can issue DFU commands.
//!
//! The first 0x10 bytes of RAM are reserved. In "memory.x" linker script
//! RAM section has 0x10 offset from an actual RAM start. The first
//! 4 bytes of RAM may have a magic value to force the bootloader
//! to enter DFU mode programmatically. Both DFU and main firmware
//! must agree on used addresses and values for this to work.
//!

#![no_std]
#![no_main]

use core::str;

use panic_halt as _;

use cortex_m_rt::entry;

use stm32f4xx_hal::{
    pac,
    prelude::*,
};

use stm32f4xx_hal::gpio::{gpioc, Output, PushPull};
use stm32f4xx_hal::pac::{interrupt, GPIOA, RCC};
use stm32f4xx_hal::otg_fs::{UsbBus, USB, UsbBusType};
use stm32f4xx_hal::flash::{FlashExt, LockedFlash, flash_sectors};
use usb_device::{bus::UsbBusAllocator, prelude::*};
use usbd_dfu::*;

use core::mem::MaybeUninit;
const TRANSFER_SIZE: usize = 256;
static mut EP_MEMORY: [u32; 1024] = [0; 1024];

/// If this value is found at the address 0x2000_0000 (beginning of RAM),
/// bootloader will enter DFU mode.
const KEY_STAY_IN_BOOT: u32 = 0xb0d42b89;

/// Board flash configuration. MEM_INFO_STRING below must also be changed.
const BOOTLOADER_SIZE_BYTES: u32 = 64 * 1024;
const FW_ADDRESS: u32 = 0x0801_0000;

type LedType = gpioc::PC13<Output<PushPull>>;

static mut USB_BUS: MaybeUninit<UsbBusAllocator<UsbBusType>> = MaybeUninit::uninit();
static mut USB_DEVICE: MaybeUninit<UsbDevice<UsbBusType>> = MaybeUninit::uninit();
static mut USB_DFU: MaybeUninit<DFUClass<UsbBusType, STM32Mem>> = MaybeUninit::uninit();

pub struct STM32Mem{
    flash: LockedFlash,
    buffer: [u8; TRANSFER_SIZE],
}

impl STM32Mem{
    fn new(flash: LockedFlash) -> Self {
        Self {
            flash,
            buffer: [0; TRANSFER_SIZE],
        }
    }
}

impl<'a> DFUMemIO for STM32Mem {
    const INITIAL_ADDRESS_POINTER: u32 = 0x0801_0000;
    const PROGRAM_TIME_MS: u32 = 12; // time it takes to program 128 bytes
    const ERASE_TIME_MS: u32 = 200;
    const FULL_ERASE_TIME_MS: u32 = 30000;
    const MANIFESTATION_TIME_MS: u32 = 2000;
    const TRANSFER_SIZE: u16 = TRANSFER_SIZE as u16;

    // Internal bootloader says: "@Internal Flash  /0x08000000/04*016Kg,01*064Kg,03*128Kg", serial="356438913137"
    //const MEM_INFO_STRING: &'static str = "@Flash/0x08008000/02*016Kg,01*064Kg,03*128Kg";
    // The bootloader only takes 16kb so far, but we leave a few 16kB sectors at the start for config
    const MEM_INFO_STRING: &'static str = "@Flash/0x08010000/01*064Kg,03*128Kg";
    const HAS_DOWNLOAD: bool = true; // download from host into the device is enabled
    const HAS_UPLOAD: bool = false;  // upload from the device to the host is disabled, also read code is commented out
    const MANIFESTATION_TOLERANT: bool = false;

    fn read(&mut self, address: u32, length: usize) -> core::result::Result<&[u8], DFUMemError> {
       
        let flash_top: u32 = (self.flash.address() + self.flash.len()) as u32;

        if address < FW_ADDRESS {
            return Err(DFUMemError::Address);
        }
        if address >= flash_top {
            return Ok(&[]);
        }

        let len = length.min((flash_top - address) as usize);

        let mem = unsafe { &*core::ptr::slice_from_raw_parts(address as *const u8, len) };

        Ok(mem)
       
    }

    fn erase(&mut self, address: u32) -> core::result::Result<(), DFUMemError> {
        
        if address < self.flash.address() as u32 + BOOTLOADER_SIZE_BYTES {
            return Err(DFUMemError::Address);
        }

        if address >= self.flash.address() as u32 + self.flash.len() as u32 {
            return Err(DFUMemError::Address);
        }

        let flash_offset = address as usize - self.flash.address();
        
        match self.flash.sector(flash_offset) {
            Some(sector) => {
                // let's try to start by not accepting aligned addresses
                if sector.offset != flash_offset {
                    return Err(DFUMemError::Erase);
                }
                let mut unlocked_flash = self.flash.unlocked();
                // TO-DO: perform better error translation here
                unlocked_flash.erase(sector.number).unwrap();
                return Ok(());
            }
            None => return Ok(()) 
        }
    }

    fn erase_all(&mut self) -> Result<(), DFUMemError> {

        for sector in flash_sectors(self.flash.len(), self.flash.dual_bank()) {
             // skip erasing bootloader sectors
             if sector.offset < BOOTLOADER_SIZE_BYTES as usize {
                continue;
            }
            let mut unlocked_flash = self.flash.unlocked();
            // TO-DO: perform better error translation here
            unlocked_flash.erase(sector.number).unwrap();
        }

        return Ok(())
    }

    fn store_write_buffer(&mut self, src: &[u8]) -> core::result::Result<(), ()> {
        self.buffer[..src.len()].copy_from_slice(src);
        Ok(())
    }

    fn program(&mut self, address: u32, length: usize) -> core::result::Result<(), DFUMemError> {
        
        if address < self.flash.address() as u32 {
            return Err(DFUMemError::Address);
        }

        let offset = address - self.flash.address() as u32;

        // skip programming bootloader sectors
        if offset < BOOTLOADER_SIZE_BYTES {
            return Err(DFUMemError::Address);
        }

        // skip programming beyond the end of flash
        if offset as usize + length >= self.flash.len() {
            return Err(DFUMemError::Address);
        }

        let mut unlocked_flash = self.flash.unlocked();
        // TO-DO: perform better error translation here
        unlocked_flash.program(offset as usize, self.buffer[..length].iter()).unwrap();

        return Ok(())
    }

    fn manifestation(&mut self) -> Result<(), DFUManifestationError> {
        controller_reset();
    }
}

/// Return device serial based on U_ID registers.
fn read_serial() -> u32 {
    let u_id0 = 0x1FFF_7A10 as *const u32;
    let u_id1 = 0x1FFF_7A14 as *const u32;

    unsafe { u_id0.read().wrapping_add(u_id1.read()) }
}

/// Returns device serial number as hex string slice.
fn get_serial_str() -> &'static str {
    static mut SERIAL: [u8; 8] = [b' '; 8];
    let serial = unsafe { SERIAL.as_mut() };

    fn hex(v: u8) -> u8 {
        match v {
            0..=9 => v + b'0',
            0xa..=0xf => v - 0xa + b'a',
            _ => b' ',
        }
    }

    let sn = read_serial();

    for (i, d) in serial.iter_mut().enumerate() {
        *d = hex(((sn >> (i * 4)) & 0xf) as u8)
    }

    unsafe { str::from_utf8_unchecked(serial) }
}

/// Initialize, configure all peripherals, and setup USB DFU.
/// Interrupts must be disabled.
fn dfu_init() -> LedType {
    // let cortex = cortex_m::Peripherals::take().unwrap();
    let device = unsafe { pac::Peripherals::steal() };

    let rcc = device.RCC.constrain();

    let clocks = rcc
        .cfgr
        .use_hse(25.MHz())
        .sysclk(48.MHz())
        .pclk1(8.MHz())
        .require_pll48clk()
        .freeze();

    let gpioa = device.GPIOA.split();

    // Acquire the GPIOC peripheral
    let gpioc = device.GPIOC.split();
    let mut led = gpioc.pc13.into_push_pull_output();
    led.set_high();

    // BlackPill (check) board has a pull-up resistor on the D+ line.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    // This forced reset is needed only for development, without it host
    // will not reset your device when you upload new firmware.
    let mut usb_dp = gpioa.pa12.into_push_pull_output();

    usb_dp.set_low();
    cortex_m::asm::delay(1024*50);

    /* USB Peripheral */

    let usb_periph = USB {
        usb_global: device.OTG_FS_GLOBAL,
        usb_device: device.OTG_FS_DEVICE,
        usb_pwrclk: device.OTG_FS_PWRCLK,
        pin_dm: gpioa.pa11.into_alternate(),
        pin_dp: usb_dp.into_alternate(),
        hclk: clocks.hclk(),
    };


    let bus = unsafe {
        USB_BUS.as_mut_ptr().write(UsbBus::new(usb_periph, &mut EP_MEMORY ));
        &*USB_BUS.as_ptr()
    };

    /* DFU */
    let stm32mem = STM32Mem::new(LockedFlash::new(device.FLASH));

    unsafe {
        USB_DFU.as_mut_ptr().write(DFUClass::new(&bus, stm32mem));
    }

    /* USB device */

    let usb_dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x2b23, 0x1012))
        .manufacturer("Red Hat Inc.")
        .product("Jumpstarter DFU Mode")
        .serial_number(get_serial_str())
        .device_release(0x0001)// Intentionally keep a very low version 0.1, so that the main firmware
                                // will always have a higher version number.
        .self_powered(false)
        .max_power(250)
        .max_packet_size_0(64)
        .build();

    unsafe {
        USB_DEVICE.as_mut_ptr().write(usb_dev);
    }

    unsafe {
        cortex_m::peripheral::NVIC::unmask(stm32f4xx_hal::pac::Interrupt::OTG_FS);
    }

    return led;
}

fn minimal_init() {
    unsafe {
        // enable PWR
        (*RCC::ptr()).apb1enr.modify(|_, w| w.pwren().set_bit());
        (*RCC::ptr()).ahb1enr.modify(|_, w| w.gpioaen().set_bit());
    }

    unsafe {
        // setup PA0 as input with pull-up
        (*GPIOA::ptr()).moder.modify(|_, w| w.moder0().input());
        (*GPIOA::ptr()).pupdr.modify(|_, w| w.pupdr0().pull_up());
    }
 
    cortex_m::asm::delay(100);
}

/// Check if DFU force external condition.
/// Check KEY BUTTON state.
fn dfu_enforced() -> bool {
    // check BOOT1, PA0 state
    unsafe { (*GPIOA::ptr()).idr.read().idr0().bit_is_clear() }
}

/// Reset registers that were used for a
/// check if DFU mode must be enabled to a
/// default values before starting main firmware.
fn quick_uninit() {
    unsafe {
        (*GPIOA::ptr()).moder.reset();
        (*GPIOA::ptr()).pupdr.reset();
        (*RCC::ptr()).apb1enr.reset();
        (*RCC::ptr()).ahb1enr.reset();
    }
}

/// Initialize stack pointer and jump to a main firmware.
#[inline(never)]
fn jump_to_app() -> ! {
    let vt = FW_ADDRESS as *const u32;
    unsafe {
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
        cortex_m::peripheral::Peripherals::steal().SCB.vtor.write(vt as u32);
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
        cortex_m::asm::bootload(vt);
    }
}

/// Check if FW looks OK and jump to it, or return.
fn try_start_app() {
    let sp = unsafe { (FW_ADDRESS as *const u32).read() };
    if sp & 0xfffc_0000 == 0x2000_0000 {
        quick_uninit();
        jump_to_app();
    }
}

/// Read magic value to determine if
/// device must enter DFU mode.
fn get_uninit_val() -> u32 {
    let p = 0x2000_0000 as *mut u32;
    unsafe { p.read_volatile() }
}

/// Erase magic value in RAM so that
/// DFU would be triggered only once.
fn clear_uninit_val() {
    let p = 0x2000_0000 as *mut u32;
    unsafe { p.write_volatile(0) };
}

/// Return true if "uninit" area of RAM has a
/// special value. Used to force DFU mode from
/// a main firmware programmatically.
fn dfu_ram_requested() -> bool {
    let stay = get_uninit_val() == KEY_STAY_IN_BOOT;
    if stay {
        clear_uninit_val();
    }
    stay
}

#[entry]
fn main() -> ! {
    if !dfu_ram_requested() {
        minimal_init();
        if !dfu_enforced() {
            try_start_app();
        }
    }

    cortex_m::interrupt::disable();

    let mut led = dfu_init();

    cortex_m::asm::dsb();
    unsafe { cortex_m::interrupt::enable() };

    loop {
        cortex_m::asm::delay(24*1024*1024);
        led.toggle();
    }
}

fn controller_reset() -> ! {
    cortex_m::interrupt::disable();

    let cortex = unsafe { cortex_m::Peripherals::steal() };

    cortex_m::asm::dsb();
    unsafe {
        // System reset request
        cortex.SCB.aircr.modify(|v| 0x05FA_0004 | (v & 0x700));
    }
    cortex_m::asm::dsb();

    loop {}
}

#[interrupt]
fn OTG_FS() {
    let usb_dev = unsafe { &mut *USB_DEVICE.as_mut_ptr() };
    let dfu = unsafe { &mut *USB_DFU.as_mut_ptr() };

    usb_dev.poll(&mut [dfu]);
}
