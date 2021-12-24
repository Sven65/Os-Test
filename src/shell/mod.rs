use alloc::vec::Vec;
use alloc::string::String;
use core::mem::transmute;
use crate::{print, println, exit_qemu, QemuExitCode};

use crate::time::get_time;

use oorandom::Rand32;
const SHELL_PROMPT: &str = "TestOS > ";

pub fn prompt() {
	print!("{}", SHELL_PROMPT);
}

pub fn pass_to_shell(v: Vec<u8>) {	
	match &*v {
		b"help" => print!("This is a help command"),
		b"rand" => {
			let mut rng = Rand32::new(123);
			let n: i32 = rng.rand_i32();

			print!("Random number is {}", n);
		},
		b"time" => {
			let time = get_time();

			print!("Current time is {}", time);
			
		},
		_ => {},
	}

	print!("\n");
	prompt();
}