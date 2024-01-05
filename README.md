# dutlink-board firmware

This repository contains the firmware for the [dutlink-board](https://github.com/jumpstarter-dev/dutlink-board) project, a project within jumpstarter
that enables easy testing and development of SOC/SOM and other systems with Hardware in the Loop.

Jumpstarter is a project designed to enable generic Hardware in the Loop testability for Edge / embedded designs.

The firmware is written in Rust, and it is made of two parts:
 * The bootloader, which enables seamless firmware updates via the [fwupd](https://fwupd.org) project.
 * The application, which provides the functionality described in the following sections.


you can find out more about jumpstarter in https://jumpstarter.dev

# Description

The DUTLink board communicates with a host via USB, enabling the switching of a USB3 storage device
between the DUT and the Test host. This enables very fast storage injection on the DUT,
and very fast lifecycles for testing.

In addition DUTLink can control power, provide console access via TX/RX UART, and measure power
consumption. See features for a more detailed insight.


## Features

* USB Storage sharing
  * A storage device can be connected to the Testing host, and then passed down to the DUT
  * USB3.1 (up to 5Gbps, potentially 10Gbps future releases) capable
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


