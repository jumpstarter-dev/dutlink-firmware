/**
 * WebDFU - DFU implementation for WebUSB
 * Based on the DFU 1.1 specification
 */

class WebDFU {
    constructor() {
        this.device = null;
        this.interface = null;
        this.transferSize = 2048;
        
        // DFU Class-specific requests
        this.DFU_DETACH = 0x00;
        this.DFU_DNLOAD = 0x01;
        this.DFU_UPLOAD = 0x02;
        this.DFU_GETSTATUS = 0x03;
        this.DFU_CLRSTATUS = 0x04;
        this.DFU_GETSTATE = 0x05;
        this.DFU_ABORT = 0x06;
        
        // DFU States
        this.DFU_STATE = {
            appIDLE: 0,
            appDETACH: 1,
            dfuIDLE: 2,
            dfuDNLOAD_SYNC: 3,
            dfuDNBUSY: 4,
            dfuDNLOAD_IDLE: 5,
            dfuMANIFEST_SYNC: 6,
            dfuMANIFEST: 7,
            dfuMANIFEST_WAIT_RESET: 8,
            dfuUPLOAD_IDLE: 9,
            dfuERROR: 10
        };
        
        // DFU Status
        this.DFU_STATUS = {
            OK: 0x00,
            errTARGET: 0x01,
            errFILE: 0x02,
            errWRITE: 0x03,
            errERASE: 0x04,
            errCHECK_ERASED: 0x05,
            errPROG: 0x06,
            errVERIFY: 0x07,
            errADDRESS: 0x08,
            errNOTDONE: 0x09,
            errFIRMWARE: 0x0A,
            errVENDOR: 0x0B,
            errUSBR: 0x0C,
            errPOR: 0x0D,
            errUNKNOWN: 0x0E,
            errSTALLEDPKT: 0x0F
        };
    }

    async connect(filters = []) {
        try {
            // Default filters for common DFU devices
            const defaultFilters = [
                { vendorId: 0x0483, productId: 0xdf11 }, // STM32 Factory DFU
                { vendorId: 0x2b23, productId: 0x1012 }  // Custom bootloader
            ];
            
            const allFilters = filters.length > 0 ? filters : defaultFilters;
            
            this.device = await navigator.usb.requestDevice({
                filters: allFilters
            });
            
            await this.device.open();
            
            // Find DFU interface
            for (const config of this.device.configurations) {
                for (const iface of config.interfaces) {
                    for (const alt of iface.alternates) {
                        if (alt.interfaceClass === 0xFE && alt.interfaceSubclass === 0x01) {
                            this.interface = {
                                number: iface.interfaceNumber,
                                alternate: alt.alternateSetting
                            };
                            break;
                        }
                    }
                    if (this.interface) break;
                }
                if (this.interface) break;
            }
            
            if (!this.interface) {
                throw new Error('No DFU interface found');
            }
            
            await this.device.selectConfiguration(1);
            await this.device.claimInterface(this.interface.number);
            await this.device.selectAlternateInterface(this.interface.number, this.interface.alternate);
            
            return true;
        } catch (error) {
            console.error('Failed to connect:', error);
            throw error;
        }
    }

    async disconnect() {
        if (this.device) {
            try {
                await this.device.close();
            } catch (error) {
                console.error('Error closing device:', error);
            }
            this.device = null;
            this.interface = null;
        }
    }

    async getStatus() {
        const result = await this.device.controlTransferIn({
            requestType: 'class',
            recipient: 'interface',
            request: this.DFU_GETSTATUS,
            value: 0,
            index: this.interface.number
        }, 6);
        
        if (result.status !== 'ok') {
            throw new Error('Failed to get DFU status');
        }
        
        const data = new Uint8Array(result.data.buffer);
        return {
            status: data[0],
            pollTimeout: data[1] | (data[2] << 8) | (data[3] << 16),
            state: data[4],
            string: data[5]
        };
    }

    async clearStatus() {
        const result = await this.device.controlTransferOut({
            requestType: 'class',
            recipient: 'interface',
            request: this.DFU_CLRSTATUS,
            value: 0,
            index: this.interface.number
        });
        
        if (result.status !== 'ok') {
            throw new Error('Failed to clear DFU status');
        }
    }

    async download(data, address = 0) {
        let blockNum = 0;
        let bytesWritten = 0;
        
        // If address is specified, send address first (for STM32)
        if (address > 0) {
            const addressBytes = new Uint8Array(5);
            addressBytes[0] = 0x21; // Set Address Pointer command
            addressBytes[1] = address & 0xFF;
            addressBytes[2] = (address >> 8) & 0xFF;
            addressBytes[3] = (address >> 16) & 0xFF;
            addressBytes[4] = (address >> 24) & 0xFF;
            
            await this.downloadBlock(blockNum++, addressBytes);
            await this.waitForReady();
        }
        
        while (bytesWritten < data.length) {
            const chunkSize = Math.min(this.transferSize, data.length - bytesWritten);
            const chunk = data.slice(bytesWritten, bytesWritten + chunkSize);
            
            await this.downloadBlock(blockNum++, chunk);
            await this.waitForReady();
            
            bytesWritten += chunkSize;
            
            // Report progress
            if (this.onProgress) {
                this.onProgress(bytesWritten, data.length);
            }
        }
        
        // Send zero-length packet to indicate end of download
        await this.downloadBlock(blockNum, new Uint8Array(0));
        await this.waitForReady();
        
        return bytesWritten;
    }

    async downloadBlock(blockNum, data) {
        const result = await this.device.controlTransferOut({
            requestType: 'class',
            recipient: 'interface',
            request: this.DFU_DNLOAD,
            value: blockNum,
            index: this.interface.number
        }, data);
        
        if (result.status !== 'ok') {
            throw new Error(`Failed to download block ${blockNum}`);
        }
    }

    async waitForReady() {
        let status;
        do {
            status = await this.getStatus();
            
            if (status.status !== this.DFU_STATUS.OK) {
                throw new Error(`DFU Error: ${status.status}`);
            }
            
            if (status.state === this.DFU_STATE.dfuDNBUSY) {
                // Wait for the specified poll timeout
                await new Promise(resolve => setTimeout(resolve, status.pollTimeout));
            }
        } while (status.state === this.DFU_STATE.dfuDNBUSY);
        
        if (status.state === this.DFU_STATE.dfuERROR) {
            await this.clearStatus();
            throw new Error('DFU entered error state');
        }
    }

    async detach() {
        try {
            await this.device.controlTransferOut({
                requestType: 'class',
                recipient: 'interface',
                request: this.DFU_DETACH,
                value: 1000, // Timeout in ms
                index: this.interface.number
            });
        } catch (error) {
            // Detach often causes the device to disconnect, which is expected
            console.log('Device detached (expected)');
        }
    }
}

// Export for use in other scripts
window.WebDFU = WebDFU;
