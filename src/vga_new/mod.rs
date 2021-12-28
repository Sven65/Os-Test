use vga::colors::{Color16, TextModeColor};
use vga::writers::{ScreenCharacter, TextWriter, Text80x25};
use core::{fmt, fmt::Write};
use spin::Mutex;


use crate::serial_println;
use lazy_static::lazy_static;

pub struct Writer {
	mode: Text80x25,
	column_position: usize,
	color_code: TextModeColor,
}

pub fn init() {
	let text_mode = Text80x25::new();
    let color = TextModeColor::new(Color16::Yellow, Color16::Black);
    let screen_character = ScreenCharacter::new(b'T', color);

    WRITER.lock().mode.set_mode();
    WRITER.lock().mode.clear_screen();
    WRITER.lock().mode.write_character(0, 0, screen_character);
}

lazy_static! {
	pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
		mode: Text80x25::new(),
		column_position: 0,
		color_code: TextModeColor::new(Color16::Yellow, Color16::Black),
	});
}

#[doc(hidden)]
pub fn _print(args: &str) {
	let color = TextModeColor::new(Color16::Yellow, Color16::Black);

	for (offset, character) in args.chars().enumerate() {
        serial_println!("Printing char {}", character);

        let screen_char = ScreenCharacter::new(character as u8, color);

        WRITER.lock().mode.write_character(0 + offset, 0, screen_char);
    }
}