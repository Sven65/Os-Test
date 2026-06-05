use core::ptr::NonNull;
use virtio_drivers::{BufferDirection, Hal, PhysAddr, PAGE_SIZE};
use x86_64::structures::paging::{Page, PageTableFlags, PageTable, Size4KiB};
use x86_64::VirtAddr;
use x86_64::registers::control::Cr3;
use spin::Mutex;

pub struct OsHal;

static mut PHYS_MEM_OFFSET: u64 = 0;

const DMA_POOL_SIZE: usize = 64 * 1024;
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

pub fn init_hal(phys_mem_offset: u64) {
    unsafe { PHYS_MEM_OFFSET = phys_mem_offset; }
}

fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    VirtAddr::new(unsafe { PHYS_MEM_OFFSET } + paddr as u64)
}

unsafe fn virt_to_phys_mapped(vaddr: VirtAddr) -> PhysAddr {
    use x86_64::structures::paging::{OffsetPageTable, Translate};
    use x86_64::structures::paging::mapper::TranslateResult;

    let offset = unsafe { PHYS_MEM_OFFSET };
    let (frame, _) = Cr3::read();
    let virt = VirtAddr::new(offset + frame.start_address().as_u64());
    let page_table = &mut *(virt.as_mut_ptr::<PageTable>());
    let mapper = OffsetPageTable::new(page_table, VirtAddr::new(offset));

    match mapper.translate(vaddr) {
        TranslateResult::Mapped { frame, offset, flags: _ } => {
            (frame.start_address().as_u64() + offset) as PhysAddr
        }
        _ => panic!("virt_to_phys: address {:#x} not mapped", vaddr.as_u64()),
    }
}

unsafe impl Hal for OsHal {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let size = pages * PAGE_SIZE;
        let ptr = DMA_POOL.lock().alloc(size, PAGE_SIZE);
        let vaddr = VirtAddr::from_ptr(ptr);
        let paddr = unsafe { virt_to_phys_mapped(vaddr) };
        (paddr, NonNull::new(ptr).unwrap())
    }

    unsafe fn dma_dealloc(_paddr: PhysAddr, _vaddr: NonNull<u8>, _pages: usize) -> i32 {
        // Bump allocator — no dealloc needed
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        let vaddr = phys_to_virt(paddr);
        NonNull::new(vaddr.as_mut_ptr()).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        let vaddr = VirtAddr::from_ptr(buffer.as_ptr() as *const u8);
        unsafe { virt_to_phys_mapped(vaddr) }
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {}
}

pub fn mark_dma_pool_uncached(phys_mem_offset: u64) {
    use x86_64::structures::paging::{OffsetPageTable, Mapper};

    let pool_addr = &DMA_POOL as *const _ as u64;
    let pool_size = core::mem::size_of::<DmaBumpAlloc>();
    let num_pages = (pool_size + 4095) / 4096;

    for i in 0..num_pages {
        let virt = VirtAddr::new(pool_addr + (i * 4096) as u64);
        let page = Page::<Size4KiB>::containing_address(virt);
        let (frame, _) = Cr3::read();
        let pt_virt = VirtAddr::new(phys_mem_offset + frame.start_address().as_u64());
        let page_table = unsafe { &mut *(pt_virt.as_mut_ptr::<PageTable>()) };
        let mut mapper = unsafe { OffsetPageTable::new(page_table, VirtAddr::new(phys_mem_offset)) };
        unsafe {
            mapper.update_flags(page,
                                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE
            ).expect("failed to mark DMA pool uncached").flush();
        }
    }
}