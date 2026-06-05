use x86_64::{instructions::port::Port, structures::paging::{FrameAllocator, OffsetPageTable, Size4KiB}, PhysAddr, VirtAddr};
use core::ptr;

use crate::serial_println;

use super::pci_config_read;

// Define your AHCI controller structure (example, adjust as needed)
const AHCI_CONTROLLER_BASE: u64 = 0xF0000000; // Example physical address
const AHCI_MEMORY_SIZE: usize = 0x1000; // Example size in bytes

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

#[repr(C)]
pub struct AhciHbaMem {
    // Add relevant fields here based on the AHCI specification
    cap: u32,         // 0x00
    ghc: u32,         // 0x04
    is: u32,          // 0x08
    pi: u32,          // 0x0C
    vs: u32,          // 0x10
    ccc_ctl: u32,     // 0x14
    ccc_pts: u32,     // 0x18
    em_loc: u32,      // 0x1C
    em_ctl: u32,      // 0x20
    cap2: u32,        // 0x24
    bohc: u32,        // 0x28
    _reserved: [u8; 0x74],
    ports: [AhciPort; 32], // 0x100 - Port control registers
}

#[repr(C)]
pub struct AhciPort {
    clb: u32,         // 0x00
    clbu: u32,        // 0x04
    fb: u32,          // 0x08
    fbu: u32,         // 0x0C
    is: u32,          // 0x10
    ie: u32,          // 0x14
    cmd: u32,         // 0x18
    _reserved0: u32,  // 0x1C
    tfd: u32,         // 0x20
    sig: u32,         // 0x24
    ssts: u32,        // 0x28
    sctl: u32,        // 0x2C
    serr: u32,        // 0x30
    sact: u32,        // 0x34
    ci: u32,          // 0x38
    sntf: u32,        // 0x3C
    fbs: u32,         // 0x40
    _reserved1: [u32; 11], // 0x44
    vendor: [u32; 4], // 0x70
}

// fn map_ahci_controller(base_address: u64) -> Option<&'static mut AhciHbaMem> {
//     let size = 0x1100; // Size of AHCI memory-mapped region
//     let phys_addr = PhysAddr::new(base_address);
//     let virt_addr = unsafe {
//         translate_addr(VirtAddr::new(base_address), VirtAddr::new(0xFFFFFFFF_80000000))
//             .expect("Failed to translate virtual address")
//     };

//     serial_println!("Mapping physical address {:#x} to virtual address {:#x}", base_address, virt_addr.as_u64());

//     if let Some(mapped_memory) = map_physical_memory(base_address, size) {
//         unsafe {
//             Some(&mut *(mapped_memory.as_ptr() as *mut AhciHbaMem))
//         }
//     } else {
//         None
//     }
// }


pub unsafe fn init_ahci_controller(
    physical_memory_offset: VirtAddr,
    page_table: &mut OffsetPageTable<'static>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>
) {
    // Map AHCI controller physical memory to virtual memory
    if let Some(ahci_memory) = map_physical_memory(
        AHCI_CONTROLLER_BASE,
        AHCI_MEMORY_SIZE,
        physical_memory_offset,
        page_table,
        frame_allocator
    ) {
        // Cast the memory to a pointer to your AHCI controller structure
        let ahci_controller = unsafe {
            &mut *(ahci_memory.as_mut_ptr() as *mut AhciHbaMem)
        };

        // Initialize AHCI controller (example initialization code)
        // You may need to perform additional setup depending on the AHCI controller's specs

        // Enable AHCI mode
        ahci_controller.ghc |= 1 << 31;

        // Reset controller
        ahci_controller.ghc |= 1 << 0;
        while ahci_controller.ghc & (1 << 0) != 0 {}

        // Enable interrupts
        ahci_controller.is = u32::MAX;
        for port in ahci_controller.ports.iter_mut() {
            port.is = u32::MAX;
            port.ie = 0xFFFFFFFF;
        }
    } else {
        // Handle mapping failure
        panic!("Failed to map AHCI controller memory");
    }
}


pub fn identify_sata_devices(hba_mem: &mut AhciHbaMem) {
    for (i, port) in hba_mem.ports.iter_mut().enumerate() {
        if (hba_mem.pi & (1 << i)) != 0 {
            let ssts = port.ssts;
            let ipm = (ssts >> 8) & 0x0F;
            let det = ssts & 0x0F;
            if det == 0x03 && ipm == 0x01 {
                serial_println!("SATA drive found on port {}", i);
                // Further initialization code for SATA devices
            }
        }
    }
}