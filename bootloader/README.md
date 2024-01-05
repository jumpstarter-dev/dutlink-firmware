# STM32F103 Example DFU Bootloader for fwupd

This bootloader is based on the [usbd-dfu](https://github.com/vitalyvb/usbd-dfu)
stack example bootloader published [here](https://github.com/vitalyvb/usbd-dfu-example),
from [vitalyvb](https://github.com/vitalyvb).

It implements the DFUse extensions (DFU 1.1a), and declares a memory map via USB
descriptor string in the following way:

```rust
const MEM_INFO_STRING: &'static str = "@Flash/0x08010000/01*064Kg,03*128Kg";
```

The exposed map does not include the bootloader address space 0x08000000 - 0x08007fff
since that confuses fwupd, and it doesn't make sense to expose since the bootloader
can't update itself.

It also does not contain the area in 0x08008000 to 0x0800FFFF used for configuration storage
by the firmware.

This directory contains a Makefile which should help you flash the bootloader into a
device. Please read the (security considerations)[../../#bootloader-security-considerations].


## Building

To make the bootloader you can use:

```bash
$ make
cargo build --release
   Compiling semver-parser v0.7.0
..
   Compiling dfu-bootloader v0.2.0 (/home/majopela/firmware/et/firmware-on-the-edge/firmware-examples/stm32f103/bootloader)
..
    Finished release [optimized + debuginfo] target(s) in 15.20s
arm-none-eabi-objcopy -O binary target/thumbv7em-none-eabihf/release/dfu-bootloader dfu-bootloader.bin
```

## Flashing

You can flash your device with the bootloader using `make flash` via dfu-util or `st-flash` if you
have an STLINKv3 connected to your board.

i.e. connected to your board while pushing the FACTORY-DFU button, this will
trigget the internal ROM DFU bootloader from the chip.

```bash
$ make flash
sudo dfu-util -d 0483:df11 -a 0 -s 0x08000000:leave -D dfu-bootloader.bin
dfu-util 0.11

Copyright 2005-2009 Weston Schmidt, Harald Welte and OpenMoko Inc.
Copyright 2010-2021 Tormod Volden and Stefan Schmidt
This program is Free Software and has ABSOLUTELY NO WARRANTY
Please report bugs to http://sourceforge.net/p/dfu-util/tickets/

dfu-util: Warning: Invalid DFU suffix signature
dfu-util: A valid DFU suffix will be required in a future dfu-util release
Opening DFU capable USB device...
Device ID 0483:df11
Device DFU version 011a
Claiming USB DFU Interface...
Setting Alternate Interface #0 ...
Determining device status...
DFU state(10) = dfuERROR, status(10) = Device's firmware is corrupt. It cannot return to run-time (non-DFU) operations
Clearing status
Determining device status...
DFU state(2) = dfuIDLE, status(0) = No error condition is present
DFU mode device DFU version 011a
Device returned transfer size 2048
DfuSe interface name: "Internal Flash  "
Downloading element to address = 0x08000000, size = 14340
Erase           [=========================] 100%        14340 bytes
Erase    done.
Download        [=========================] 100%        14340 bytes
Download done.
File downloaded successfully
Submitting leave request...
Transitioning to dfuMANIFEST state
```

The bootloader is located at 0x08000000, and uses 32KB. Leaving additional 2*16KB sectors
for configuration storage. This means that the application should be compiled to run at
0x08010000 (see the memory.x linkerscript in the application and bootloader directories).


Once flashed, you can see the device connecting via USB on the dmesg output:

(TODO @mangelajo: update this output)
```
[ 116.701570] usb 2-2.7: new full-speed USB device number 120 using xhci_hcd
[ 116.843527] usb 2-2.7: New USB device found, idVendor=2b23, idProduct=e012, bcdDevice= 0.01
[ 116.843555] usb 2-2.7: New USB device strings: Mfr=1, Product=2, SerialNumber=3
[ 116.843560] usb 2-2.7: Product: DFU Bootloader for STM32F411CEU6
[ 116.843564] usb 2-2.7: Manufacturer: Red Hat
[ 116.843567] usb 2-2.7: SerialNumber: c6156613
```

Now when powering up your device, you can use the DFU button if you want to force
our DFU bootloader to stay instead of jumping to your final application when
it's already flashed.

