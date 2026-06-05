use volatile::Volatile;
use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::port::Port;
use alloc::vec::Vec;
use crate::serial_println;

pub mod ansii;

pub const BUFFER_HEIGHT: usize = 25;
pub const BUFFER_WIDTH: usize = 80;

// VGA controller ports for cursor control
const VGA_CTRL_PORT: u16 = 0x3D4;
const VGA_DATA_PORT: u16 = 0x3D5;
const VGA_CURSOR_HIGH: u8 = 0x0E;
const VGA_CURSOR_LOW: u8 = 0x0F;

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
pub struct ScreenChar {
    pub ascii_character: u8,
    pub color_code: ColorCode,
}

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    pub row: usize,
    pub col: usize,
    pub color_code: ColorCode,
    buffer: &'static mut Buffer,
    // ANSI escape handling
    color_buf: Vec<u8>,
    is_escaped: bool,
}

impl Writer {
    // Write a character at an arbitrary position without moving the cursor
    pub fn write_at(&mut self, row: usize, col: usize, byte: u8, color: ColorCode) {
        if row >= BUFFER_HEIGHT || col >= BUFFER_WIDTH {
            return;
        }
        self.buffer.chars[row][col].write(ScreenChar {
            ascii_character: byte,
            color_code: color,
        });
    }

    // Write a string at an arbitrary position, returns the col after the last char
    pub fn write_str_at(&mut self, row: usize, col: usize, s: &str, color: ColorCode) -> usize {
        let mut c = col;
        for byte in s.bytes() {
            if c >= BUFFER_WIDTH { break; }
            self.write_at(row, c, byte, color);
            c += 1;
        }
        c
    }

    // Fill a row with spaces using a given color — useful for status bar and clearing lines
    pub fn clear_row_color(&mut self, row: usize, color: ColorCode) {
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(ScreenChar {
                ascii_character: b' ',
                color_code: color,
            });
        }
    }

    // Clear the entire screen with the current color
    pub fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row_color(row, self.color_code);
        }
        self.row = 0;
        self.col = 0;
        self.move_hardware_cursor(0, 0);
    }

    // Move the blinking hardware cursor to a position.
    // This is done via two port I/O writes to the VGA controller.
    // The position is a single number: row * 80 + col.
    // We send the high byte first, then the low byte.
    pub fn move_hardware_cursor(&mut self, row: usize, col: usize) {
        let pos = (row * BUFFER_WIDTH + col) as u16;
        unsafe {
            let mut ctrl = Port::<u8>::new(VGA_CTRL_PORT);
            let mut data = Port::<u8>::new(VGA_DATA_PORT);
            // High byte
            ctrl.write(VGA_CURSOR_HIGH);
            data.write((pos >> 8) as u8);
            // Low byte
            ctrl.write(VGA_CURSOR_LOW);
            data.write((pos & 0xFF) as u8);
        }
    }

    // Read what character is currently at a position
    pub fn read_at(&self, row: usize, col: usize) -> ScreenChar {
        self.buffer.chars[row][col].read()
    }

    // The regular terminal write_byte — writes at current cursor and advances it
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b'\x7f' => {
                // Backspace
                if self.col > 0 {
                    self.col -= 1;
                }
                self.buffer.chars[self.row][self.col].write(ScreenChar {
                    ascii_character: b' ',
                    color_code: self.color_code,
                });
                self.move_hardware_cursor(self.row, self.col);
            }
            byte => {
                if self.col >= BUFFER_WIDTH {
                    self.new_line();
                }
                let color_code = self.color_code;
                self.buffer.chars[self.row][self.col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.col += 1;
                self.move_hardware_cursor(self.row, self.col);
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x1b => self.handle_escape(byte),
                0x20..=0x7e | b'\n' | 0x7f => {
                    if self.is_escaped {
                        self.handle_escape(byte);
                    } else {
                        self.write_byte(byte);
                    }
                }
                _ => self.write_byte(0xfe),
            }
        }
    }

    pub fn clear_color(&mut self) {
        self.color_code = ColorCode::new(Color::White, Color::Black);
    }

    pub fn set_color(&mut self, color: ColorCode) {
        self.color_code = color;
    }

    fn handle_escape(&mut self, byte: u8) {
        match byte {
            b'\x1b' => {
                self.is_escaped = true;
            }
            b'm' => {
                self.is_escaped = false;
                self.color_code = ansii::convert_ansii_to_color(self.color_buf.clone());
                self.color_buf = Vec::new();
            }
            byte => {
                self.color_buf.push(byte);
            }
        }
    }

    fn new_line(&mut self) {
        if self.row < BUFFER_HEIGHT - 1 {
            self.row += 1;
        } else {
            // Scroll up
            for row in 1..BUFFER_HEIGHT {
                for col in 0..BUFFER_WIDTH {
                    let ch = self.buffer.chars[row][col].read();
                    self.buffer.chars[row - 1][col].write(ch);
                }
            }
            self.clear_row_color(BUFFER_HEIGHT - 1, self.color_code);
        }
        self.col = 0;
        self.move_hardware_cursor(self.row, self.col);
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        row: 23, // start near bottom like a terminal, leaving row 24 for status
        col: 0,
        color_code: ColorCode::new(Color::White, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
        is_escaped: false,
        color_buf: Vec::new(),
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! reset_color {
    () => ($crate::vga::_reset_color());
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

// Public helpers for the editor and other code that needs direct screen access
pub fn write_at(row: usize, col: usize, byte: u8, color: ColorCode) {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().write_at(row, col, byte, color);
    });
}

pub fn write_str_at(row: usize, col: usize, s: &str, color: ColorCode) -> usize {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().write_str_at(row, col, s, color)
    })
}

pub fn clear_screen() {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().clear_screen();
    });
}

pub fn clear_row(row: usize, color: ColorCode) {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().clear_row_color(row, color);
    });
}

pub fn move_cursor(row: usize, col: usize) {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().move_hardware_cursor(row, col);
    });
}

pub fn get_chars() {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        serial_println!("{:#?}", WRITER.lock().buffer.chars[0][0]);
    });
}