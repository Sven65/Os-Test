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
		
		// Backgrounds

		b"[40" => { color_code = ColorCode::new(Color::White, Color::Black); },
		b"[41" => { color_code = ColorCode::new(Color::White, Color::Red); },
		b"[42" => { color_code = ColorCode::new(Color::White, Color::Green); },
		b"[43" => { color_code = ColorCode::new(Color::White, Color::Yellow); },
		b"[44" => { color_code = ColorCode::new(Color::White, Color::Blue); },
		b"[45" => { color_code = ColorCode::new(Color::White, Color::Magenta); },	
		b"[46" => { color_code = ColorCode::new(Color::White, Color::Cyan); },	
		b"[47" => { color_code = ColorCode::new(Color::White, Color::White); },	
		_ => {},
	}

	color_code
}