use core::mem::size_of;
use core::ptr;

use alloc::vec::Vec;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::VirtAddr;

use crate::serial_println;

#[repr(C)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
struct VirtqAvail {
    flags: u16,
    idx: u16,
    ring: [u16; 256], // VIRTQ_AVAIL_RING_SIZE
}

#[repr(C)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
struct VirtqUsed {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; 256], // VIRTQ_USED_RING_SIZE
}


const VIRTIO_PCI_CONFIG_OFFSET: u16 = 0x100;
const VIRTIO_PCI_STATUS: u8 = 0x12;
const VIRTIO_PCI_DEVICE_FEATURES: u8 = 0x10;
const VIRTIO_PCI_QUEUE_PFN: u8 = 0x20;
const VIRTIO_PCI_QUEUE_SIZE: u8 = 0x12;
const VIRTIO_PCI_QUEUE_SEL: u8 = 0x14;
const VIRTIO_PCI_QUEUE_NOTIFY: u8 = 0x16;
const VIRTIO_PCI_QUEUE_ADDR: u8 = 0x08;

const VIRTQ_DESC_SIZE: usize = 16; // size_of::<VirtqDesc>() which is 16 bytes
const VIRTQ_AVAIL_RING_SIZE: usize = 256;
const VIRTQ_USED_RING_SIZE: usize = 256;


const PAGE_SIZE: usize = Size4KiB::SIZE as usize; // 4 KiB

// fn allocate_and_map_virtqueue(
//     queue_size: usize,
//     mapper: &mut impl Mapper<Size4KiB>,
//     frame_allocator: &mut impl FrameAllocator<Size4KiB>,
// ) -> (*mut VirtqDesc, *mut VirtqAvail, *mut VirtqUsed) {
//     let total_size = calculate_virtqueue_size(queue_size);
//     serial_println!("Total vqueue size is {}", total_size);
//     let num_pages = (total_size + PAGE_SIZE - 1) / PAGE_SIZE;
//     serial_println!("num_pages is {}", num_pages);


//     let start_frame = frame_allocator.allocate_frame().expect("Failed to allocate frame");
//     let end_frame = start_frame + num_pages as u64 - 1;

//     let frame_range = PhysFrame::range_inclusive(start_frame, end_frame);
//     let start_virt = VirtAddr::new(start_frame.start_address().as_u64());

//     for (i, frame) in frame_range.enumerate() {
//         let page = Page::containing_address(start_virt + i * PAGE_SIZE);
//         let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
//         serial_println!("Mapping page {:?} to frame {:?}", page, frame);
//         unsafe {
//             mapper.map_to(page, frame, flags, frame_allocator)
//                 .expect("Failed to map page")
//                 .flush();
//         }
//     }

//     let virtqueue_mem = start_virt.as_mut_ptr::<u8>();
    
//     let desc = virtqueue_mem as *mut VirtqDesc;
//     let avail = (virtqueue_mem as usize + VIRTQ_DESC_SIZE * queue_size) as *mut VirtqAvail;
//     let used = (virtqueue_mem as usize + VIRTQ_DESC_SIZE * queue_size + core::mem::size_of::<VirtqAvail>() + queue_size * core::mem::size_of::<u16>()) as *mut VirtqUsed;

//     (desc, avail, used)
// }


fn allocate_and_map_virtqueue(
    queue_size: usize,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> (*mut VirtqDesc, *mut VirtqAvail, *mut VirtqUsed) {
    let total_size = calculate_virtqueue_size(queue_size);
    serial_println!("Total vqueue size is {}", total_size);
    let num_pages = (total_size + PAGE_SIZE - 1) / PAGE_SIZE;
    serial_println!("num_pages is {}", num_pages);

    let start_frame = frame_allocator.allocate_frame().expect("Failed to allocate frame");
    let end_frame = start_frame + num_pages as u64 - 1;

    let frame_range = PhysFrame::range_inclusive(start_frame, end_frame);
    let start_virt = VirtAddr::new(start_frame.start_address().as_u64());

    for (i, frame) in frame_range.enumerate() {
        let page = Page::containing_address(start_virt + i * PAGE_SIZE);
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        // Check if page is already mapped (modify this if necessary)
        if !mapper.translate_page(page).is_ok() {
            serial_println!("Mapping page {:?} to frame {:?}", page, frame);
            unsafe {
                mapper.map_to(page, frame, flags, frame_allocator)
                    .expect("Failed to map page")
                    .flush();
            }
        }
    }

    let virtqueue_mem = start_virt.as_mut_ptr::<u8>();

    let desc = virtqueue_mem as *mut VirtqDesc;
    let avail = (virtqueue_mem as usize + VIRTQ_DESC_SIZE * queue_size) as *mut VirtqAvail;
    let used = (virtqueue_mem as usize + VIRTQ_DESC_SIZE * queue_size + core::mem::size_of::<VirtqAvail>() + queue_size * core::mem::size_of::<u16>()) as *mut VirtqUsed;

    (desc, avail, used)
}

fn calculate_virtqueue_size(queue_size: usize) -> usize {
    let desc_size = VIRTQ_DESC_SIZE * queue_size;
    let avail_size = size_of::<VirtqAvail>() + queue_size * size_of::<u16>();
    let used_size = size_of::<VirtqUsed>() + queue_size * size_of::<VirtqUsedElem>();

    // Round up to the next page size
    (desc_size + avail_size + used_size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

pub fn initialize_virtqueue(
    base_addr: u64,
    queue_index: u16,
    queue_size: usize,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    let (desc, avail, used) = allocate_and_map_virtqueue(queue_size, mapper, frame_allocator);

    unsafe {
        // Select the queue
        core::ptr::write_volatile((base_addr + VIRTIO_PCI_QUEUE_SEL as u64) as *mut u16, queue_index);

        // Set the queue address (Page Frame Number)
        let pfn = (desc as usize >> 12) as u32;
        core::ptr::write_volatile((base_addr + VIRTIO_PCI_QUEUE_PFN as u64) as *mut u32, pfn);

        // Set queue size
        core::ptr::write_volatile((base_addr + VIRTIO_PCI_QUEUE_SIZE as u64) as *mut u16, queue_size as u16);

        // Notify the device about the queue
        core::ptr::write_volatile((base_addr + VIRTIO_PCI_QUEUE_NOTIFY as u64) as *mut u16, queue_index);
    }
}