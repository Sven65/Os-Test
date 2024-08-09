use core::ptr;

use x86_64::structures::paging::{FrameAllocator, Mapper, Size4KiB};
use byteorder::{BigEndian, ByteOrder};


use crate::{device::virtq::initialize_virtqueue, println, serial_println};

use super::pci_config_read;

const VIRTIO_PCI_STATUS: u8 = 0x12;
const VIRTIO_PCI_DEVICE_FEATURES: u8 = 0x10;

const VIRTIO_PCI_QUEUE_SIZE: u8 = 0x12;
const VIRTIO_PCI_QUEUE_SEL: u8 = 0x14;

const COMMAND_BUFFER_OFFSET: u64 = 0x00; // Example, adjust as needed
const RESPONSE_OFFSET: u64 = 0x10;

// Constants for Virtio SCSI
const VIRTIO_PCI_QUEUE_NOTIFY: u64 = 0x50; // Adjust as needed

const BLOCK_LENGTH_OFFSET: u64 = 0x28;



pub fn find_scsi_controller() -> Option<(u8, u8, u8, u64)> {
    for bus in 0..=255 {
        for slot in 0..32 {
            let vendor_device_id = pci_config_read(bus, slot, 0, 0);
            if vendor_device_id != 0xFFFFFFFF {
                let class_code = (pci_config_read(bus, slot, 0, 8) >> 24) as u8;
                if class_code == 0x01 { // Mass Storage Controller
                    let subclass = (pci_config_read(bus, slot, 0, 8) >> 16) as u8;
                    if subclass == 0x00 { // SCSI
                        for bar_index in (0x10..=0x24).step_by(4) {
                            let bar = pci_config_read(bus, slot, 0, bar_index);
                            if bar != 0 && bar != 0xFFFFFFFF {
                                let base_addr = (bar & 0xFFFFFFF0) as u64;
                                // Check if the BAR is 64-bit (if lower bits are 1, it's a 64-bit BAR)
                                if (bar & 0x06) == 0x04 {
                                    let bar_upper = pci_config_read(bus, slot, 0, bar_index + 4);
                                    return Some((bus, slot, 0, base_addr | ((bar_upper as u64) << 32)));
                                } else {
                                    return Some((bus, slot, 0, base_addr));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn wait_for_command_completion(base_addr: u64) {
    const STATUS_REGISTER_OFFSET: u64 = 0x00; // Adjust as needed
    const STATUS_READY: u8 = 0x01; // Example flag indicating completion

    loop {
        let status = unsafe { 
            ptr::read_volatile((base_addr + STATUS_REGISTER_OFFSET) as *const u8) 
        };

        if status & STATUS_READY != 0 {
            break;
        }
    }
}

pub fn get_block_info(base_addr: u64) -> (u64, u64) {
    // Send SCSI READ CAPACITY command
    let command = [0x25, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let command_ptr = (base_addr + COMMAND_BUFFER_OFFSET) as *mut u8;
    unsafe {
        core::ptr::copy_nonoverlapping(command.as_ptr(), command_ptr, command.len());
        // Set other command bytes as needed
    }

     // Notify the device about the new command
     unsafe {
        core::ptr::write_volatile((base_addr + VIRTIO_PCI_QUEUE_NOTIFY) as *mut u8, 0x01);
    }


    // Wait for command completion
    wait_for_command_completion(base_addr);

    // ...


    // // Read the response (8 bytes)
    let response = unsafe {
        let response_ptr = (base_addr + RESPONSE_OFFSET) as *const [u8; 8];
        *response_ptr
    };
    

    serial_println!("Response is {:#?}", response);

    // Manually extract values from the response
    let num_blocks = BigEndian::read_u32(&response[0..4]) as u64;
    let block_size = BigEndian::read_u32(&response[4..8]) as u64;

    serial_println!("Block size {}", block_size);
    serial_println!("Num blocks {}", num_blocks);

    serial_println!("Total size: {}", num_blocks * block_size);

    (block_size, num_blocks)

}

pub fn initialize_virtio_scsi(
    base_addr: u64,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> bool {
    // let base_addr = pci_config_read(bus, slot, func, 0x10) & 0xFFFFFFF0;
    if base_addr == 0 {
        serial_println!("Invalid base address\n");
        return false;
    }

    unsafe {
        println!("[SCSI] Negotiating features");
        let device_features = ptr::read_volatile((base_addr + VIRTIO_PCI_DEVICE_FEATURES as u64) as *mut u32);

        serial_println!("device features {}", device_features);

        let driver_features = device_features; // Negotiate all features for simplicity

        serial_println!("driver features {}", driver_features);


        ptr::write_volatile((base_addr + VIRTIO_PCI_DEVICE_FEATURES as u64) as *mut u32, driver_features);

        println!("[SCSI] Resetting device");
        ptr::write_volatile((base_addr + VIRTIO_PCI_STATUS as u64) as *mut u8, 0x00); // Reset

        println!("[SCSI] Setting ACKNOWLEDGE status");
        ptr::write_volatile((base_addr + VIRTIO_PCI_STATUS as u64) as *mut u8, 0x01); // ACKNOWLEDGE

        println!("[SCSI] Setting DRIVER status");
        ptr::write_volatile((base_addr + VIRTIO_PCI_STATUS as u64) as *mut u8, 0x02); // DRIVER

        println!("[SCSI] Reading queue size");
        ptr::write_volatile((base_addr + VIRTIO_PCI_QUEUE_SEL as u64) as *mut u16, 0);
        let queue_size = ptr::read_volatile((base_addr + VIRTIO_PCI_QUEUE_SIZE as u64) as *mut u16) as usize;

        serial_println!("Queue size: {}", queue_size);

        if queue_size == 0 {
            serial_println!("Invalid queue size\n");
            return false;
        }

        println!("[SCSI] Allocating and initializing virtqueue");
        // Allocation and initialization of the virtqueue omitted for simplicity

        println!("[SCSI] Setting DRIVER_OK status");
        ptr::write_volatile((base_addr + VIRTIO_PCI_STATUS as u64) as *mut u8, 0x04); // DRIVER_OK

        // Get block size

      
        // Initialize the virtqueue
        let queue_index = 0;
        println!("[SCSI] Initializing virtqueue");
        initialize_virtqueue(base_addr, queue_index, queue_size, mapper, frame_allocator);
        println!("[SCSI] Virtqueue initialized, OK to go.");

        let shit = ptr::read_volatile((base_addr + BLOCK_LENGTH_OFFSET as u64) as *mut u16);

        serial_println!("shit {}", shit);


        true
    }
}
