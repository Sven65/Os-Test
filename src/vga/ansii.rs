use alloc::vec::Vec;
use crate::vga::vga_buffer::{ColorCode, Color};

pub fn convert_ansii_to_color(buf: Vec<u8>) -> ColorCode {
	let mut color_code: ColorCode = ColorCode::new(Color::White, Color::Black);

	match &*buf {
		b"[30" => { color_code = ColorCode::new(Color::Black, Color::Black); },
		b"[31" => { color_code = ColorCode::new(Color::Red, Color::Black); },
		b"[32" => { color_code = ColorCode::new(Color::Green, Color::Black); },
		b"[33" => { color_code = ColorCode::new(Color::Yellow, Color::Black); },
		b"[34" => { color_code = ColorCode::new(Color::Blue, Color::Black); },
		b"[35" => { color_code = ColorCode::new(Color::Magenta, Color::Black); },
		b"[36" => { color_code = ColorCode::new(Color::Cyan, Color::Black); },
		b"[37" => { color_code = ColorCode::new(Color::White, Color::Black); },
		_ => {},
	}

	color_code
}