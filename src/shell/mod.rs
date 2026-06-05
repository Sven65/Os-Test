use alloc::vec::Vec;
use alloc::string::{String, ToString};
use crate::allocator::HEAP_KIB;
use crate::device::ahci::{find_ahci_controller, find_sata_devices, read_ahci_memory, AHCI_MEMORY_SIZE};
use crate::device::get_all_devices;
use crate::fs::{read_file, write_file};
use crate::memory::{dump_memory, test_memory_access};
use crate::vga_old::vga_buffer::get_chars;
use crate::{exit_qemu, print, println, reset_color, serial_print, serial_println, QemuExitCode};
use crate::time::get_time;
use crate::util::bitfield::BitField;
use core::str;

use oorandom::Rand32;
// use crate::vga::vga_buffer::Color;

const SHELL_PROMPT: &str = "> ";

pub fn prompt() {
	print!("{}", SHELL_PROMPT);

}

/**
 * Parses a Vec<u8> into Vec<String>, split by a space
 */
fn parse_command_line(input: &Vec<u8>) -> Vec<String> {
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

	let command = parsed[0].clone().into_bytes();
	let args = parsed.clone().split_off(1);
	
	match &*command {
		b"help" => print!("This is a help command"),
		b"devices" => {
			get_all_devices();
		},
		b"raddr" => {
			let addr = &args[0];
			let addr = u64::from_str_radix(addr.as_str(), 16).expect("Failed to convert addr");
			test_memory_access(addr);
		},
		b"ahci" => {
			match find_ahci_controller() {
				Some((_bus, _slot, _function, base_addr)) => {
		
					find_sata_devices(base_addr);
					
				}
				None => {
					println!("No AHCI controller found");
				}
			}
		},
		b"dump" => {
			let dumptype = &args[0];

			match dumptype.as_str() {
				"mem" => {
					dump_memory(0x_4444_4444_0000, HEAP_KIB);
				},
				"ahci" => {
					match find_ahci_controller() {
						Some((_bus, _slot, _function, base_addr)) => {
				
							read_ahci_memory(base_addr, AHCI_MEMORY_SIZE);
							
						}
						None => {
							println!("No AHCI controller found");
						}
					}
				}
				&_ => {}
			}
		},
		b"param" => {
			print!("Got args {:#?}", args);
		},
		b"write" => {
			if args.is_empty() { println!("Usage: write <filename> <contents>"); prompt(); return; }
			let filename = &args[0];
			let contents = args[1..].join(" ");
			if write_file(filename, contents.as_bytes()) {
				println!("Wrote {} bytes to {}", contents.len(), filename);
			} else {
				println!("Failed to write {}", filename);
			}
		},
		b"read" => {
			if args.is_empty() { println!("Usage: read <filename>"); prompt(); return; }
			let filename = &args[0];
			match read_file(filename) {
				Some(data) => {
					let s = core::str::from_utf8(&data).unwrap_or("(not utf8)");
					println!("{}", s);
				}
				None => { println!("Failed to read {}", filename); }
			}
		},
		b"ls" => {
			for (name, is_dir) in crate::fs::list_dir() {
				if is_dir {
					print!("\x1b[32m{} ", name);
				} else {
					print!("{} ", name);
				}
			}
			reset_color!();
		},

		b"mkdir" => {
			if args.is_empty() { println!("Usage: mkdir <dirname>"); prompt(); return; }
			if !crate::fs::create_dir(&args[0]) {
				println!("Failed to create dir");
			}
		},
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