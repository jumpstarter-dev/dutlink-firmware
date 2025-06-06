#!/bin/bash

# DUTLink Firmware Flash Script
# Downloads latest release and flashes to device using dfu-util

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# GitHub repository info
REPO_OWNER="jumpstarter-dev"
REPO_NAME="dutlink-firmware"
GITHUB_API_URL="https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest"

echo -e "${BLUE}DUTLink Firmware Flash Script${NC}"
echo "================================"

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}Error: This script must be run as root for dfu-util to work properly.${NC}"
   echo "Please run: sudo $0"
   exit 1
fi

echo -e "${GREEN}✓ Running as root${NC}"

# Check if dfu-util is installed
if ! command -v dfu-util &> /dev/null; then
    echo -e "${RED}Error: dfu-util is not installed.${NC}"
    echo "Please install dfu-util first:"
    echo "  Ubuntu/Debian: apt install dfu-util"
    echo "  macOS: brew install dfu-util"
    echo "  Fedora: dnf install dfu-util"
    exit 1
fi

echo -e "${GREEN}✓ dfu-util is installed${NC}"

# Check if curl is available
if ! command -v curl &> /dev/null; then
    echo -e "${RED}Error: curl is not installed.${NC}"
    echo "Please install curl first."
    exit 1
fi

echo -e "${GREEN}✓ curl is available${NC}"

# Create temporary directory
TEMP_DIR=$(mktemp -d)
echo -e "${BLUE}Using temporary directory: ${TEMP_DIR}${NC}"

# Cleanup function
cleanup() {
    echo -e "${YELLOW}Cleaning up temporary files...${NC}"
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# Get latest release info
echo -e "${BLUE}Fetching latest release information...${NC}"
RELEASE_INFO=$(curl -s "$GITHUB_API_URL")

if [[ $? -ne 0 ]]; then
    echo -e "${RED}Error: Failed to fetch release information from GitHub${NC}"
    exit 1
fi

# Extract tag name and download URLs
TAG_NAME=$(echo "$RELEASE_INFO" | grep '"tag_name":' | sed -E 's/.*"tag_name": "([^"]+)".*/\1/')
BOOTLOADER_URL=$(echo "$RELEASE_INFO" | grep '"browser_download_url":.*dfu-bootloader.*\.bin"' | sed -E 's/.*"browser_download_url": "([^"]+)".*/\1/')
APPLICATION_URL=$(echo "$RELEASE_INFO" | grep '"browser_download_url":.*jumpstarter\.bin"' | sed -E 's/.*"browser_download_url": "([^"]+)".*/\1/')

if [[ -z "$TAG_NAME" ]]; then
    echo -e "${RED}Error: Could not parse release tag name${NC}"
    exit 1
fi

if [[ -z "$BOOTLOADER_URL" || -z "$APPLICATION_URL" ]]; then
    echo -e "${RED}Error: Could not find bootloader or application binary URLs${NC}"
    echo "Available assets:"
    echo "$RELEASE_INFO" | grep '"browser_download_url":' | sed -E 's/.*"browser_download_url": "([^"]+)".*/\1/'
    exit 1
fi

echo -e "${GREEN}✓ Found release: ${TAG_NAME}${NC}"
echo -e "${BLUE}Bootloader URL: ${BOOTLOADER_URL}${NC}"
echo -e "${BLUE}Application URL: ${APPLICATION_URL}${NC}"

# Download binaries
echo -e "${BLUE}Downloading bootloader binary...${NC}"
curl -L -o "$TEMP_DIR/bootloader.bin" "$BOOTLOADER_URL"

echo -e "${BLUE}Downloading application binary...${NC}"
curl -L -o "$TEMP_DIR/application.bin" "$APPLICATION_URL"

echo -e "${GREEN}✓ Downloaded both binaries${NC}"

# Pause for user to prepare device
echo ""
echo -e "${YELLOW}DEVICE PREPARATION:${NC}"
echo "1. Connect your DUTLink device to USB"
echo "2. Put the device in DFU mode by:"
echo "   - Hold the FACTORY_DFU button"
echo "   - Press and release the RESET button"
echo "   - Release the FACTORY_DFU button"
echo "3. The device should now be in the MCU Factory DFU mode"
echo ""
read -p "Press Enter when the device is ready in DFU mode..."

# Check if DFU device is detected
echo -e "${BLUE}Checking for DFU device...${NC}"
dfu-util -l

if ! dfu-util -l | grep -q "0483:df11" ; then
    echo -e "${RED}Error: No DFU device found.${NC}"
    echo "Please ensure:"
    echo "1. Device is connected via USB"
    echo "2. Device is in DFU mode (follow the steps above)"
    echo "3. USB cable supports data transfer (not just power)"
    exit 1
fi

echo -e "${GREEN}✓ DFU device detected${NC}"

echo ""
echo -e "${YELLOW}FLASHING PROCESS:${NC}"


# Flash bootloader
echo -e "${BLUE}Flashing bootloader...${NC}"
if dfu-util -d 0483:df11 -a 0 -s 0x08000000:leave -D "$TEMP_DIR/bootloader.bin"; then
    echo -e "${GREEN}✓ Bootloader flashed successfully${NC}"
else
    echo -e "${RED}Error: Failed to flash bootloader${NC}"
    exit 1
fi

echo ""
echo -e "${YELLOW}FLASHING APP FIRMWARE ON TOP OF OUR BOOTLOADER${NC}"
echo ""

sleep 3

# Make sure that ht device is in bootloader mode, even if the app was previously flashed

dfu-util -e

sleep 3

# Check for DFU device again
echo -e "${BLUE}Checking for DFU device again...${NC}"
if ! dfu-util -l | grep -q "2b23:1012"; then
    echo -e "${RED}Error: No DFU device found for application flash.${NC}"
    echo "Please put the device back in DFU mode."
    exit 1
fi

echo -e "${GREEN}✓ DFU device detected for second stage bootloader${NC}"

# Flash application
echo -e "${BLUE}Flashing application...${NC}"

dfu-util -d 2b23:1012 -a 0 -s 0x08010000:leave -D "$TEMP_DIR/application.bin" || true

echo -e "${GREEN}🎉 FLASHING COMPLETE! 🎉${NC}"
echo "================================"
echo -e "${GREEN}Successfully flashed DUTLink firmware version: ${TAG_NAME}${NC}"
echo ""
echo "The device should now boot with the new firmware."
echo "You can verify the installation by connecting to the device"
echo "and checking the version with the 'version' command."
echo ""
echo -e "${BLUE}Next steps:${NC}"
echo "1. The device will reset automatically"
echo "2. Wait a few seconds for it to boot"
echo "3. Connect via serial terminal or USB to test"
