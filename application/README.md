# DUTLink board application

We implement the DFU runtime using the [usbd-dfu-rt](https://github.com/jedrzejboczar/usbd-dfu-rt)
Rust library from [jedrzejboczar](https://github.com/jedrzejboczar).

This folder contains a Makefile, the sources and an example firmware.metadata.xml.

The Makefile will help you build the firmware binary, and .cab files to work with fwupd.
```
