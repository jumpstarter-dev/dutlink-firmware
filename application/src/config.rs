use core::{mem::size_of, cmp::min};

use stm32f4xx_hal::flash::{LockedFlash, FlashExt};

// Configuration is stored in the 3'rd sector of the flash memory, starting at 0x0800_C000.
// The sector is 16k, so we can store 16 ConfigBlocks of 1k each. The last one with 
// the magic word is the valid one.
// Each sector has a limited amout of times it can be erased, so we use the next free block
// and only erase the sector when all blocks are used.

const FLASH_SECTOR : u8 = 3;
const FLASH_BASE : usize = 0x0800_0000;
const FLASH_CONFIG_BASE : usize = 0x0800_C000; // see memory.x

#[repr(C, packed)]
#[derive(Debug)]
pub struct ConfigBlock {
    pub name: [u8; 64],       // device name
    pub tags: [u8; 256],      // device tags
    pub usb_console: [u8; 64], // separate usb console i.e. used for the orin agx board to access the USB only UEFI console
    // New variables can go here, but make sure to update the padding below
    // the previously stored versions will be 0's due to the padding
    pub power_on: [u8; 32], // power_on method i.e. "bL,w1,bZ"
    pub power_off: [u8; 32], // power_off method i.e. "bL,w11,bZ"
    pub power_rescue: [u8; 32], // power_off method i.e. "aL,rL,w1,rZ,w1,aZ"
    pub json : [u8; 512], // json blob config
    padding: [u8; 1024-64-256-64-4-32-32-32-512], // padding to make up for 1024 byte blocks
    magic: u32,           // magic word to know if this flash config block is valid

}

impl ConfigBlock {
    pub fn new() -> Self {
        ConfigBlock {
            name: [0; 64],
            tags: [0; 256],
            usb_console: [0; 64],
            power_on: [0; 32],
            power_off: [0; 32],
            power_rescue: [0; 32],
            json : [0; 512], // json blob config
            magic: MAGIC,
            padding: [0; 1024-64-256-64-4-32-32-32-512],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC
    }

    pub fn format_error(&self) -> bool {
        self.magic != MAGIC && self.magic != 0xFFFF_FFFF
    }

    pub fn set_name(mut self,name: &[u8]) -> Self {
        let l = min(name.len(), self.name.len());
        self.name[..l].copy_from_slice(&name[..l]);
        self.name[l..].fill(0);
        self
    }

    pub fn set_tags(mut self, tags: &[u8]) -> Self {
        let l = min(tags.len(), self.tags.len());
        self.tags[..l].copy_from_slice(&tags[..l]);
        self.tags[l..].fill(0);
        self
    }

    pub fn set_json(mut self, json: &[u8]) -> Self {
        let l = min(json.len(), self.json.len());
        self.json[..l].copy_from_slice(&json[..l]);
        self.json[l..].fill(0);
        self
    }

    pub fn set_usb_console(mut self, usb_console: &[u8]) -> Self {
        let l = min(usb_console.len(), self.usb_console.len());
        self.usb_console[..l].copy_from_slice(&usb_console[..l]);
        self.usb_console[l..].fill(0);
        self
    }

    pub fn set_power_on(mut self, power_on: &[u8]) -> Self {
        let l = min(power_on.len(), self.power_on.len());
        self.power_on[..l].copy_from_slice(&power_on[..l]);
        self.power_on[l..].fill(0);
        self
    }
    pub fn set_power_off(mut self, power_off: &[u8]) -> Self {
        let l = min(power_off.len(), self.power_off.len());
        self.power_off[..l].copy_from_slice(&power_off[..l]);
        self.power_off[l..].fill(0);
        self
    }

    pub fn set_power_rescue(mut self, power_rescue: &[u8]) -> Self {
        let l = min(power_rescue.len(), self.power_rescue.len());
        self.power_rescue[..l].copy_from_slice(&power_rescue[..l]);
        self.power_rescue[l..].fill(0);
        self
    }

}

const MAGIC: u32 = 0x601dbeef;

// The flash area in 0x0800_C000 - 0x0800_FFFF is reserved for the config block.
#[repr(C, packed)]
struct ConfigAreaFlash {
    config: [ConfigBlock; 16],
    // DO NOT ADD MORE VARIABLES HERE
}

pub struct ConfigArea {
    flash_config: &'static ConfigAreaFlash,
    flash: LockedFlash,
}

impl ConfigArea {
    pub fn new(flash: LockedFlash) -> Self {
        let mut cfg = ConfigArea {
            flash_config: ConfigAreaFlash::new(),
            flash: flash,
        };
        if cfg.flash_config.format_error() {
            cfg.erase_flash();
        }
        cfg
    }

    pub fn get(&self) -> ConfigBlock {
        self.flash_config.ram_config()
    }

    fn erase_flash(&mut self) {
        let mut unlocked_flash = self.flash.unlocked();
        unlocked_flash.erase(FLASH_SECTOR).unwrap();
    }
    pub fn write_config(&mut self, cfg: &ConfigBlock) -> Result<(),()> {
        let next = self.flash_config.get_next();
        let next_i: usize;
        match next {
            Some(i) => {
                next_i = i;
            },
            None => {
                self.erase_flash();
                next_i = 0;
            },
        }
        let offset = next_i * size_of::<ConfigBlock>();
        let mut unlocked_flash = self.flash.unlocked();
        let buffer = unsafe { as_u8_slice(cfg) };
        let base = FLASH_CONFIG_BASE - FLASH_BASE;
        unlocked_flash.program(base + offset, buffer.iter()).unwrap();

        Ok(())
    }
}

impl ConfigAreaFlash {
    fn new() -> &'static Self {
        let cfg = FLASH_CONFIG_BASE as *const ConfigAreaFlash;
        return unsafe { &*cfg };
    }

    fn get_next(&self) -> Option<usize> {
        for i in 0..16 {
            if !self.config[i].is_valid() {
                return Some(i)
            }
        }
        return None
    }

    // detect if any of the config blocks have a format error (magic word is not 0x600dbeef of 0xffffffff)
    pub fn format_error(&self) -> bool {
        for i in 0..16 {
            if self.config[i].format_error() {
                return true
            }
        }
        return false
    }

    fn get_current(&self) -> Option<usize> {
        for i in (0..16).rev() {
            if self.config[i].is_valid() {
                return Some(i)
            }
        }
        return None
    }

    fn get_config(&self) -> Option<&ConfigBlock> {
        match self.get_current() {
            Some(i) => Some(&self.config[i]),
            None => None,
        }
    }

    // get the last config block, as a copy in RAM
    fn ram_config(&self) -> ConfigBlock {
        let mut ram_cfg = ConfigBlock::new();

        match self.get_config() {
            Some(cfg) => {
                let src: &[u8] = unsafe { as_u8_slice(cfg) };
                let dst: &mut [u8] = unsafe { as_mut_u8_slice(&mut ram_cfg) };
                dst.copy_from_slice(src);
            },
            None => {},
        };
        ram_cfg
    }
}


unsafe fn as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::core::slice::from_raw_parts(
        (p as *const T) as *mut u8,
        ::core::mem::size_of::<T>(),
    )
}

unsafe fn as_mut_u8_slice<T: Sized>(p: &T) -> &mut [u8] {
    ::core::slice::from_raw_parts_mut(
        (p as *const T) as *mut u8,
        ::core::mem::size_of::<T>(),
    )
}
