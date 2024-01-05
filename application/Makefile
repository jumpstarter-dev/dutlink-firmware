# Copyright (C) 2015 Richard Hughes <richard@hughsie.com>
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

TARGET      = jumpstarter
TARGET_ELF  = target/thumbv7em-none-eabihf/release/jumpstarter
TARGET_DEBUG_ELF  = target/thumbv7em-none-eabihf/debug/jumpstarter
VERSION     = 0.07

METADATA_DATE = $(shell date -u +%Y-%m-%d)
GIT_REF=$(shell git describe --always --abbrev=12 --dirty)
TARGET_CAB = ${TARGET}-${VERSION}.cab

CAB_FILES =					\
	jumpstarter.bin			\
	firmware.metainfo.xml

all: $(TARGET_CAB)

firmware.metainfo.xml: firmware.metainfo.xml.in Makefile
	sed -e "s/@VERSION@/${VERSION}/g" $< | sed "s/@DATE@/${METADATA_DATE}/g" > $@

check: firmware.metainfo.xml
	appstream-util validate-relax $<

$(TARGET_ELF): src/*.rs Cargo.toml memory.x Makefile
	VERSION=${VERSION} GIT_REF=${GIT_REF} cargo build --release

$(TARGET_DEBUG_ELF): src/*.rs Cargo.toml memory.x Makefile
	VERSION=${VERSION} GIT_REF=${GIT_REF} cargo build

debug: $(TARGET_DEBUG_ELF)
	gdb ./target/thumbv7em-none-eabihf/debug/jumpstarter -x openocd.gdb

jumpstarter.bin: $(TARGET_ELF) Makefile
	arm-none-eabi-objcopy -Obinary $(TARGET_ELF) jumpstarter.bin

jumpstarter.hex: $(TARGET_ELF) Makefile
	arm-none-eabi-objcopy -Oihex $(TARGET_ELF) jumpstarter.hex

rpmbuild/SOURCES:
	mkdir -p ./rpmbuild/{BUILD,RPMS,SOURCES,SPECS,SRPMS)

rpmbuild/SOURCES/$(TARGET_CAB): rpmbuild/SOURCES $(TARGET_CAB)
	cp $(TARGET_CAB) rpmbuild/SOURCES

rpm: rpmbuild/SOURCES/$(TARGET_CAB) jumpstarter-firmware.spec
	rpmbuild -bb --define "_topdir $(CURDIR)/rpmbuild" jumpstarter-firmware.spec

srpm: rpmbuild/SOURCES/$(TARGET_CAB) jumpstarter-firmware.spec
	rpmbuild -bs --define "_topdir $(CURDIR)/rpmbuild" jumpstarter-firmware.spec

$(TARGET_CAB): $(CAB_FILES) check
	gcab --create --nopath $@ $(CAB_FILES)

copr: clean srpm
	copr-cli build @redhat-et/jumpstarter rpmbuild/SRPMS/jumpstarter*.src.rpm

flash: $(TARGET_CAB)
	sudo fwupdtool install -vv $(TARGET_CAB) --force --allow-older --allow-reinstall

flash-dfu: jumpstarter.bin
	sudo dfu-util -s 0x08010000:force:leave -D jumpstarter.bin --verbose

clean:
	rm -rf target jumpstarter.hex jumpstarter.bin *.cab rpmbuild

.build-container:
	podman build -f Containerfile -t firmware-builder
	touch .build-container

firmware-builder: .build-container
	podman run -ti --rm -v $(CURDIR)/../../:/workdir -w /workdir/firmware/application firmware-builder bash

in-container: .build-container
	podman run -ti --rm -v $(CURDIR)/../../:/workdir -w /workdir/firmware/application firmware-builder make $(THIS)