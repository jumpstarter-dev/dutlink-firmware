use core::str;

use stm32f4xx_hal::otg_fs::UsbBusType;
use usbd_dfu_rt::{DfuRuntimeClass, DfuRuntimeOps};
use usb_device::class_prelude::*;

pub struct DFUBootloader; 

pub type DFUBootloaderRuntime = DfuRuntimeClass<DFUBootloader>;

const KEY_STAY_IN_BOOT: u32 = 0xb0d42b89;

impl DfuRuntimeOps for DFUBootloader {
    const DETACH_TIMEOUT_MS: u16 = 500;
    const CAN_UPLOAD: bool = false;
    const WILL_DETACH: bool = true;

    fn detach(&mut self) {
        cortex_m::interrupt::disable();

        let cortex = unsafe { cortex_m::Peripherals::steal() };

        let p = 0x2000_0000 as *mut u32;
        unsafe { p.write_volatile(KEY_STAY_IN_BOOT) };

        cortex_m::asm::dsb();
        unsafe {
            // System reset request
            cortex.SCB.aircr.modify(|v| 0x05FA_0004 | (v & 0x700));
        }
        cortex_m::asm::dsb();
        loop {}
    }
}

/// Returns device serial number as hex string slice.
pub fn get_serial_str() -> &'static str {
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

/// Return device serial based on U_ID registers.
fn read_serial() -> u32 {
    let u_id0 = 0x1FFF_7A10 as *const u32;
    let u_id1 = 0x1FFF_7A14 as *const u32;

    unsafe { u_id0.read().wrapping_add(u_id1.read()) }
}

pub fn new_dfu_bootloader(u:&UsbBusAllocator<UsbBusType>) -> DFUBootloaderRuntime {
    DfuRuntimeClass::new(u, DFUBootloader)
}