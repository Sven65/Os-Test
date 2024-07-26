use alloc::vec::Vec;
use crate::vga_old::vga_buffer::{ColorCode, Color};
use core::str::from_utf8;
use crate::serial_println;

// TODO: Make this support light mode
fn convert_code_to_color(color: &str) -> Color {
	match color {
		// FG colors

		"30" => Color::Black,
		"31" => Color::Red,
		"32" => Color::Green,
		"33" => Color::Yellow,
		"34" => Color::Blue,
		"35" => Color::Magenta,
		"36" => Color::Cyan,
		"37" => Color::White,

		// Bg colors

		"40" => Color::Black,
		"41" => Color::Red,
		"42" => Color::Green,
		"43" => Color::Yellow,
		"44" => Color::Blue,
		"45" => Color::Magenta,	
		"46" => Color::Cyan,	
		"47" => Color::White,

		_ => Color::Black,
	}
}

pub fn convert_ansii_to_color(buf: Vec<u8>) -> ColorCode {
	let mut fg_color: Color = Color::White;
	let mut bg_color: Color = Color::Black;
	let color_code: ColorCode;

	let color_result = from_utf8(&*buf);

	serial_println!("Color result: {:#?}", color_result);


	serial_println!("Yeet");

	match color_result {
		Err(_) => {
			panic!("Exception when parsing color code");
		},
		Ok(v) => {
			serial_println!("Color string: {}", v.replace("[", ""));

			let replaced_string = v.replace("[", "");
			// let mut is_light = false;

			let split = replaced_string.split(";");

			serial_println!("Hello, Serial World!");

			for part in split {
				serial_println!("Part {}", part);
				serial_println!("Color is {:#?}", convert_code_to_color(part));
			
				let ch = part.chars().nth(0).unwrap();

				match &ch {
					// '1' => { is_light = true; },
					'3' => {fg_color = convert_code_to_color(part); },
					'4' => {bg_color = convert_code_to_color(part); },
					_ => {},
				}
			}

			serial_println!("FG: {:#?}, BG: {:#?}", fg_color, bg_color);

			color_code = ColorCode::new(fg_color, bg_color); 
		}
	}

	color_code
}