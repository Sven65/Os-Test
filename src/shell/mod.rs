use alloc::vec::Vec;
use alloc::string::String;
use core::mem::transmute;
use crate::{print, println, exit_qemu, QemuExitCode};
//use crate::vga_buffer::write_byte;

const SHELL_PROMPT: &str = "TestOS > ";

pub fn prompt() {
	print!("{}", SHELL_PROMPT);
}

pub fn pass_to_shell(v: Vec<u8>) {
	if v.eq(b"help") {
		print!("This is a help command");
	}

	print!("\n");
	prompt();
}