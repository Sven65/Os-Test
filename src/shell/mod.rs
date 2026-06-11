mod commands;
mod flags;
pub mod history;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::str;
use core::sync::atomic::Ordering;
use crate::{print, println, serial_println};
use crate::interrupts::TICKS;

const SHELL_PROMPT: &str = "> ";

pub fn prompt() {
	print!("{}", SHELL_PROMPT);
}

fn parse_command_line(input: &[u8]) -> Vec<String> {
	let mut result = Vec::new();
	let mut start = 0;
	for (i, &byte) in input.iter().enumerate() {
		if byte == b' ' {
			if start != i {
				if let Ok(slice) = str::from_utf8(&input[start..i]) {
					result.push(slice.to_string());
				}
			}
			start = i + 1;
		}
	}
	if start < input.len() {
		if let Ok(slice) = str::from_utf8(&input[start..]) {
			result.push(slice.to_string());
		}
	}
	result
}

pub fn pass_to_shell(v: Vec<u8>) {
	let parsed = parse_command_line(&v);
	if parsed.is_empty() { return; } // remove the prompt() call here

	let args = parsed[1..].to_vec();

	match commands::find_command(parsed[0].as_str()) {
		Some(cmd) => cmd.execute(&args),
		None => println!("Unknown command: {}", parsed[0]),
	}

	//print!("\n");
}