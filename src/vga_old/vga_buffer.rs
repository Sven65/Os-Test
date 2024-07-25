use volatile::Volatile;
use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use alloc::vec::Vec;
use crate::vga_old::ansii::convert_ansii_to_color;
use crate::{serial_println};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
	Black = 0,
	Blue = 1,
	Green = 2,
	Cyan = 3,
	Red = 4,
	Magenta = 5,
	Brown = 6,
	LightGray = 7,
	DarkGray = 8,
	LightBlue = 9,
	LightGreen = 10,
	LightCyan = 11,
	LightRed = 12,
	Pink = 13,
	Yellow = 14,
	White = 15,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    pub fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
	chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
    color_buf: Vec<u8>,
    is_escaped: bool,
}

impl Writer {
    pub fn handle_escape(&mut self, byte: u8) {
        match byte {
            b'\x1b' => {
                // ESC
                self.is_escaped = true;
            }
            b'm' => {
            	serial_println!("Encountered an m");

                self.is_escaped = false;
                self.color_code = convert_ansii_to_color(self.color_buf.clone());
                
                self.color_buf = Vec::new();
            }
            byte => {
                self.color_buf.push(byte as u8);

                // let row = BUFFER_HEIGHT - 1;
                // let col = self.column_position;

                // let color_code = self.color_code;

                // self.buffer.chars[row][col].write(ScreenChar {
                //     ascii_character: byte,
                //     color_code,
                // });
                // self.column_position += 1;
            }
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b'\x7f' => {// Backspace
                self.column_position -= 1;
                self.buffer.chars[BUFFER_HEIGHT - 1][self.column_position].write(ScreenChar {
                    ascii_character: b' ',
                    color_code: self.color_code,
                });
            }
           
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
				self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
    }

	pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x1b => self.handle_escape(byte),
                0x20..=0x7e | b'\n' | 0x7f => {
                    if self.is_escaped {
                        self.handle_escape(byte);
                    } else {
                        self.write_byte(byte);
                    }
                }
                // not part of printable ASCII range
                _ => self.write_byte(0xfe),
            }

        }
    }

    pub fn clear_color(&mut self) {
        self.color_code = ColorCode::new(Color::White, Color::Black);
    }

	fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

	fn clear_row (&mut self, row: usize) {
		let blank = ScreenChar {
			ascii_character: b' ',
			color_code: self.color_code,
		};

		for col in 0..BUFFER_WIDTH {
			self.buffer.chars[row][col].write(blank);
		}
	}
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::White, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
        is_escaped: false,
        color_buf: Vec::new(),
    });
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_old::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! reset_color {
    ($($arg:tt)*) => ($crate::vga_old::vga_buffer::_reset_color());
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

#[doc(hidden)]
pub fn _reset_color() {
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        WRITER.lock().clear_color();
    });
}

pub fn get_chars() {
    use x86_64::instructions::interrupts;


    interrupts::without_interrupts(|| {
        serial_println!("{:#?}", WRITER.lock().buffer.chars[0][0]);
    });
}

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}

#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}