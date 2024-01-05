
use core::fmt::Write;
const VERSION: &str = env!("VERSION");
const GIT_REF: &str  = env!("GIT_REF");

pub const fn version() -> &'static str {
    VERSION
}

pub const fn git_ref() -> &'static str {
    GIT_REF
}

pub fn write_version(writer: &mut dyn Write) {
    write!(writer, "{} git-ref: {}", version(), git_ref()).ok();
}

pub const fn usb_version_bcd_device() -> u16 {
    _usb_version_bcd_device(version())
}

const fn _usb_version_bcd_device(version_str:&'static str) -> u16 {
    let mut major: u16 = 0;
    let mut minor: u16 = 0;
    let mut major_found = false;

    let mut bytes = version_str.as_bytes();

    // version is in the form of "major.minor" with BCD coding
    // the result shoudl be coded in BCD where the MSB byte is
    // the major version, amd the LSB byte is the minor version
    while let [byte, rest @ ..] = bytes {
        bytes = rest;
        let c = *byte as char;
        if c == '.' {
            major_found = true;
            continue;
        }
        if !major_found {
            major = (major << 4)  + (c as u16 - '0' as u16);
        } else {
            minor = (minor << 4) + (c as u16 - '0' as u16);
        }
    }

    let version = (major <<8) | minor;

    version
}
