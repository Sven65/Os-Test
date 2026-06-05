use core::ptr::read_volatile;

use x86_64::{
    PhysAddr, VirtAddr, structures::paging::{
        FrameAllocator, OffsetPageTable, Page, PageTable, PageTableFlags, PageTableIndex, PhysFrame, Size4KiB
    }
};

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};

use crate::{serial_print, serial_println};

/// A FrameAllocator that always returns `None`.
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

     /// Returns an iterator over the usable frames specified in the memory map.
     fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

/// Initialize a new OffsetPageTable.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

/// Translates the given virtual address to the mapped physical address, or
/// `None` if the address is not mapped.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`.
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr> {
    translate_addr_inner(addr, physical_memory_offset)
}

/// Private function that is called by `translate_addr`.
///
/// This function is safe to limit the scope of `unsafe` because Rust treats
/// the whole body of unsafe functions as an unsafe block. This function must
/// only be reachable through `unsafe fn` from outside of this module.
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr> {
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    // read the active level 4 frame from the CR3 register
    let (level_4_table_frame, _) = Cr3::read();

    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    let mut frame = level_4_table_frame;

    // traverse the multi-level page table
    for &index in &table_indexes {
        // convert the frame into a page table reference
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe {&*table_ptr};

        // read the page table entry and update `frame`
        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    // calculate the physical address by adding the page offset
    Some(frame.start_address() + u64::from(addr.page_offset()))
}

pub fn dump_memory(start_addr: u64, size: usize) {
    let end_addr = start_addr + size as u64;
    let mut addr = start_addr;

    while addr < end_addr {
        unsafe {
            let value = read_volatile(addr as *const u8);
            // Print address and value
            let _ = serial_print!("{:08X}: {:02X} ", addr, value);
        }
        
        addr += 1;

        // Print a new line every 16 bytes for better readability
        if (addr - start_addr) % 16 == 0 {
            let _ = serial_println!("");
        }
    }
}

pub fn test_memory_access(address: u64) -> u32 {
    unsafe {
        let reg_ptr = address as *const u32;
        read_volatile(reg_ptr)
    }
}

pub fn split_and_remap_as_uncached(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    phys_mem_offset: VirtAddr,
    phys_addr: u64,
    num_pages: usize,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    for i in 0..num_pages {
        let phys = PhysAddr::new(phys_addr + (i * 4096) as u64);
        let virt = phys_mem_offset + phys_addr + (i * 4096) as u64;
        let page = Page::<Size4KiB>::containing_address(virt);
        let frame = PhysFrame::<Size4KiB>::containing_address(phys);
        let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_CACHE | Flags::NO_EXECUTE;

        // Walk the page table manually to handle the huge page case
        unsafe {
            // Get the level 4 table
            let (l4_frame, _) = x86_64::registers::control::Cr3::read();
            let l4_table = &mut *(phys_mem_offset + l4_frame.start_address().as_u64()).as_mut_ptr::<PageTable>();
            
            let l4_entry = &mut l4_table[page.p4_index()];
            let l3_table = &mut *(phys_mem_offset + l4_entry.addr().as_u64()).as_mut_ptr::<PageTable>();
            
            let l3_entry = &mut l3_table[page.p3_index()];
            let l2_table = &mut *(phys_mem_offset + l3_entry.addr().as_u64()).as_mut_ptr::<PageTable>();
            
            let l2_entry = &mut l2_table[page.p2_index()];
            
            if l2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                // This is a 2MB huge page — we need to split it
                // Allocate a new level 1 page table
                let new_l1_frame = frame_allocator.allocate_frame()
                    .expect("failed to allocate L1 page table frame");
                
                // Zero the new page table
                let new_l1_table = &mut *(phys_mem_offset + new_l1_frame.start_address().as_u64()).as_mut_ptr::<PageTable>();
                new_l1_table.zero();
                
                // Fill new L1 table with 512 entries mapping the same 2MB region
                let huge_phys_base = l2_entry.addr().as_u64(); // base of the 2MB region
                let orig_flags = l2_entry.flags() & !PageTableFlags::HUGE_PAGE;
                
                for j in 0..512usize {
                    let entry_phys = PhysAddr::new(huge_phys_base + (j * 4096) as u64);
                    let entry_frame = PhysFrame::<Size4KiB>::containing_address(entry_phys);
                    new_l1_table[PageTableIndex::new(j as u16)].set_frame(entry_frame, orig_flags);
                }
                
                // Replace the huge page entry with the new L1 table
                l2_entry.set_frame(new_l1_frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
                
                // Flush TLB for this 2MB region
                x86_64::instructions::tlb::flush_all();
            }
            
            // Now map the specific 4KB page as uncached
            let l2_entry = &mut l2_table[page.p2_index()];
            let l1_table = &mut *(phys_mem_offset + l2_entry.addr().as_u64()).as_mut_ptr::<PageTable>();
            l1_table[page.p1_index()].set_frame(frame, flags);
            x86_64::instructions::tlb::flush(virt);
        }
    }
    serial_println!("[mem] remapped {:#x}+{} pages as uncached", phys_addr, num_pages);
}