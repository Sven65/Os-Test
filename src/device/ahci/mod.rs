use core::ptr::{read_volatile, write_volatile};

use x86_64::{structures::paging::{mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB}, VirtAddr};

use crate::serial_println;

use super::pci_config_read;

pub const AHCI_CONTROLLER_DRIVE_COUNT: usize = 6;
pub const AHCI_CONTROLLER_MEMORY_PER_DRIVE: usize = 4096;
pub const AHCI_MEMORY_SIZE: usize = AHCI_CONTROLLER_MEMORY_PER_DRIVE * AHCI_CONTROLLER_MEMORY_PER_DRIVE;

const PORT_REG_BASE: u64 = 0x1000; // Base offset for port registers
const PORT_SIG_OFFSET: u32 = 0xA0; // Signature register offset
const PORT_CMD_OFFSET: u32 = 0x00; // Command register offset
const PORT_SIG_SATA: u32 = 0x00000101; // SATA signature
const PORT_COMMAND_REGISTER_OFFSET: u32 = 0x18; // Command register offset
const PORT_DETECTED_MASK: u32 = 0x1; // Device detected bit mask

#[derive(Debug)]
pub enum AhciError {
    PciReadError,
    MemoryMappingError,
    RegisterReadError,
    DeviceNotFound,
    InvalidSignature,
    InvalidMemoryAddress,
}


// Scanning the PCI bus for AHCI controllers
pub fn find_ahci_controller() -> Option<(u8, u8, u8, u64)> {
    for bus in 0..255 {
        for slot in 0..32 {
            let vendor_device_id = pci_config_read(bus, slot, 0, 0);
            if vendor_device_id != 0xFFFFFFFF {
                let class_code = (pci_config_read(bus, slot, 0, 8) >> 24) as u8;
                if class_code == 0x01 { // Mass Storage Controller
                    let subclass = (pci_config_read(bus, slot, 0, 8) >> 16) as u8;
                    if subclass == 0x06 { // SATA
                        let prog_if = (pci_config_read(bus, slot, 0, 8) >> 8) as u8;
                        if prog_if == 0x01 { // AHCI
                            let bar5 = pci_config_read(bus, slot, 0, 0x24);
                            return Some((bus, slot, 0, bar5 as u64 & 0xFFFFFFF0));
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn map_ahci_memory(mapper: &mut impl Mapper<Size4KiB>, frame_allocator: &mut impl FrameAllocator<Size4KiB>, ahci_base: u64) -> Result<(), MapToError<Size4KiB>> {
    let start = VirtAddr::new(ahci_base);
    let end = start + AHCI_MEMORY_SIZE as u64;
    let start_page = Page::containing_address(start);
    let end_page = Page::containing_address(end);

    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    Ok(())
}


pub fn read_ahci_memory(base_address: u64, size: usize) {
    let mut addr = base_address;
    let end = base_address + size as u64;

    while addr < end {
        unsafe {
            let value = read_volatile(addr as *const u32); // Read as u32 to align with typical register size
            serial_println!("0x{:X}: 0x{:08X}", addr, value);
        }
        addr += 4; // Increment by 4 bytes since we're reading u32
    }
}


fn ahci_register_read(base_address: u64, offset: u32) -> u32 {
    unsafe {
        let reg_ptr = (base_address + offset as u64) as *const u32;
        read_volatile(reg_ptr)
    }
}

fn ahci_register_write(base_address: u64, offset: u32, value: u32) {
    unsafe {
        let reg_ptr = (base_address + offset as u64) as *mut u32;
        write_volatile(reg_ptr, value);
    }
}

fn read_port_register(base_address: u64, port: usize, offset: u32) -> Result<u32, AhciError> {
    let register_address = base_address + (port as u64 * 0x1000) + offset as u64;

    if register_address < 0x100000000 && register_address > 0x00000000 {
        unsafe {
            let reg_ptr = register_address as *const u32;
            let value = read_volatile(reg_ptr);
            Ok(value)
        }
    } else {
        Err(AhciError::InvalidMemoryAddress)
    }
}

fn is_device_present(base_address: u64, port: usize) -> Result<bool, AhciError> {
    let port_command = read_port_register(base_address, port, PORT_COMMAND_REGISTER_OFFSET)?;

    serial_println!("port command for port {} is {}", port, port_command);

    Ok((port_command & PORT_DETECTED_MASK) != 0)
}

pub fn find_sata_devices(base_address: u64) {
    const MAX_PORTS: usize = AHCI_CONTROLLER_DRIVE_COUNT; // Number of ports (adjust as needed)

    for port in 0..MAX_PORTS {

        match read_port_register(base_address, port, PORT_SIG_OFFSET) {
            Ok(sig) => {
                serial_println!("Sig for port {} is {}", port, sig);
                if sig == PORT_SIG_SATA {        
                    // Optional: Further interrogation to gather more details
                    match read_port_register(base_address, port, PORT_CMD_OFFSET) {
                        Ok(cmd) => {
                            serial_println!("Port {} Command Register: 0x{:X}", port, cmd);
                            match is_device_present(base_address, port) {
                                Ok(present) => { serial_println!("Is device present at port {}? {}", port, present); }
                                Err(e) => { serial_println!("Failed to get device presence at port {}: {:#?}", port, e); }
                            }
                        }
                        Err(e) => { serial_println!("Failed to read port register for cmd {:#?}", e); }
                    }
                    
                } else {
                    serial_println!("No SATA device on port {}", port);
                }
            }
            Err(e) => {
                serial_println!("Failed to read port register {:#?}", e);
            }
        }
    }
}