use vga::colors::{Color16, TextModeColor};
use vga::writers::{ScreenCharacter, TextWriter, Text80x25, Screen};
use core::{fmt, fmt::Write};
use spin::Mutex;


use crate::serial_println;
use lazy_static::lazy_static;


#[derive(Debug)]
pub struct Writer {
	mode: Text80x25,
	column_position: usize,
	color_code: TextModeColor,

}

lazy_static! {
	pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
		mode: Text80x25::new(),
		column_position: 0,
		color_code: TextModeColor::new(Color16::Yellow, Color16::Black),
	});
}

impl Writer {
	pub fn handle_escape(&mut self, _byte: u8) {

	}

	pub fn write_byte(&mut self, byte: u8) {
		//if (self.column_position >= Text80x25.)
		
		let screen_char = ScreenCharacter::new(byte, self.color_code);
	
		self.mode.write_character(self.column_position, 24, screen_char);
	
		self.column_position += 1;
	}

	pub fn write_string(&mut self, s: &str) {
		for byte in s.bytes() {
			match byte {
				0x1b => self.handle_escape(byte),
				b'\n' => self.new_line(),
				_ => self.write_byte(byte),
			}
		}
	}

	fn new_line(&mut self) {
		let chars_left = Text80x25::WIDTH - self.column_position;

		for row in 1..Text80x25::HEIGHT {
			for col in 0..Text80x25::WIDTH {
				let character = self.mode.read_character(row, col);
				self.mode.write_character(row - 1, col, character);
			}
		}

		for _ in 0..chars_left {
			self.write_byte(b' ');
		}
		
		self.column_position = 0;


	}
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
		serial_println!("D: {:?}", Text80x25::WIDTH);

        self.write_string(s);
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
	WRITER.lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! new_print {
    ($($arg:tt)*) => ($crate::vga_new::_print(format_args!($($arg)*)));
}
