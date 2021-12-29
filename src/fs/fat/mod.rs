use crate::fs::vfs::FsFunctions;
use crate::serial_println;


#[derive(Debug, Clone, Copy, Default)]
pub struct FatFs;

impl FsFunctions for FatFs {
	fn read(&self, offset: u32, size: u32, buffer: u8) -> u32 {
		serial_println!("offset: {}, size: {}, buf: {}", offset, size, buffer);

		return 10;
	}
	
	fn write(&self, offset: u32, size: u32, buffer: u8) -> u32 {
		return 11;
	}

	fn open(&self, read: u8, write: u8) {}

	fn close(&self) {}
}