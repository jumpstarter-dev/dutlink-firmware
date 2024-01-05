use stm32f4xx_hal::otg_fs::UsbBusType;
use usbd_serial::SerialPort;

pub type USBSerialType = SerialPort<'static, UsbBusType, BufferStore, BufferStore>;

 // Bigger USB Serial buffer
 use core::borrow::{Borrow, BorrowMut};

pub const BUFFER_SIZE:usize = 1024;
 pub struct BufferStore(pub [u8; BUFFER_SIZE]);

 impl Borrow<[u8]> for BufferStore {
     fn borrow(&self) -> &[u8] {
         &self.0
     }
 }

 impl BorrowMut<[u8]> for BufferStore {
     fn borrow_mut(&mut self) -> &mut [u8] {
         &mut self.0
     }
 }

 macro_rules! new_usb_serial {
     ($usb:expr) => {
         SerialPort::new_with_store($usb, BufferStore([0; BUFFER_SIZE]), BufferStore([0; BUFFER_SIZE]))
     };
 }
 pub(crate) use new_usb_serial;
