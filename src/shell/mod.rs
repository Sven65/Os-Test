use alloc::vec::{self, Vec};
use alloc::string::{String, ToString};
use crate::fs::{create_filesystem, RamStorage, FILE_SYSTEM};
use crate::{exit_qemu, print, println, reset_color, serial_print, serial_println, QemuExitCode};
use crate::vga_old::vga_buffer::{get_chars};
use crate::time::get_time;
use crate::util::bitfield::BitField;
use core::str;
use fatfs::{File, FileSystem, FsOptions, Read, Write};

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
		b"param" => {
			print!("Got args {:#?}", args);
		},
		b"write" => {
			let filename = &args[0];
			let contents = args.clone().split_off(1);
			let fs = FILE_SYSTEM.lock();

			let root_dir = fs.root_dir();

			let file_result = root_dir.create_file(filename);

			match file_result {
				Err(e) => { println!("Failed to create file: {:#?}", e); }
				Ok(mut file) => {
					let contents = contents.join(" ");
					let contents_u8 = contents.as_bytes();
		
					serial_println!("writing {:#?}", contents_u8);
		
					file.write(contents_u8).expect("Failed to write file");
				}
			}
		}
		b"read" => {
			let filename = &args[0];
			let fs = FILE_SYSTEM.lock();

			let root_dir = fs.root_dir();


			let file_result = root_dir.open_file(filename);

			match file_result {
				Ok(mut file) => {
					let mut buf = Vec::<u8>::new();
					file.read(&mut buf).expect("Failed to read file");
	
					
					let str = core::str::from_utf8(&buf).expect("Failed to create str");
					serial_println!("read file {:#?} = {}", buf, str);
	
					println!("{}", str);
				},
				Err(e) => {
					println!("Failed to read file: {:#?}", e);
				}
			};

		},
		b"ls" => {
			let fs = FILE_SYSTEM.lock();
			let dir = fs.root_dir();
			let files = dir.iter();

			for file in files {
				match file {
					Ok(file) => {
						let name = core::str::from_utf8(file.short_file_name_as_bytes()).expect("Failed to convert name to str");
						if file.is_file() {
							print!("{} ", name);
						} else if file.is_dir() {
							print!("\x1b[32m{} ", name);
						}

						serial_println!("{:#?}", file);
					}
					Err(e) => { serial_println!("Failed to stat item: {:#?}", e); }
				}				
			}

			reset_color!();
		},
		b"mkdir" => {
			let dirname = &args[0];
			let fs = FILE_SYSTEM.lock();

			let root_dir = fs.root_dir();

			let res = root_dir.create_dir(dirname);

			if res.is_err() {
				println!("Failed to create dir: {:#?}", res.err());
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