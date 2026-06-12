use alloc::string::String;
use alloc::vec::Vec;
use wasmi::{Caller, Engine, Linker, Module, Store};
use crate::fs::read_file;
use crate::println;
use crate::shell::commands::Command;
use crate::wasm::run;

pub struct RunCommand;
impl Command for RunCommand {
    fn name(&self) -> &'static str { "run" }
    fn description(&self) -> &'static str { "Run a program" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: run <filename>"); return; }
        match read_file(&args[0]) {
            Some(data) => {
                // Execute code

                let prog_args = args.to_vec();
                
                match run(data, prog_args) {
                    Ok(()) => println!("Program completed"),
                    Err(e) => println!("Error during program execution: {}", e),
                }
            },
            None => println!("Failed to read {}", args[0]),
        }
    }
}