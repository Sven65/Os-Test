use alloc::string::{String, ToString};
use crate::{println, print};
use crate::device::ahci::{find_ahci_controller, find_sata_devices, read_ahci_memory, AHCI_MEMORY_SIZE};
use crate::device::get_all_devices;
use crate::memory::{dump_memory, test_memory_access};
use crate::allocator::{HEAP_SIZE, HEAP_START};
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
            "mem" => {
                let len = args.get(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(4096)
                    .min(HEAP_SIZE);
                dump_memory(HEAP_START as u64, len);
            }
            "ahci" => match find_ahci_controller() {
                Some((_bus, _slot, _function, base_addr)) => read_ahci_memory(base_addr, AHCI_MEMORY_SIZE),
                None => println!("No AHCI controller found"),
            },
            _ => println!("Unknown dump type: {}", args[0]),
        }
    }
}

pub struct ConfigCommand;
impl Command for ConfigCommand {
    fn name(&self) -> &'static str { "config" }
    fn description(&self) -> &'static str { "Get or set config: config <key> [value]" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() {
            let cfg = crate::CONFIG.lock();
            println!("hostname={}", cfg.hostname);
            println!("keyboard_layout={}", cfg.keyboard_layout);
            return;
        }

        let flags = crate::shell::flags::Flags::parse(args);
        let key = match flags.get(0) {
            Some(k) => k.to_string(),
            None => { println!("Usage: config <key> [value]"); return; }
        };

        if flags.args.len() == 1 {
            let cfg = crate::CONFIG.lock();
            match key.as_str() {
                "hostname"         => println!("{}", cfg.hostname),
                "keyboard_layout"  => println!("{}", cfg.keyboard_layout),
                _ => println!("Unknown key: {}", key),
            }
            return;
        }

        let value = flags.args[1..].join(" ");
        {
            let mut cfg = crate::CONFIG.lock();
            match key.as_str() {
                "hostname"        => cfg.hostname = value.clone(),
                "keyboard_layout" => cfg.keyboard_layout = value.clone(),
                _ => { println!("Unknown key: {}", key); return; }
            }
            if !cfg.save() {
                println!("Failed to save config");
                return;
            }
        }
        println!("Saved {} = {}", key, value);
    }
}

pub struct MemInfoCommand;
impl Command for MemInfoCommand {
    fn name(&self) -> &'static str { "meminfo" }
    fn description(&self) -> &'static str { "Show memory usage" }
    fn execute(&self, _args: &[String]) {
        use core::sync::atomic::Ordering;
        let used = crate::allocator::fixed_size_block::ALLOC_BYTES.load(Ordering::Relaxed) as usize;
        let total = crate::allocator::HEAP_SIZE;
        println!(
            "Heap: {} KiB / {} KiB used ({}%)",
            used / 1024,
            total / 1024,
            used * 100 / total,
        );
    }
}