use core::alloc::Layout;
use core::ptr::NonNull;
use virtio_drivers::{BufferDirection, Hal, PhysAddr, PAGE_SIZE};
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr as X86PhysAddr, VirtAddr};

use x86_64::structures::paging::mapper::TranslateResult;
use x86_64::structures::paging::{OffsetPageTable, PageTable, Translate};
use x86_64::registers::control::Cr3;

use spin::Mutex;

use crate::serial_println;

/// This is the "HAL" (Hardware Abstraction Layer) that virtio-drivers
/// needs from us. It has two jobs:
///   1. Allocate physical memory pages for DMA (so the device can read/write them)
///   2. Tell virtio-drivers the physical address of a virtual pointer
///
/// Because we use bootloader's map_physical_memory feature, our physical
/// memory is mapped 1:1 starting at phys_mem_offset. We store that offset
/// here so we can convert between physical and virtual addresses.
pub struct OsHal;

/// Physical memory offset — set once during init, read by the HAL.
/// This is the virtual address where physical address 0 is mapped.
static mut PHYS_MEM_OFFSET: u64 = 0;

static DMA_ALLOC_COUNT: core::sync::atomic::AtomicUsize = 
    core::sync::atomic::AtomicUsize::new(0);

const DMA_POOL_SIZE: usize = 64 * 1024; // 64KB, enough for a few virtqueues
static DMA_POOL: Mutex<DmaBumpAlloc> = Mutex::new(DmaBumpAlloc::new());

struct DmaBumpAlloc {
    pool: [u8; DMA_POOL_SIZE],
    offset: usize,
}

impl DmaBumpAlloc {
    const fn new() -> Self {
        Self { pool: [0u8; DMA_POOL_SIZE], offset: 0 }
    }

    fn alloc(&mut self, size: usize, align: usize) -> *mut u8 {
        let aligned = (self.offset + align - 1) & !(align - 1);
        assert!(aligned + size <= DMA_POOL_SIZE, "DMA pool exhausted");
        self.offset = aligned + size;
        &mut self.pool[aligned] as *mut u8
    }
}

/// Call this during kernel init after you set up the memory mapper.
pub fn init_hal(phys_mem_offset: u64) {
    unsafe {
        PHYS_MEM_OFFSET = phys_mem_offset;
    }
    serial_println!("[HAL] init_hal: phys_mem_offset={:#x}", phys_mem_offset);
}

fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    VirtAddr::new(unsafe { PHYS_MEM_OFFSET } + paddr as u64)
}

fn virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    let offset = unsafe { PHYS_MEM_OFFSET };
    serial_println!("[HAL] virt_to_phys: vaddr={:#x} offset={:#x}", vaddr.as_u64(), offset);
    (vaddr.as_u64() - offset) as PhysAddr
}


unsafe fn virt_to_phys_mapped(vaddr: VirtAddr) -> PhysAddr {
    let offset = unsafe { PHYS_MEM_OFFSET };

    // Always use page table walk for correctness
    // The simple subtraction only works for addresses in the direct physical
    // memory map region (PHYS_MEM_OFFSET to PHYS_MEM_OFFSET + RAM_SIZE)
    // Heap, stack, and other specially mapped regions need the page table walk
    use x86_64::structures::paging::{OffsetPageTable, Translate};
    use x86_64::structures::paging::mapper::TranslateResult;
    use x86_64::registers::control::Cr3;

    let (frame, _) = Cr3::read();
    let phys = frame.start_address();
    let virt = VirtAddr::new(offset + phys.as_u64());
    let page_table = &mut *(virt.as_mut_ptr::<PageTable>());
    let mut mapper = OffsetPageTable::new(page_table, VirtAddr::new(offset));

    match mapper.translate(vaddr) {
        TranslateResult::Mapped { frame, offset, flags: _ } => {
            (frame.start_address().as_u64() + offset) as PhysAddr
        }
        _ => panic!("virt_to_phys: address {:#x} not mapped", vaddr.as_u64()),
    }
}

pub fn print_dma_pool_addr() {
    serial_println!("[HAL] DMA pool addr: virt={:#x}", &DMA_POOL as *const _ as u64);
}

unsafe impl Hal for OsHal {
    /// Allocate `pages` pages of physically contiguous memory.
    /// Returns the physical address (what the device sees) and a
    /// non-null pointer to the virtual address (what we see).
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let size = pages * PAGE_SIZE;
        let ptr = DMA_POOL.lock().alloc(size, PAGE_SIZE);
        let vaddr = VirtAddr::from_ptr(ptr);
        let paddr = unsafe { virt_to_phys_mapped(vaddr) };
        serial_println!("[HAL] dma_alloc: virt={:#x} phys={:#x} pages={}", 
            vaddr.as_u64(), paddr, pages);
        (paddr, NonNull::new(ptr).unwrap())
    }

    /// Free memory previously allocated with dma_alloc.
    unsafe fn dma_dealloc(paddr: PhysAddr, _vaddr: NonNull<u8>, pages: usize) -> i32 {
        let vaddr = phys_to_virt(paddr);
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        alloc::alloc::dealloc(vaddr.as_mut_ptr(), layout);
        0 // 0 = success
    }

    /// Convert a physical address to a virtual address.
    /// virtio-drivers calls this when it needs to read memory the device wrote.
    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, size: usize) -> NonNull<u8> {
        let vaddr = phys_to_virt(paddr);
        serial_println!("[HAL] mmio_phys_to_virt: phys={:#x} size={:#x} virt={:#x}", paddr, size, vaddr.as_u64());
        NonNull::new(vaddr.as_mut_ptr()).unwrap()
    }

    /// Convert a virtual address to a physical address.
    /// virtio-drivers calls this when it needs to tell the device where to write.
    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        let vaddr = VirtAddr::from_ptr(buffer.as_ptr() as *const u8);
        let paddr = unsafe { virt_to_phys_mapped(vaddr) };
        serial_println!("[HAL] share: virt={:#x} phys={:#x} dir={:?}", vaddr.as_u64(), paddr, _direction);
        paddr
    }

    /// Called when the device is done with a shared buffer. Nothing to do for us.
    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {}
}


pub fn mark_dma_pool_uncached(phys_mem_offset: u64) {
    let pool_addr = unsafe { &DMA_POOL as *const _ as u64 };
    let pool_size = core::mem::size_of::<DmaBumpAlloc>();
    let num_pages = (pool_size + 4095) / 4096;

    for i in 0..num_pages {
        let virt = VirtAddr::new(pool_addr + (i * 4096) as u64);
        let page = Page::<Size4KiB>::containing_address(virt);

        let (frame, _) = Cr3::read();
        let pt_virt = VirtAddr::new(phys_mem_offset + frame.start_address().as_u64());
        let page_table = unsafe { &mut *(pt_virt.as_mut_ptr::<x86_64::structures::paging::PageTable>()) };

        use x86_64::structures::paging::{OffsetPageTable, Mapper};
        let mut mapper = unsafe { OffsetPageTable::new(page_table, VirtAddr::new(phys_mem_offset)) };

        unsafe {
            mapper.update_flags(page, 
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE
            ).expect("failed to mark DMA pool uncached").flush();
        }
    }
    serial_println!("[HAL] DMA pool marked uncached");
}