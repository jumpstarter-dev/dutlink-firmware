
use embedded_hal::digital::v2::OutputPin;

 // Device control abstractions

 pub trait StorageSwitchTrait {
     fn power_off(&mut self);
     fn connect_to_dut(&mut self);
     fn connect_to_host(&mut self);
 }
 pub struct StorageSwitch<OEnPin, SelPin, PwDUTPin, PWHostPin>
 where
     OEnPin: OutputPin,
     SelPin: OutputPin,
     PwDUTPin: OutputPin,
     PWHostPin: OutputPin,
 {
     usb_store_oen: OEnPin,
     usb_store_sel: SelPin,
     usb_pw_dut: PwDUTPin,
     usb_pw_host: PWHostPin,
 }

 impl<OEnPin, SelPin, PwDUTPin, PWHostPin> StorageSwitch<OEnPin, SelPin, PwDUTPin, PWHostPin>
 where
     OEnPin: OutputPin,
     SelPin: OutputPin,
     PwDUTPin: OutputPin,
     PWHostPin: OutputPin,
 {
     pub fn new(
         usb_store_oen: OEnPin,
         usb_store_sel: SelPin,
         usb_pw_dut: PwDUTPin,
         usb_pw_host: PWHostPin,
     ) -> Self {
         Self {
             usb_store_oen,
             usb_store_sel,
             usb_pw_dut,
             usb_pw_host,
         }
     }
}

 impl<OEnPin, SelPin, PwDUTPin, PWHostPin> StorageSwitchTrait for StorageSwitch<OEnPin, SelPin, PwDUTPin, PWHostPin>
 where
     OEnPin: OutputPin,
     SelPin: OutputPin,
     PwDUTPin: OutputPin,
     PWHostPin: OutputPin,
 {

    fn power_off(&mut self) {
        self.usb_pw_dut.set_low().ok();
        self.usb_pw_host.set_low().ok();
        self.usb_store_oen.set_high().ok();
     }

     fn connect_to_dut(&mut self) {
         self.usb_pw_host.set_low().ok();
         self.usb_pw_dut.set_high().ok();
         self.usb_store_oen.set_low().ok();
         self.usb_store_sel.set_high().ok();
     }

     fn connect_to_host(&mut self) {
         self.usb_pw_dut.set_low().ok();
         self.usb_pw_host.set_high().ok();
         self.usb_store_oen.set_low().ok();
         self.usb_store_sel.set_low().ok();
     }
 }
