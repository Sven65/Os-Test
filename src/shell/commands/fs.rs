use alloc::string::String;
use crate::{println, print, serial_println};
use crate::fs::{read_file, write_file, create_dir, list_dir};
use crate::reset_color;
use super::Command;

pub struct WriteCommand;
impl Command for WriteCommand {
    fn name(&self) -> &'static str { "write" }
    fn description(&self) -> &'static str { "Write a file: write <filename> <contents>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: write <filename> <contents>"); return; }
        let contents = args[1..].join(" ");
        if write_file(&args[0], contents.as_bytes()) {
            println!("Wrote {} bytes to {}", contents.len(), args[0]);
        } else {
            println!("Failed to write {}", args[0]);
        }
    }
}

pub struct ReadCommand;
impl Command for ReadCommand {
    fn name(&self) -> &'static str { "read" }
    fn description(&self) -> &'static str { "Read a file: read <filename>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: read <filename>"); return; }
        match read_file(&args[0]) {
            Some(data) => println!("{}", core::str::from_utf8(&data).unwrap_or("(not utf8)")),
            None => println!("Failed to read {}", args[0]),
        }
    }
}

pub struct LsCommand;
impl Command for LsCommand {
    fn name(&self) -> &'static str { "ls" }
    fn description(&self) -> &'static str { "List directory contents" }
    fn execute(&self, args: &[String]) {
        let path = args.first().map(|s| s.as_str()).unwrap_or("");
        for (name, is_dir) in list_dir(path) {
            if is_dir {
                print!("\x1b[32m{} ", name);
            } else {
                print!("{} ", name);
            }
        }
        reset_color!();
    }
}

pub struct MkdirCommand;
impl Command for MkdirCommand {
    fn name(&self) -> &'static str { "mkdir" }
    fn description(&self) -> &'static str { "Create a directory: mkdir <dirname>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: mkdir <dirname>"); return; }
        if !create_dir(&args[0]) {
            println!("Failed to create directory");
        }
    }
}