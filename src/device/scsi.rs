use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::pci::bus::{Cam};
use virtio_drivers::transport::pci::{bus::PciRoot, PciTransport};
use virtio_drivers::transport::{DeviceType, Transport};

use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};


use crate::serial_println;
use super::virtio_hal::OsHal;

pub fn map_mmconfig(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    mmconfig_base: usize,
) {
    // MMCONFIG is typically 256 buses × 32 devices × 8 functions × 4KB = 256MB
    // But we only need to map enough for the devices we'll actually scan.
    let size = 0x10000000; // 256MB — full ECAM space
    let num_pages = size / 4096;

    for i in 0..num_pages {
        let phys = PhysAddr::new((mmconfig_base + i * 4096) as u64);
        let virt = VirtAddr::new((mmconfig_base + i * 4096) as u64);

        let frame = PhysFrame::<Size4KiB>::containing_address(phys);
        let page = Page::<Size4KiB>::containing_address(virt);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE;

        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)
                .expect("mmconfig map failed")
                .flush();
        }
    }
}

/// Scan PCI, find the VirtIO block device, and return a working VirtIOBlk.
///
/// How PCI scanning works:
///   PCI devices are arranged in buses (0-255), slots (0-31), and functions (0-7).
///   Each device has a config space you can read. The first two fields are
///   vendor ID and device ID — like USB IDs, they identify who made it and what it is.
///
///   VirtIO devices always have vendor ID 0x1AF4 (Red Hat).
///   Device IDs 0x1000-0x107F are VirtIO devices; 0x1042 is virtio-blk specifically.
///   The modern (1.0+) device IDs start at 0x1040 + device type.
///   Device type 2 = block, so 0x1042.
pub fn find_and_init_blk(mmconfig_base: usize) -> Option<VirtIOBlk<OsHal, PciTransport>> {
    let mut pci_root = unsafe { PciRoot::new(mmconfig_base as *mut u8, Cam::Ecam) };

    // 255_u8

    for bus in 0..=255_u8 {
        for (device_function, device_info) in pci_root.enumerate_bus(bus) {
            serial_println!("[PCI] {:02x}:{:02x}.{} vendor={:#06x} device={:#06x}",
                device_function.bus,
                device_function.device,
                device_function.function,
                device_info.vendor_id,
                device_info.device_id,
            );

            // Enable bus mastering before creating transport
            use virtio_drivers::transport::pci::bus::Command;
            let (_status, command) = pci_root.get_status_command(device_function);
            pci_root.set_command(device_function, command | Command::BUS_MASTER | Command::MEMORY_SPACE);

            let Ok(transport) = PciTransport::new::<OsHal>(&mut pci_root, device_function)
            else {
                continue;
            };

            let device_type = transport.device_type();
            if device_type != DeviceType::Block {
                continue;
            }

            match VirtIOBlk::<OsHal, PciTransport>::new(transport) {
                Ok(blk) => {
                    serial_println!("[VirtIO] Block device ready! capacity={} sectors", blk.capacity());
                    return Some(blk);
                }
                Err(e) => {
                    serial_println!("[VirtIO] Failed to init block device: {:?}", e);
                }
            }
        }
    }
    None
}

/// Read a single 512-byte sector from the block device.
///
/// `sector` is the LBA (Logical Block Address) — just a sector number
/// starting from 0. Sector 0 is the MBR / first 512 bytes of the disk.
pub fn read_sector(blk: &mut VirtIOBlk<OsHal, PciTransport>, sector: u64, buf: &mut [u8; 512]) {
    serial_println!("[disk] queue size: {}", blk.virt_queue_size());
    blk.disable_interrupts();
    serial_println!("[disk] about to read...");

    serial_println!("[disk] peek_used before: {:?}", blk.peek_used());
    blk.read_blocks(sector as usize, buf).expect("read failed");
    serial_println!("[disk] peek_used after: {:?}", blk.peek_used());
}

pub fn map_virtio_mmio(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    phys_mem_offset: u64,
) {
    use x86_64::{PhysAddr, VirtAddr};
    use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame, Size4KiB};

    // virtio-pci MMIO region from info mtree: 0xfe000000-0xfe003fff
    let base_phys = 0xfe000000u64;
    let size = 0x4000usize; // 16KB
    let num_pages = size / 4096;

    for i in 0..num_pages {
        let phys = PhysAddr::new(base_phys + (i * 4096) as u64);
        let virt = VirtAddr::new(phys_mem_offset + base_phys + (i * 4096) as u64);
        let frame = PhysFrame::<Size4KiB>::containing_address(phys);
        let page = Page::<Size4KiB>::containing_address(virt);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)
                .expect("virtio mmio map failed")
                .flush();
        }
    }
    serial_println!("[VirtIO] Mapped MMIO at phys={:#x} virt={:#x}", 
        base_phys, phys_mem_offset + base_phys);
}