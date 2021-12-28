use alloc::vec::Vec;
use crate::{print, println, exit_qemu, QemuExitCode, serial_print, serial_println};
use crate::vga_old::vga_buffer::{get_chars};
use crate::time::get_time;
use crate::util::bitfield::BitField;

use oorandom::Rand32;
// use crate::vga::vga_buffer::Color;

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
		b"color" => {
			serial_print!("Hello there, Serial World!");

			//println!("\x1b[1;32;42mHello World");
			for n in 30..37 {
				println!("\x1b[{}m{}", n, n);
			}

			for n in 40..47 {
				println!("\x1b[{}m{}", n, n);
			}

			for n1 in 30..37 {
				for n2 in 40..47 {
					print!("\x1b[{};{}m{};{} ", n1, n2, n1, n2);
				}
			}

			for n1 in 30..37 {
				for n2 in 40..47 {
					print!("\x1b[1;{};{}m1;{};{} ", n1, n2, n1, n2);
				}
			}

			print!("\x1b[33;40m");

			//serial_println!("buf {:#?}", WRITER);

			get_chars();
		},
		b"bits" => {
			let mut bf = BitField::new(16);
			serial_println!("Bit 1: {}", bf.get(1));

			bf.set(0);
			bf.set(14);

			serial_println!("Bit 1: {}", bf.get(1));
			serial_println!("Value: {}", bf.get_value());
		},
		b"exit" => {
			exit_qemu(QemuExitCode::Success);
		}
		_ => {},
	}

	print!("\n");
	prompt();
}