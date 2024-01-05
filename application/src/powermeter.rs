use crate::filter::{self, Filter};
use core::fmt::Write;

pub trait PowerMeter {
        fn get_power(&mut self) -> f32;
        fn get_voltage(&mut self) -> f32;
        fn get_current(&mut self) -> f32;
        fn feed_voltage(&mut self, value:f32);
        fn feed_current(&mut self, value:f32);
        fn write_trace(&mut self, writer: &mut dyn Write);
        fn write(&mut self, writer: &mut dyn Write);

}

// Moving average power meter
pub struct MAVPowerMeter {
        voltage: filter::MovingAverage,
        current: filter::MovingAverage,
}

impl MAVPowerMeter {
        pub fn new() -> Self {
                Self{voltage: filter::MovingAverage::new(),
                     current: filter::MovingAverage::new()}
        }
}

impl PowerMeter for MAVPowerMeter {
        fn get_power(&mut self) -> f32 {
                self.voltage.get() * self.current.get()
        }
        fn get_voltage(&mut self) -> f32 {
                self.voltage.get()
        }
        fn get_current(&mut self) -> f32 {
                self.current.get()
        }
        fn feed_voltage(&mut self, value:f32) {
                self.voltage.feed(value);
        }
        fn feed_current(&mut self, value:f32) {
                self.current.feed(value);
        }

        fn write_trace(&mut self, writer: &mut dyn Write) {
            let pw_w = self.get_power();
            write!(writer, "{:.2}W> ",pw_w).ok();
        }
        fn write(&mut self, writer: &mut dyn Write) {
            let pw_w = self.get_power();
            let pw_v = self.get_voltage();
            let pw_a = self.get_current();

            write!(writer, "{:.2}A {:.2}V {:.2}W", pw_a, pw_v, pw_w).ok();
        }
}