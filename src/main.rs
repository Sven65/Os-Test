#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_os::test_runner)]
#![reexport_test_harness_main = "test_main"]


extern crate alloc;

use test_os::{memory, println, allocator, register_kb_hook, serial_println, serial_print};
use core::panic::PanicInfo;
use bootloader::{BootInfo, entry_point};

use test_os::memory::BootInfoFrameAllocator;
use x86_64::{structures::paging::Page, VirtAddr};
use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};

use test_os::task::{Task, keyboard};
use test_os::task::executor::Executor; 

use vga::colors::{Color16, TextModeColor};
use vga::writers::{ScreenCharacter, TextWriter, Text80x25};


use test_os::fs::fat::RamFS;

use test_os::fs::vfs::FsFunctions;

entry_point!(kernel_main);

async fn async_number() -> u32 {
    42
}

fn kb_hook_cb() {
    serial_println!("Hello from the other hook");
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("Hello World{}", "!");

    test_os::init();

    // new_print!("Hello World!\nWeed!");
    // new_print!("In publishing and graphic design, Lorem ipsum is a placeholder text commonly used to demonstrate the visual form of a document or a typeface without relying on meaningful content. Lorem ipsum may be used as a placeholder before the final copy is available");

    // for (offset, character) in "Hello World! \x1b[32mA".chars().enumerate() {
    //     serial_println!("Printing char {}", character);

    //     let screen_char = ScreenCharacter::new(character as u8, color);

    //     text_mode.write_character(1 + offset, 0, screen_char);
    // }
    
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    fs_root = initialise_initrd(0);

    // list the contents of /
    int i = 0;
    struct dirent *node = 0;
    while ( (node = readdir_fs(fs_root, i)) != 0) {
        serial_print!("Found file ");
        serial_print!("{}\n", node.name);
        let fsnode = finddir_fs(fs_root, node.name);

        if ((fsnode->flags&0x7) == FS_DIRECTORY)
        {
            serial_print!("\n\t(directory)\n");
        }
        else
        {
            serial_print("\n\t contents: \"");
            char buf[256];
            u32int sz = read_fs(fsnode, 0, 256, buf);
            int j;
            for (j = 0; j < sz; j++) {
                serial_print(buf[j]);
            }
            
            serial_print("\"\n");
        }
        i++;
    }

    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::print_keypresses()));

    register_kb_hook!(|| {
        serial_println!("Hello from hook");
    });

    register_kb_hook!(kb_hook_cb);

    executor.run();


    

    #[cfg(test)]
    test_main();

    println!("No crashes, woo!");
    test_os::hlt_loop();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    test_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_os::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}