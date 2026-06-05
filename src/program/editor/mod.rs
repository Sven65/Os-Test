use alloc::vec::Vec;
use alloc::string::String;
use alloc::vec;
use core::sync::atomic::Ordering;
use pc_keyboard::DecodedKey;
use crate::task::keyboard::InputFocus;
use crate::vga::{
    self, Color, ColorCode, BUFFER_HEIGHT, BUFFER_WIDTH,
};

use crate::task::yield_now;

const EDITOR_ROWS: usize = BUFFER_HEIGHT - 1; // leave last row for status bar
const STATUS_ROW: usize = BUFFER_HEIGHT - 1;

const COLOR_TEXT: ColorCode = ColorCode::new(Color::White, Color::Black);
const COLOR_STATUS: ColorCode = ColorCode::new(Color::Black, Color::LightGray);
const COLOR_CURSOR: ColorCode = ColorCode::new(Color::Black, Color::White);

pub struct Editor {
    lines: Vec<Vec<u8>>,   // file content as lines of bytes
    cursor_row: usize,     // which line the cursor is on
    cursor_col: usize,     // which column the cursor is on
    scroll_offset: usize,  // which line is at the top of the screen
    filename: String,
    modified: bool,
}

impl Editor {
    pub fn new(filename: &str) -> Self {
        let content = crate::fs::read_file(filename);
        let lines = match content {
            Some(data) => {
                let mut lines: Vec<Vec<u8>> = data
                    .split(|&b| b == b'\n')
                    .map(|l| l.to_vec())
                    .collect();
                if lines.is_empty() {
                    lines.push(Vec::new());
                }
                lines
            }
            None => vec![Vec::new()], // new file
        };

        Self {
            lines,
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            filename: String::from(filename),
            modified: false,
        }
    }

    // Draw everything — called after every keypress
    pub fn draw(&self) {
        // Draw text area
        for screen_row in 0..EDITOR_ROWS {
            let file_row = screen_row + self.scroll_offset;
            vga::clear_row(screen_row, COLOR_TEXT);
            if file_row < self.lines.len() {
                let line = &self.lines[file_row];
                for (col, &byte) in line.iter().enumerate() {
                    if col >= BUFFER_WIDTH { break; }
                    vga::write_at(screen_row, col, byte, COLOR_TEXT);
                }
            } else {
                // Empty rows past end of file show a tilde like vim/nano
                vga::write_at(screen_row, 0, b'~', ColorCode::new(Color::DarkGray, Color::Black));
            }
        }

        // Draw status bar
        self.draw_status();

        // Move hardware cursor
        let screen_row = self.cursor_row - self.scroll_offset;
        vga::move_cursor(screen_row, self.cursor_col);
    }

    fn draw_status(&self) {
        vga::clear_row(STATUS_ROW, COLOR_STATUS);
        let modified_str = if self.modified { " [modified]" } else { "" };
        let status = alloc::format!(
            " {} {}  Ln {} Col {} | ^S Save  ^Q Quit",
            self.filename,
            modified_str,
            self.cursor_row + 1,
            self.cursor_col + 1,
        );
        vga::write_str_at(STATUS_ROW, 0, &status, COLOR_STATUS);
    }

    pub fn handle_key(&mut self, key: DecodedKey) -> bool {
        use pc_keyboard::DecodedKey::*;
        use pc_keyboard::KeyCode;

        match key {
            // Ctrl+Q — quit, returns false to signal exit
            Unicode('\x11') => return false,

            // Ctrl+S — save
            Unicode('\x13') => self.save(),

            // Enter
            Unicode('\n') | Unicode('\r') => self.insert_newline(),

            // Backspace
            Unicode('\x08') => self.backspace(),

            // Regular printable character
            Unicode(c) if c as u32 >= 0x20 => self.insert_char(c as u8),

            // Arrow keys
            RawKey(KeyCode::ArrowUp)    => self.move_up(),
            RawKey(KeyCode::ArrowDown)  => self.move_down(),
            RawKey(KeyCode::ArrowLeft)  => self.move_left(),
            RawKey(KeyCode::ArrowRight) => self.move_right(),

            // Home/End
            RawKey(KeyCode::Home) => self.cursor_col = 0,
            RawKey(KeyCode::End)  => {
                let len = self.current_line_len();
                self.cursor_col = len;
            }

            _ => {}
        }

        true // still running
    }

    fn insert_char(&mut self, byte: u8) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        self.lines[row].insert(col, byte);
        self.cursor_col += 1;
        self.modified = true;
    }

    fn backspace(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;

        if col > 0 {
            self.lines[row].remove(col - 1);
            self.cursor_col -= 1;
            self.modified = true;
        } else if row > 0 {
            // At start of line — merge with previous line
            let current = self.lines.remove(row);
            let prev_len = self.lines[row - 1].len();
            self.lines[row - 1].extend_from_slice(&current);
            self.cursor_row -= 1;
            self.cursor_col = prev_len;
            self.modified = true;
        }
    }

    fn insert_newline(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;
        let rest = self.lines[row].split_off(col);
        self.lines.insert(row + 1, rest);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.modified = true;
        self.scroll_if_needed();
    }

    fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.cursor_col.min(self.current_line_len());
            if self.cursor_row < self.scroll_offset {
                self.scroll_offset -= 1;
            }
        }
    }

    fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = self.cursor_col.min(self.current_line_len());
            self.scroll_if_needed();
        }
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.current_line_len();
        }
    }

    fn move_right(&mut self) {
        let len = self.current_line_len();
        if self.cursor_col < len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    fn scroll_if_needed(&mut self) {
        let screen_row = self.cursor_row - self.scroll_offset;
        if screen_row >= EDITOR_ROWS {
            self.scroll_offset += 1;
        }
    }

    fn current_line_len(&self) -> usize {
        self.lines[self.cursor_row].len()
    }

    fn save(&mut self) {
        let mut data: Vec<u8> = Vec::new();
        for (i, line) in self.lines.iter().enumerate() {
            data.extend_from_slice(line);
            if i + 1 < self.lines.len() {
                data.push(b'\n');
            }
        }
        if crate::fs::write_file(&self.filename, &data) {
            self.modified = false;
        }
    }

    pub async fn run(filename: &str) {
        crate::serial_println!("[editor] run started for {}", filename);
        crate::task::executor::SUPPRESS_PROMPT.store(false, Ordering::SeqCst);
        let mut editor = Editor::new(filename);
        let input = InputFocus::acquire();

        crate::vga::clear_screen();
        editor.draw();

        loop {
            crate::task::yield_now().await;

            while let Some(key) = input.poll_key() {
                if !editor.handle_key(key) {
                    drop(input); // release focus before clearing
                    crate::vga::clear_screen();
                    // Print a fresh prompt once
                    crate::print!("> ");
                    return;
                }
                editor.draw();
            }
        }
    }
}