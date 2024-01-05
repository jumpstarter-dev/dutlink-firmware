# Copyright (C) 2023 Miguel Angel Ajo <majopela@redhat.com>
#
# Licensed under the GNU General Public License Version 2
#
# This program is free software; you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation; either version 2 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program; if not, write to the Free Software
# Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.
#

all: dfu-bootloader.bin

target/thumbv7em-none-eabihf/release/dfu-bootloader: src/main.rs Cargo.toml memory.x
	cargo build --release

dfu-bootloader.bin: target/thumbv7em-none-eabihf/release/dfu-bootloader
	arm-none-eabi-objcopy -O binary target/thumbv7em-none-eabihf/release/dfu-bootloader dfu-bootloader.bin

st-flash: dfu-bootloader.bin
	st-flash write dfu-bootloader.bin 0x8000000

flash: dfu-bootloader.bin
	sudo dfu-util -d 0483:df11 -a 0 -s 0x08000000:leave -D dfu-bootloader.bin

flash-erase: dfu-bootloader.bin
	sudo dfu-util -d 0483:df11 -a 0 -s 0x08000000:mass-erase:force:leave -D dfu-bootloader.bin

openocd-b: flash
	openocd -c -f openocd.cfg

openocd: flash
	openocd -f openocd.cfg

clean:
	rm -rf target dfu-bootloader.bin

.build-container:
	podman build -f ../application/Containerfile -t firmware-builder
	touch .build-container

firmware-builder: .build-container
	podman run -ti --rm -v $(CURDIR)/../../:/workdir -w /workdir/firmware/bootloader firmware-builder bash

in-container: .build-container
	podman run -ti --rm -v $(CURDIR)/../../:/workdir -w /workdir/firmware/bootloader firmware-builder make $(THIS)

.PHONY: clean all flash openocd
