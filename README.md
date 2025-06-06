# dutlink-board firmware

This repository contains the firmware for the [dutlink-board](https://github.com/jumpstarter-dev/dutlink-board) project, a project within jumpstarter
that enables easy testing and development of SOC/SOM and other systems with Hardware in the Loop.

Jumpstarter is a project designed to enable generic Hardware in the Loop testability for Edge / embedded designs.

Dutlink is based on the [STM32F411](https://www.st.com/en/microcontrollers-microprocessors/stm32f411.html)
microcontroller and the [Rust](https://www.rust-lang.org/) programming language.

The firmware is written in Rust, and it is made of two parts:
 * The bootloader, which enables seamless firmware updates via the [fwupd](https://fwupd.org) project.
 * The application, which provides the functionality described in the following sections.


you can find out more about jumpstarter in https://jumpstarter.dev

# Flashing Firmware

## Quick Start - Using the Flash Script

The easiest way to flash firmware to your DUTLink device is using the provided `flash-dutlink.sh` script, which automatically downloads the latest release and flashes it to your device.

### Prerequisites

- **Linux system** (the script is designed for Linux)
- **Root access** (required for dfu-util to access USB devices)
- **dfu-util** installed on your system
- **curl** for downloading firmware

### Flashing Process

1. **Download the flash script:**
   ```bash
   wget https://raw.githubusercontent.com/jumpstarter-dev/dutlink-firmware/main/flash-dutlink.sh
   chmod +x flash-dutlink.sh
   ```

2. **Run the script as root:**
   ```bash
   sudo ./flash-dutlink.sh
   ```

3. **Follow the on-screen instructions:**
   - Connect your DUTLink device via USB
   - Put the device in DFU mode:
     - Hold the **FACTORY_DFU** button
     - Press and release the **RESET** button  
     - Release the **FACTORY_DFU** button
   - Press Enter when ready

4. **Wait for completion:**
   - The script will automatically download the latest firmware
   - Flash the bootloader first
   - Then flash the application firmware
   - The device will reset automatically when complete

The script handles the entire flashing process, including downloading the latest release binaries and managing the two-stage flashing process (bootloader + application).

# Devel environment

## Environment setup
For development environment in this case, we recommend a Fedora machine with the following
packages installed:

```bash
$ dnf install -y stlink openocd arm-none-eabi-binutils-cs gcab libappstream-glib rpm-build copr-cli dfu-util

# install the community version of rust, and the ARM thumbv7m-none-eaby target
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
$ source "$HOME/.cargo/env"
$ rustup target add thumbv7m-none-eabi
$ rustup target add thumbv7em-none-eabihf

```

One hack to install gcab in RHEL9 is:
```bash
sudo subscription-manager repos --enable=codeready-builder-for-rhel-9-x86_64-rpms
cd /tmp
dnf download --source libgcab1
sudo dnf builddep libgcab1
sudo dnf install rpm-build
sudo rpmbuild --rebuild libgcab*.src.rpm
sudo dnf install ~/rpmbuild/RPMS/$(uname -m)/gcab* ~/rpmbuild/RPMS/$(uname -m)/libgcab*
```

## Bootloader and Update protocol

The bootloader implements the [DFU](https://www.usb.org/sites/default/files/DFU_1.1.pdf)
/ DFUse protocols, as fwupd supports these protocols.

DFU defines two interfaces: a bootloader interface that can write, erase, or read (if enabled) the
application space, and a runtime interface that allows the host to request the application to jump
back to the bootloader.

The DFU protocol is implemented using the [usbd-dfu](https://github.com/vitalyvb/usbd-dfu) Rust
library, which is based on the [usb-device](https://github.com/rust-embedded-community/usb-device)
stack. We build the bootloaders using the [usbd-dfu-example](https://github.com/vitalyvb/usbd-dfu-example)
provided by the author.

When the application firmware is running, the applications implement the DFU runtime interface,
declaring the device as DFU-enabled and accepting the DFU_DETACH and GET_STATUS commands.

We implement the DFU runtime using the [usbd-dfu-rt](https://github.com/jedrzejboczar/usbd-dfu-rt) Rust
library, which is based on the [usb-device](https://github.com/rust-embedded-community/usb-device) stack.

# Description

The DUTLink board communicates with a host via USB, enabling the switching of a USB3 storage device
between the DUT and the Test host. This enables very fast storage injection on the DUT,
and very fast lifecycles for testing.

In addition DUTLink can control power, provide console access via TX/RX UART, and measure power
consumption. See features for a more detailed insight.


## Features

* USB Storage sharing
  * A storage device can be connected to the Testing host, and then passed down to the DUT
  * USB3.1 (up to 5Gbps, Do not use 10Gbps devices, we exceed the USB3 spec trace lengths, and the signal is degraded) capable
  * USB3.0 and 2.0 capable for backwards compatibility
  * Storage device power cycling

* DUT Power management:
  * ON/OFF, Reset, Reboot
  * Power consumption measurement and logging in watts
  * 5-25V power supply support
  * USB-PD power supply support 5-20V with DUT power negotiation
 
* DUT Control (3.3v I/O)
  * UART Serial port TX/RX (for logging and tracing)
  * 4 customizable control signals
  * RESET signal


