/**
 * DUTLink Firmware Flasher Web Application
 */

let dfu = null;
let releaseData = null;
let bootloaderBinary = null;
let applicationBinary = null;

// GitHub repository info
const REPO_OWNER = "jumpstarter-dev";
const REPO_NAME = "dutlink-firmware";
const GITHUB_API_URL = `https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest`;

// Initialize the application
document.addEventListener('DOMContentLoaded', function() {
    checkWebUSBSupport();
    checkHTTPS();
});

function checkWebUSBSupport() {
    if (!navigator.usb) {
        document.getElementById('webusb-support').classList.remove('hidden');
        return false;
    }
    return true;
}

function checkHTTPS() {
    if (location.protocol !== 'https:' && location.hostname !== 'localhost') {
        document.getElementById('https-warning').classList.remove('hidden');
        return false;
    }
    return true;
}

function addStatus(message, type = 'info') {
    const statusDiv = document.getElementById('status-messages');
    const statusElement = document.createElement('div');
    statusElement.className = `status ${type}`;
    statusElement.textContent = message;
    statusDiv.appendChild(statusElement);
    
    // Auto-remove after 10 seconds for non-error messages
    if (type !== 'error') {
        setTimeout(() => {
            if (statusElement.parentNode) {
                statusElement.parentNode.removeChild(statusElement);
            }
        }, 10000);
    }
    
    // Scroll to bottom
    statusDiv.scrollTop = statusDiv.scrollHeight;
}

function addLog(message) {
    const logContainer = document.getElementById('log-container');
    logContainer.classList.remove('hidden');
    logContainer.textContent += new Date().toLocaleTimeString() + ': ' + message + '\n';
    logContainer.scrollTop = logContainer.scrollHeight;
}

function updateProgress(current, total) {
    const progressContainer = document.getElementById('progress-container');
    const progressBar = document.getElementById('progress-bar');
    
    if (total > 0) {
        progressContainer.classList.remove('hidden');
        const percentage = (current / total) * 100;
        progressBar.style.width = percentage + '%';
    } else {
        progressContainer.classList.add('hidden');
    }
}

async function checkLatestRelease() {
    const button = document.getElementById('check-release-btn');
    button.disabled = true;
    button.textContent = '🔄 Checking...';
    
    try {
        addStatus('Fetching latest release information...', 'info');
        addLog('Fetching release data from GitHub API');
        
        const response = await fetch(GITHUB_API_URL);
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        
        releaseData = await response.json();
        
        // Find bootloader and application assets
        const bootloaderAsset = releaseData.assets.find(asset => 
            asset.name.includes('dfu-bootloader') && asset.name.endsWith('.bin')
        );
        const applicationAsset = releaseData.assets.find(asset => 
            asset.name.includes('jumpstarter') && asset.name.endsWith('.bin')
        );
        
        if (!bootloaderAsset || !applicationAsset) {
            throw new Error('Could not find required binary assets in release');
        }
        
        // Update UI with release info
        document.getElementById('release-version').textContent = releaseData.tag_name;
        document.getElementById('bootloader-size').textContent = formatBytes(bootloaderAsset.size);
        document.getElementById('application-size').textContent = formatBytes(applicationAsset.size);
        document.getElementById('release-info').classList.remove('hidden');
        
        // Download binaries
        addStatus('Downloading firmware binaries...', 'info');
        addLog('Downloading bootloader binary');
        bootloaderBinary = await downloadBinary(bootloaderAsset.browser_download_url);
        
        addLog('Downloading application binary');
        applicationBinary = await downloadBinary(applicationAsset.browser_download_url);
        
        addStatus(`✅ Ready to flash firmware ${releaseData.tag_name}`, 'success');
        addLog('Binaries downloaded successfully');
        
        // Enable connect button
        document.getElementById('connect-btn').disabled = false;
        
    } catch (error) {
        addStatus(`❌ Failed to fetch release: ${error.message}`, 'error');
        addLog(`Error: ${error.message}`);
    } finally {
        button.disabled = false;
        button.textContent = '📡 Check Latest Release';
    }
}

async function downloadBinary(url) {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`Failed to download: HTTP ${response.status}`);
    }
    
    const arrayBuffer = await response.arrayBuffer();
    return new Uint8Array(arrayBuffer);
}

function formatBytes(bytes) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

async function connectDevice() {
    const button = document.getElementById('connect-btn');
    button.disabled = true;
    button.textContent = '🔄 Connecting...';
    
    try {
        addStatus('Connecting to DFU device...', 'info');
        addLog('Requesting USB device access');
        
        dfu = new WebDFU();
        await dfu.connect();
        
        addStatus('✅ Connected to DFU device', 'success');
        addLog(`Connected to device: ${dfu.device.productName || 'Unknown'}`);
        
        // Enable flash button
        document.getElementById('flash-btn').disabled = false;
        button.textContent = '✅ Connected';
        
    } catch (error) {
        addStatus(`❌ Failed to connect: ${error.message}`, 'error');
        addLog(`Connection error: ${error.message}`);
        button.disabled = false;
        button.textContent = '🔌 Connect to Device';
        dfu = null;
    }
}

async function flashFirmware() {
    if (!dfu || !bootloaderBinary || !applicationBinary) {
        addStatus('❌ Not ready to flash. Please check release and connect device first.', 'error');
        return;
    }
    
    const button = document.getElementById('flash-btn');
    button.disabled = true;
    button.textContent = '⚡ Flashing...';
    
    try {
        // Set up progress callback
        dfu.onProgress = updateProgress;
        
        addStatus('🔄 Starting firmware flash process...', 'info');
        addLog('Starting flash process');
        
        // Flash bootloader first
        addStatus('📝 Flashing bootloader...', 'info');
        addLog('Flashing bootloader to 0x08000000');
        await dfu.download(bootloaderBinary, 0x08000000);
        
        addStatus('✅ Bootloader flashed successfully', 'success');
        addLog('Bootloader flash completed');
        
        // Detach to reset device
        addStatus('🔄 Resetting device...', 'info');
        addLog('Detaching device to trigger reset');
        await dfu.detach();
        await dfu.disconnect();
        
        // Wait for device to reset and enter custom bootloader
        addStatus('⏳ Waiting for device to enter custom bootloader...', 'warning');
        addLog('Waiting 3 seconds for device reset');
        await new Promise(resolve => setTimeout(resolve, 3000));
        
        // Reconnect to custom bootloader
        addStatus('🔄 Connecting to custom bootloader...', 'info');
        addLog('Attempting to connect to custom bootloader');
        
        dfu = new WebDFU();
        await dfu.connect([{ vendorId: 0x2b23, productId: 0x1012 }]);
        dfu.onProgress = updateProgress;
        
        addStatus('✅ Connected to custom bootloader', 'success');
        addLog('Connected to custom bootloader');
        
        // Flash application
        addStatus('📝 Flashing application...', 'info');
        addLog('Flashing application to 0x08010000');
        await dfu.download(applicationBinary, 0x08010000);
        
        addStatus('✅ Application flashed successfully', 'success');
        addLog('Application flash completed');
        
        // Final detach
        addLog('Final device detach');
        await dfu.detach();
        await dfu.disconnect();
        
        updateProgress(0, 0); // Hide progress bar
        
        addStatus('🎉 FIRMWARE FLASH COMPLETE! 🎉', 'success');
        addStatus(`Successfully flashed DUTLink firmware ${releaseData.tag_name}`, 'success');
        addLog('Flash process completed successfully');
        
        // Reset UI
        document.getElementById('connect-btn').disabled = true;
        document.getElementById('connect-btn').textContent = '🔌 Connect to Device';
        
    } catch (error) {
        addStatus(`❌ Flash failed: ${error.message}`, 'error');
        addLog(`Flash error: ${error.message}`);
        
        if (dfu) {
            try {
                await dfu.disconnect();
            } catch (e) {
                console.error('Error disconnecting:', e);
            }
        }
        
        // Reset UI
        document.getElementById('connect-btn').disabled = false;
        document.getElementById('connect-btn').textContent = '🔌 Connect to Device';
        
    } finally {
        button.disabled = false;
        button.textContent = '⚡ Flash Firmware';
        updateProgress(0, 0);
        dfu = null;
    }
}

// Handle device disconnection
navigator.usb.addEventListener('disconnect', event => {
    if (dfu && dfu.device === event.device) {
        addStatus('📱 Device disconnected', 'warning');
        addLog('USB device disconnected');
        dfu = null;
        
        // Reset UI
        document.getElementById('connect-btn').disabled = false;
        document.getElementById('connect-btn').textContent = '🔌 Connect to Device';
        document.getElementById('flash-btn').disabled = true;
    }
});
