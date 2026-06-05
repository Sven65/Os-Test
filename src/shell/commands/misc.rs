use alloc::string::String;
use crate::{print, println, serial_print, exit_qemu, reset_color, QemuExitCode};
use crate::time::get_time;
use crate::util::bitfield::BitField;
use crate::vga::get_chars;
use oorandom::Rand32;
use super::Command;

pub struct RandCommand;
impl Command for RandCommand {
    fn name(&self) -> &'static str { "rand" }
    fn description(&self) -> &'static str { "Generate a random number" }
    fn execute(&self, _args: &[String]) {
        let mut rng = Rand32::new(123);
        print!("Random number is {}", rng.rand_i32());
    }
}

pub struct TimeCommand;
impl Command for TimeCommand {
    fn name(&self) -> &'static str { "time" }
    fn description(&self) -> &'static str { "Show current time" }
    fn execute(&self, _args: &[String]) {
        print!("Current time is {}", get_time());
    }
}

pub struct ColorCommand;
impl Command for ColorCommand {
    fn name(&self) -> &'static str { "color" }
    fn description(&self) -> &'static str { "Color test" }
    fn execute(&self, _args: &[String]) {
        serial_print!("Hello there, Serial World!");
        for n in 30..37 { println!("\x1b[{}m{}", n, n); }
        for n in 40..47 { println!("\x1b[{}m{}", n, n); }
        for n1 in 30..37 {
            for n2 in 40..47 { print!("\x1b[{};{}m{};{} ", n1, n2, n1, n2); }
        }
        for n1 in 30..37 {
            for n2 in 40..47 { print!("\x1b[1;{};{}m1;{};{} ", n1, n2, n1, n2); }
        }
        print!("\x1b[33;40m");
        get_chars();

        reset_color!();
    }
}

pub struct BitsCommand;
impl Command for BitsCommand {
    fn name(&self) -> &'static str { "bits" }
    fn description(&self) -> &'static str { "Bitfield test" }
    fn execute(&self, _args: &[String]) {
        let mut bf = BitField::new(16);
        bf.set(0);
        bf.set(14);
        print!("Value: {}", bf.get_value());
    }
}

pub struct ExitCommand;
impl Command for ExitCommand {
    fn name(&self) -> &'static str { "exit" }
    fn description(&self) -> &'static str { "Exit QEMU" }
    fn execute(&self, _args: &[String]) {
        exit_qemu(QemuExitCode::Success);
    }
}