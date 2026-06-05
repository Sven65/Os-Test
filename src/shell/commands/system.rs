use alloc::string::String;
use crate::{println, print};
use crate::device::ahci::{find_ahci_controller, find_sata_devices, read_ahci_memory, AHCI_MEMORY_SIZE};
use crate::device::get_all_devices;
use crate::memory::{dump_memory, test_memory_access};
use crate::allocator::HEAP_KIB;
use super::Command;

pub struct HelpCommand;
impl Command for HelpCommand {
    fn name(&self) -> &'static str { "help" }
    fn description(&self) -> &'static str { "Show available commands" }
    fn execute(&self, _args: &[String]) {
        for cmd in super::get_commands() {
            println!("  {:10} {}", cmd.name(), cmd.description());
        }
    }
}

pub struct DevicesCommand;
impl Command for DevicesCommand {
    fn name(&self) -> &'static str { "devices" }
    fn description(&self) -> &'static str { "List all PCI devices" }
    fn execute(&self, _args: &[String]) {
        get_all_devices();
    }
}

pub struct RaddrCommand;
impl Command for RaddrCommand {
    fn name(&self) -> &'static str { "raddr" }
    fn description(&self) -> &'static str { "Read memory address: raddr <hex_addr>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: raddr <hex_addr>"); return; }
        match u64::from_str_radix(args[0].as_str(), 16) {
            Ok(addr) => { test_memory_access(addr); }
            Err(_) => println!("Invalid address"),
        }
    }
}

pub struct AhciCommand;
impl Command for AhciCommand {
    fn name(&self) -> &'static str { "ahci" }
    fn description(&self) -> &'static str { "Show AHCI devices" }
    fn execute(&self, _args: &[String]) {
        match find_ahci_controller() {
            Some((_bus, _slot, _function, base_addr)) => { find_sata_devices(base_addr); }
            None => println!("No AHCI controller found"),
        }
    }
}

pub struct DumpCommand;
impl Command for DumpCommand {
    fn name(&self) -> &'static str { "dump" }
    fn description(&self) -> &'static str { "Dump memory: dump <mem|ahci>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: dump <mem|ahci>"); return; }
        match args[0].as_str() {
            "mem" => dump_memory(0x_4444_4444_0000, HEAP_KIB),
            "ahci" => match find_ahci_controller() {
                Some((_bus, _slot, _function, base_addr)) => read_ahci_memory(base_addr, AHCI_MEMORY_SIZE),
                None => println!("No AHCI controller found"),
            },
            _ => println!("Unknown dump type: {}", args[0]),
        }
    }
}