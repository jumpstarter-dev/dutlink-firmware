use core::str;
use embedded_hal::digital::v2::OutputPin;
use core::fmt::Write;

use arrayvec::ArrayString;

use crate::config::ConfigArea;
use crate::ctlpins::{PinState, CTLPinsTrait};
use crate::powermeter::PowerMeter;
use crate::{usbserial::*, ctlpins::CTLPins};
use crate::storage::StorageSwitchTrait;
use crate::version;

use ushell::{
    autocomplete::StaticAutocomplete, history::LRUHistory, Input as ushell_input,
    ShellError as ushell_error, UShell,
};
const N_COMMANDS: usize = 14;
const COMMANDS: [&str; N_COMMANDS] = ["help", "about", "get-config", "version", "meter", "storage", "send",
                                      "set", "set-config", "monitor", "power", "console", "status", "clear"];
pub type ShellType = UShell<USBSerialType, StaticAutocomplete<N_COMMANDS>, LRUHistory<512, 10>, 512>;
pub struct ShellStatus {
    pub monitor_enabled: bool,
    pub meter_enabled: bool,
    pub console_mode: bool,
}

pub const SHELL_PROMPT: &str = "#> ";
pub const CR: &str = "\r\n";

pub const HELP: &str = "\r\n\
        about               : print information about this device\r\n\
        clear               : clear the screen\r\n\
        help                : print this help\r\n\
        meter on|read|off   : read power consumption\r\n\
        monitor on|off      : enable or disable the serial console monitor in this terminal\r\n\
        console             : enter into serial console mode, exit with CTRL+A 5 times\r\n\
        power on|off        : power on or off the DUT\r\n\
        send string         : send string to the DUT\r\n\
        set r|a|b|c|d l|h|z : set RESET, CTL_A,B,C or D to low, high or high impedance\r\n\
        set-config name|tags|json|usb_console|poweron|poweroff value : set the config value in flash\r\n\
        get-config          : print all the config parameters\r\n\
        status              : print status of the device\r\n\
        storage dut|host|off: connect storage to DUT, host or disconnect\r\n\
        version             : print version information\r\n\
        ";

pub const ABOUT: &str = "\r\n\
        Jumpstarter test-harness version: ";
pub const ABOUT_CONTINUATION: &str = "\r\n\r\n\
          This is a device for testing images and power consumption of Edge devices in CI or\r\n\
          development, made as Open Hardware, designed to be used with the jumpstarter project.\r\n\
          more information can be found here:\r\n\r\n\
              https://github.com/redhat-et/jumpstarter\r\n\
        ";
pub fn new(serial:USBSerialType) -> ShellType {
    let autocomplete = StaticAutocomplete(COMMANDS);
    let history = LRUHistory::default();
    let shell: ShellType = UShell::new(serial, autocomplete, history);
    shell
}

pub fn handle_shell_commands<L, S, P>(shell: &mut ShellType,
                                      shell_status: &mut ShellStatus,
                                      led_cmd: &mut L,
                                      storage: &mut S,
                                      ctl_pins:&mut CTLPins<P>,
                                      send_to_dut: &mut dyn FnMut(&[u8]),
                                      power_meter: &mut dyn PowerMeter,
                                      config: &mut ConfigArea)
where
    L: OutputPin,
    S: StorageSwitchTrait,
    P: OutputPin,
{
    loop {
        let mut response = ArrayString::<512>::new();
        write!(response, "{0:}", CR).ok();

        let result = shell.poll();

        match result {
            Ok(Some(ushell_input::Command((cmd, args)))) => {
                led_cmd.set_low().ok();
                match cmd {
                        "about" =>      { write!(response, "{}", ABOUT).ok();
                                          version::write_version(&mut response);
                                          write!(response, "{}", ABOUT_CONTINUATION).ok();
                                        }
                        "help" =>       { shell.write_str(HELP).ok(); }
                        "clear" =>      { shell.clear().ok(); }
                        "console" =>    { handle_console_cmd(&mut response, args, shell_status); }
                        "monitor" =>    { handle_monitor_cmd(&mut response, args, shell_status); }
                        "meter" =>      { handle_meter_cmd(&mut response, args, shell_status, power_meter); }
                        "storage" =>    { handle_storage_cmd(&mut response, args, storage); }
                        "power" =>      { handle_power_cmd(&mut response, args, ctl_pins, config); }
                        "send" =>       { handle_send_cmd(&mut response, args, send_to_dut); }
                        "set" =>        { handle_set_cmd(&mut response, args, ctl_pins); }
                        "set-config" => { handle_set_config_cmd(&mut response, args, config); }
                        "get-config" => { handle_get_config_cmd(&mut response, args, config); }
                        "status" =>     { handle_status_cmd(&mut response, args, shell_status); }
                        "version" =>    { version::write_version(&mut response); }
                        "" =>           {}
                        _ =>            { write!(shell, "{0:}unsupported command{0:}", CR).ok(); }
                }
                // If response was added complete with an additional CR
                if response.len() > 2 {
                    write!(response, "{0:}", CR).ok();
                }
                // if console mode has been entered we should not print the SHELL PROMPT again
                if !shell_status.console_mode {
                    write!(response, "{}", SHELL_PROMPT).ok();
                }
                shell.write_str(&response).ok();

            }
            Err(ushell_error::WouldBlock) => break,
            _ => {}
        }
    }
}

fn handle_power_cmd<B, C>(response:&mut B, args: &str, ctlpins: &mut C, config: &ConfigArea)
where
    C: CTLPinsTrait,
    B: Write
 {
    if args == "on" {
        ctlpins.power_on(&config.get().power_on);
        write!(response, "Device powered on").ok();
    } else if args == "off" {
        ctlpins.power_off(&config.get().power_off);
        write!(response, "Device powered off").ok();
    } else if args == "force-off" {
        ctlpins.power_off(&[0u8; 0]);
        write!(response, "Device forced off").ok();
    } else if args == "force-on" {
        ctlpins.power_on(&[0u8; 0]);
        write!(response, "Device forced on").ok();
    } else if args == "rescue" {
        ctlpins.power_on(&config.get().power_rescue);
        write!(response, "Device powered on to rescue").ok();
    } else {
        write!(response, "usage: power on|off|force-on|force-off|rescue").ok();
    }
}

fn handle_send_cmd<B>(response:&mut B, args: &str, send_to_dut: &mut dyn FnMut(&[u8]))
where
    B: Write
 {
    if args.len() > 0 {
        send_to_dut(args.as_bytes());
    } else {
        write!(response, "usage: send string").ok();
    }
}

fn handle_storage_cmd<B,S>(response:&mut B, args: &str, storage: &mut S)
where
    S: StorageSwitchTrait,
    B: Write
 {
    if args == "dut" {
        storage.connect_to_dut();
        write!(response, "storage connected to device under test").ok();
    } else if args == "host" {
        storage.connect_to_host();
        write!(response, "storage connected to host").ok();
    } else if args == "off" {
        storage.power_off();
        write!(response, "storage disconnected").ok();
    } else {
        write!(response, "usage: storage dut|host|off").ok();
    }
}

fn handle_meter_cmd<B>(response:&mut B, args: &str, shell_status: &mut ShellStatus, power_meter: &mut dyn PowerMeter)
where
    B: Write
 {
    if args == "on" {
        shell_status.meter_enabled = true;
        write!(response, "Power meter monitoring enabled").ok();
    } else if args == "read" {
        power_meter.write(response);
    } else if args == "off" {
        shell_status.meter_enabled = false;
        write!(response, "Power monitor disabled").ok();
    } else {
        write!(response, "usage: meter on|read|off").ok();
    }
}

fn handle_monitor_cmd<B>(response:&mut B, args: &str, shell_status: &mut ShellStatus)
where
    B: Write
 {
    if args == "on" {
        shell_status.monitor_enabled = true;
        write!(response, "Monitor enabled").ok();
    } else if args == "off" {
        shell_status.monitor_enabled = false;
        write!(response, "Monitor disabled").ok();
    } else {
        write!(response, "usage: monitor on|off").ok();
    }
}

fn handle_console_cmd<B>(response:&mut B, args: &str, shell_status: &mut ShellStatus)
where
    B: Write
 {
    if args =="" {
        shell_status.console_mode = true;
        write!(response, "Entering console mode, type CTRL+B 5 times to exit").ok();
    } else {
        write!(response, "usage: console").ok();
    }
}

fn handle_set_cmd<B, C>(response:&mut B, args: &str, ctl_pins:&mut C)
where
    B: Write,
    C: CTLPinsTrait

 {

    if args.len() == 3 && args.as_bytes()[1] == ' ' as u8{
        let mut chars = args.chars();
        let ctl     = chars.next().unwrap();
        let _space  = chars.next().unwrap();
        let val     = chars.next().unwrap();

        if ctl != 'r' && ctl != 'a' && ctl != 'b' && ctl != 'c' && ctl != 'd' {
            write_set_usage(response);
            return;
        }

        if val != 'l' && val != 'h' && val != 'z' {
            write_set_usage(response);
            return;
        }

        let ctl_str = match ctl {
            'r' => "/RESET",
            'a' => "CTL_A",
            'b' => "CTL_B",
            'c' => "CTL_C",
            'd' => "CTL_D",
            _ => "",
        };

        let val_str = match val {
            'l' => "LOW",
            'h' => "HIGH",
            'z' => "HIGH IMPEDANCE",
            _ => "",
        };

        let ps = match val {
            'l' => PinState::Low,
            'h' => PinState::High,
            'z' => PinState::Floating,
            _ => PinState::Floating,
        };

        match ctl {
            'r' => ctl_pins.set_reset(ps),
            'a' => ctl_pins.set_ctl_a(ps),
            'b' => ctl_pins.set_ctl_b(ps),
            'c' => ctl_pins.set_ctl_c(ps),
            'd' => ctl_pins.set_ctl_d(ps),
            _ => {},
        };

        write!(response, "Set {} to {}", ctl_str, val_str).ok();
    } else {
        write_set_usage(response)
    }
}

fn handle_set_config_cmd<B>(response:&mut B, args: &str, config: &mut ConfigArea)
where
    B: Write
 {
    let mut split_args = args.split_ascii_whitespace();
    let key = split_args.next();
    let mut val = split_args.next();
    let mut usage = false;

    // empty argument = clear
    if val == None {
        val = Some("");
    }

    if let (Some(k), Some(v)) = (key, val) {
        let cfg = config.get();
        if k == "name" {
            let cfg = cfg.set_name(v.as_bytes());
            config.write_config(&cfg).ok();
            write!(response, "Set name to {}", v).ok();

        } else if k == "tags" {
            let cfg = cfg.set_tags(v.as_bytes());
            write!(response, "Set tags to {}", v).ok();
            config.write_config(&cfg).ok();

        } else if k == "json" {
            let cfg = cfg.set_json(v.as_bytes());
            write!(response, "Set json to {}", v).ok();
            config.write_config(&cfg).ok();

        } else if k == "usb_console" {
            let cfg = cfg.set_usb_console(v.as_bytes());
            write!(response, "Set usb_console to {}", v).ok();
            config.write_config(&cfg).ok();

        } else if k == "power_on" {
            let cfg = cfg.set_power_on(v.as_bytes());
            write!(response, "Set power_on to {}", v).ok();
            config.write_config(&cfg).ok();

        } else if k == "power_off" {
            let cfg = cfg.set_power_off(v.as_bytes());
            write!(response, "Set power_off to {}", v).ok();
            config.write_config(&cfg).ok();
        } else if k == "power_rescue" {
            let cfg = cfg.set_power_rescue(v.as_bytes());
            write!(response, "Set power_rescue to {}", v).ok();
            config.write_config(&cfg).ok();
        } else {
            usage = true;
        }
    } else {
        usage = true;
    }

    if usage {
        write!(response, "usage: set-config name|tags|storage|usb_storage value").ok();
    }
}

fn handle_get_config_cmd<B>(response:&mut B, args: &str, config: &mut ConfigArea)
where
    B: Write
 {
    let cfg = config.get();

    if args == "name" {
        write_u8(response, &cfg.name);
    } else if args == "tags" {
        write_u8(response, &cfg.tags);
    } else if args == "json" {
        write_u8(response, &cfg.json);
    } else if args == "usb_console" {
        write_u8(response, &cfg.usb_console);
    } else if args == "power_on" {
        write_u8(response, &cfg.power_on);
    } else if args == "power_off" {
        write_u8(response, &cfg.power_off);
    } else if args == "power_rescue" {
        write_u8(response, &cfg.power_rescue);
    } else if args == "" {
        write!(response, "name: ").ok();
        write_u8(response, &cfg.name);
        write!(response, "\r\ntags: ").ok();
        write_u8(response, &cfg.tags);
        write!(response, "\r\njson: ").ok();
        write_u8(response, &cfg.json);
        write!(response, "\r\nusb_console: ").ok();
        write_u8(response, &cfg.usb_console);
        write!(response, "\r\npower_on: ").ok();
        write_u8(response, &cfg.power_on);
        write!(response, "\r\npower_off: ").ok();
        write_u8(response, &cfg.power_off);
        write!(response, "\r\npower_rescue: ").ok();
        write_u8(response, &cfg.power_rescue);
    } else {
        write!(response, "usage: get-config [name|tags|json|usb_console|power_on|power_off|power_rescue]").ok();
    }
}

fn write_u8<B>(response:&mut B, val:&[u8])
where
    B: Write
{
    for c in val.iter() {
        if *c == 0 {
            break;
        }
        response.write_char(*c as char).ok();
    }
}

fn write_set_usage<B>(response:&mut B)
where
    B: Write
 {
    write!(response, "usage: set r|a|b|c|d l|h|z").ok();
}

fn handle_status_cmd<B>(response:&mut B, args: &str, shell_status: &mut ShellStatus)
where
    B: Write
 {
    if args =="" {
        write!(response, "Monitor: {}, Meter: {}", shell_status.monitor_enabled, shell_status.meter_enabled).ok();
    } else {
        write!(response, "usage: status").ok();
    }
}