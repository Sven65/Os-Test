#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_os::test_runner)]
#![reexport_test_harness_main = "test_main"]


extern crate alloc;

use test_os::device::ahci::{find_ahci_controller, initialize_ahci_controller, map_ahci_memory, AHCI_MEMORY_SIZE};
use test_os::{memory, println, allocator, register_kb_hook, serial_println};
use core::panic::PanicInfo;
use bootloader::{BootInfo, entry_point};

use test_os::memory::BootInfoFrameAllocator;
use x86_64::VirtAddr;

use test_os::task::{Task, keyboard};
use test_os::task::executor::Executor; 


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
    println!("Please wait, booting...");

    test_os::init();

    println!("Please wait, mapping memory...");

    
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    println!("Please wait, checking for AHCI...");

    match find_ahci_controller() {
        Some((bus, slot, function, base_addr)) => {
            let base_addr = 0xfebf1000;
            serial_println!("Found AHCI controller at bus {}, slot {}, function {}. base addr is {}", bus, slot, function, base_addr);

            println!("Please wait, mapping AHCI memory...");


            match map_ahci_memory(&mut mapper, &mut frame_allocator, base_addr, AHCI_MEMORY_SIZE) {
                Ok(_) => {
                    serial_println!("Mapped AHCI memory");
                    match initialize_ahci_controller(base_addr) {
                        Ok(controller) => { serial_println!("Initialized AHCI controller {:#?}", controller); }
                        Err(e) => { serial_println!("Failed to initialize AHCI controller: {:#?}", e); }
                    }
                },
                Err(e) => {
                    serial_println!("Failed to map AHCI memory: {:#?}", e);
                }
            }
        }
        None => {
            println!("No AHCI controller found");
        }
    }

    println!("Please wait, mapping heap...");

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::print_keypresses()));

    register_kb_hook!(|| {
        serial_println!("Hello from hook");
    });

    register_kb_hook!(kb_hook_cb);

    executor.run();


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