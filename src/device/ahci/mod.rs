use core::ptr::{read_volatile, write_volatile};

use x86_64::{structures::paging::{mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB}, VirtAddr};

use crate::serial_println;

use super::pci_config_read;

// pub const AHCI_CONTROLLER_DRIVE_COUNT: usize = 6;
// pub const AHCI_CONTROLLER_MEMORY_PER_DRIVE: usize = 0x1000;
// const ADDITIONAL_SPACE: usize = 0x1000; // Example additional space
// pub const AHCI_MEMORY_SIZE: usize = AHCI_CONTROLLER_MEMORY_PER_DRIVE * AHCI_CONTROLLER_MEMORY_PER_DRIVE;

pub const AHCI_CONTROLLER_DRIVE_COUNT: usize = 6;
pub const AHCI_CONTROLLER_MEMORY_PER_DRIVE: usize = 4096;
pub const AHCI_MEMORY_SIZE: usize = AHCI_CONTROLLER_DRIVE_COUNT * AHCI_CONTROLLER_MEMORY_PER_DRIVE; // Adjusted total size for AHCI controller

const PORT_COMMAND_REGISTER_OFFSET: u32 = 0x04; // Command register offset
const PORT_DETECTED_MASK: u32 = 0x1; // Mask for detecting a device

const AHCI_CTRL_OFFSET: u32 = 0x00; // Controller offset
const AHCI_IS_OFFSET: u32 = 0x08; // Interrupt Status
const AHCI_PI_OFFSET: u32 = 0x0C; // Port Implementation

const AHCI_PI_MASK: u32 = 0x0000003F; // Port Implemented Mask


const DELAY_COUNT: u64 = 10000000000;

const AHCI_CAP_OFFSET: u64 = 0x00; // Capability register offset
const AHCI_GHC_OFFSET: u64 = 0x04; // Global Host Control register offset
const AHCI_GHC_AE: u32 = 0x0001; // AHCI Enable bit
const AHCI_GHC_HR: u32 = 0x8000; // Host Reset bit

const PORT_REG_BASE: u64 = 0x1000; // Base offset for port registers
const PORT_SIG_OFFSET: u32 = 0xA0; // Signature register offset
const PORT_CMD_OFFSET: u32 = 0x08; // Command register offset
const PORT_CMD_ST: u32 = 0x0001; // Start bit in command register
const PORT_SIG_SATA: u32 = 0x00000101; // SATA signature (adjust as needed)

const ATA_FLAG_SATA: u32 = 1 << 1;
const ATA_FLAG_PIO_DMA: u32 = 1 << 7;
const ATA_FLAG_ACPI_SATA: u32 = 1 << 17;
const ATA_FLAG_AN: u32 = 1 << 18;

const AHCI_FLAG_COMMON: u32 = ATA_FLAG_SATA | ATA_FLAG_PIO_DMA | ATA_FLAG_ACPI_SATA | ATA_FLAG_AN;

const ATA_PIO4: u32 = (1 << 4);
const ATA_UDMA6: u32 = (1 << 6);

const AHCI_HFLAG_INTEL_PCS_QUIRK: u32 = 1 << 28;

#[derive(Debug)]
pub struct AhciController {
    base_address: u64,
    hflags: u32,
    flags: u32,
    pio_mask: u32,
    udma_mask: u32,
}

impl AhciController {
    pub fn new(base_address: u64, hflags: u32, flags: u32, pio_mask: u32, udma_mask: u32) -> Self {
        Self {
            base_address,
            hflags,
            flags,
            pio_mask,
            udma_mask,
        }
    }

    pub fn initialize(&self) -> Result<(), AhciError> {
        let base_address = self.base_address;

        serial_println!("Initializing AHCI controller at base address: 0x{:X}", base_address);

        // Read and print the CAP register
        let cap = ahci_register_read(base_address, AHCI_CAP_OFFSET)?;
        serial_println!("CAP register: 0x{:08X}", cap);

        // Read the Global Host Control register
        let mut ghc = ahci_register_read(base_address, AHCI_GHC_OFFSET)?;
        serial_println!("GHC before reset: 0x{:08X}", ghc);

        // Set the AHCI Enable bit and reset the controller
        ghc |= AHCI_GHC_AE | AHCI_GHC_HR;
        ahci_register_write(base_address, AHCI_GHC_OFFSET, ghc)?;

        // Wait for the reset bit to clear
        serial_println!("Waiting for reset...");
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 10000; // Maximum number of attempts

        while attempts < MAX_ATTEMPTS {
            ghc = ahci_register_read(base_address, AHCI_GHC_OFFSET)?;
            if (ghc & AHCI_GHC_HR) == 0 {
                serial_println!("Reset completed.");
                break;
            }

            busy_wait(1000); // Delay between checks
            attempts += 1;
        }

        if attempts == MAX_ATTEMPTS {
            serial_println!("Reset did not complete.");
            return Err(AhciError::RegisterReadError); // Reset did not complete
        }

        serial_println!("GHC after reset: 0x{:08X}", ahci_register_read(base_address, AHCI_GHC_OFFSET)?);

        // Apply Intel PCS Quirk if necessary
        if self.hflags & AHCI_HFLAG_INTEL_PCS_QUIRK != 0 {
            self.apply_intel_pcs_quirk()?;
        }

        Ok(())
    }

    fn apply_intel_pcs_quirk(&self) -> Result<(), AhciError> {
        // Assuming the PCS register is at offset 0x92 in PCI configuration space
        const PCS_REGISTER_OFFSET: u64 = 0x92;
        let pcs = ahci_register_read(self.base_address, PCS_REGISTER_OFFSET)?;
        
        // Perform the necessary quirk adjustments
        // This might involve setting or clearing certain bits in the PCS register
        // For now, let's just print the value
        serial_println!("PCS register: 0x{:08X}", pcs);

        // Write back the modified PCS register if necessary
        // ahci_register_write(self.base_address, PCS_REGISTER_OFFSET, modified_pcs)?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum AhciError {
    PciReadError,
    MemoryMappingError,
    RegisterReadError,
    DeviceNotFound,
    InvalidSignature,
    InvalidMemoryAddress,
    PortInitializationFailed,
}

pub fn get_ahci_base_address() -> Option<u64> {
    let pci_base_address = 0xfebf1000; // Use the address from your `info pci` output
    Some(pci_base_address)
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

pub fn map_ahci_memory(mapper: &mut impl Mapper<Size4KiB>, frame_allocator: &mut impl FrameAllocator<Size4KiB>, ahci_base: u64, size: usize) -> Result<(), MapToError<Size4KiB>> {
    let start = VirtAddr::new(ahci_base);
    let end = start + size as u64;
    let start_page = Page::containing_address(start);
    let end_page = Page::containing_address(end);

    serial_println!("Mapping AHCI memory from 0x{:X} to 0x{:X}", start.as_u64(), end.as_u64());

    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        serial_println!("Mapping virtual page 0x{:X} to physical frame 0x{:X}", page.start_address().as_u64(), frame.start_address().as_u64());
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    Ok(())
}

fn interpret_port_signature(sig: u32) -> &'static str {
    match sig {
        0x00000101 => "SATA Device",
        0xEB140101 => "ATAPI Device",
        0x00008000 => "SCSI Device",
        _ => "Unknown or Non-SATA Device",
    }
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

fn read_port_register(base_address: u64, port: usize, offset: u32) -> Result<u32, AhciError> {
    // Calculate the address of the port register
    let register_address = base_address + (port as u64 * 0x1000) + offset as u64;
    
    // Debug print to verify address calculation
    serial_println!("Reading port register at address: 0x{:X}", register_address);
    
    // Ensure the address is within a reasonable range
    if register_address < 0x100000000 && register_address >= base_address {
        unsafe {
            let reg_ptr = register_address as *const u32;
            let value = read_volatile(reg_ptr);
            // Debug print to verify value read
            serial_println!("Read value: 0x{:X}", value);
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

fn ahci_register_read(base_address: u64, offset: u64) -> Result<u32, AhciError> {
    unsafe {
        let reg_ptr = (base_address + offset as u64) as *const u32;
        if reg_ptr.is_null() {
            return Err(AhciError::InvalidMemoryAddress);
        }
        let value = read_volatile(reg_ptr);
        Ok(value)
    }
}

fn ahci_register_write(base_address: u64, offset: u64, value: u32) -> Result<(), AhciError> {
    unsafe {
        let reg_ptr = (base_address + offset as u64) as *mut u32;
        if reg_ptr.is_null() {
            return Err(AhciError::InvalidMemoryAddress);
        }
        write_volatile(reg_ptr, value);
        Ok(())
    }
}

fn busy_wait(count: u64) {
    let mut i = 0;
    while i < count {
        i += 1;
    }
}

pub fn initialize_ahci_controller(base_address: u64) -> Result<AhciController, AhciError> {
    let ahci_controller = AhciController::new(
        base_address,
        AHCI_HFLAG_INTEL_PCS_QUIRK, // Apply the quirk
        AHCI_FLAG_COMMON,
        ATA_PIO4,
        ATA_UDMA6,
    );

    ahci_controller.initialize()?;

    Ok(ahci_controller)
}

pub fn find_sata_devices(base_address: u64) {
    const MAX_PORTS: usize = AHCI_CONTROLLER_DRIVE_COUNT; // Number of ports (adjust as needed)

    for port in 0..MAX_PORTS {

        match read_port_register(base_address, port, PORT_SIG_OFFSET) {
            Ok(sig) => {
                serial_println!("Sig for port {} is {}", port, sig);
                serial_println!("Drive in port {} is {}", port, interpret_port_signature(sig));
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

pub fn find_and_initialize_ahci_controller(mapper: &mut impl Mapper<Size4KiB>, frame_allocator: &mut impl FrameAllocator<Size4KiB>) -> Result<(), AhciError> {
    let controller_info = find_ahci_controller().ok_or(AhciError::DeviceNotFound)?;
    let (bus, slot, _, bar5) = controller_info;
    let ahci_base = bar5 & 0xFFFFFFF0; // Ensure the base address is properly masked

    // Map the AHCI memory
    // map_ahci_memory(mapper, frame_allocator, ahci_base, AHCI_MEMORY_SIZE)?;

    // Initialize the AHCI controller
    let ahci_controller = AhciController::new(
        ahci_base,
        AHCI_HFLAG_INTEL_PCS_QUIRK, // Apply the quirk
        AHCI_FLAG_COMMON,
        ATA_PIO4,
        ATA_UDMA6,
    );

    ahci_controller.initialize()?;

    // Optionally, find SATA devices connected to the controller
    find_sata_devices(ahci_base);

    Ok(())
}
