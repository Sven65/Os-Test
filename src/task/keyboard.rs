use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use core::{pin::Pin, task::{Poll, Context}};
use futures_util::{
    stream::Stream,
    task::AtomicWaker,
};
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

use crate::println;
use crate::shell::{pass_to_shell, prompt};


use futures_util::stream::StreamExt;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use crate::print;
use crate::serial_println;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

//let mut KEYBOARD_HOOKS: Vec<fn() -> !> = Vec::new();

pub struct KeyboardHooks {
    hooks: Vec<fn()>,
}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

impl KeyboardHooks {
    pub fn new() -> Self {
        KeyboardHooks {
            hooks: Vec::new()
        }
    }
}

lazy_static! {
    pub static ref KEYBOARD_HOOKS: Mutex<KeyboardHooks> = Mutex::new(KeyboardHooks {
        hooks: Vec::new(),
    });
}


impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
		let queue = SCANCODE_QUEUE
			.try_get()
			.expect("scancode queue not initialized");

		// fast path
		if let Ok(scancode) = queue.pop() {
			return Poll::Ready(Some(scancode));
		}

		WAKER.register(&cx.waker());
		match queue.pop() {
			Ok(scancode) => {
				WAKER.take();
				Poll::Ready(Some(scancode))
			}
			Err(crossbeam_queue::PopError) => Poll::Pending,
		}
    }
}

/// Called by the keyboard interrupt handler
///
/// Must not block or allocate.
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
			WAKER.wake();
		}
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);
    let mut buff: Vec<u8> = Vec::new();

    prompt();

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => {
                        if character == '\x08' {
                            // Backspace
                            if !(buff.len() == 0) {
                                print!("\x7f");
                                //print!("bkspc");
                                // TODO: Remove from buffer

                                buff = rmvec(buff);
                            }
                        } else if character == '\n' {
                            print!("\n");
                            pass_to_shell(buff);
                            // Prompt
                            buff = Vec::new();
                        } else {
                            buff.push(character as u8);
                            print!("{}", character);
                        }
                    }
                    DecodedKey::RawKey(_key) => {
                        print!("{:?}", key);
                        serial_println!("key {:#?}", key);

                        serial_println!("HOOKS {:#?}", KEYBOARD_HOOKS.lock().hooks);


                        for f in &*KEYBOARD_HOOKS.lock().hooks {
                            serial_println!("func {:#?}", f);
                            f();
                        }
                    },
                }
            }
        }
    }
}

fn rmvec(v: Vec<u8>) -> Vec<u8> {
    let mut i = 0;
    let mut buf: Vec<u8> = Vec::new();

    while i < v.len() - 1 {

        buf.push(v[i]);
        i += 1;

    }

    buf
}

#[doc(hidden)]
pub fn _register_hook(hook: fn()) {
    // //KEYBOARD_HOOKS.lock().push(hook);

    // KEYBOARD_HOOK_EXECUTOR.spawn(hook);

    // // executor.spawn(Task::new(example_task()));
    // // executor.spawn(Task::new(keyboard::print_keypresses()));
    // KEYBOARD_HOOK_EXECUTOR.run();

    serial_println!("Registering hook {:#?}", hook);

    KEYBOARD_HOOKS.lock().hooks.push(hook);
}

#[macro_export]
macro_rules! register_kb_hook {
    //($item:item) => ($crate::task::keyboard::_register_hook($item));
    ($item:expr) => {
        $crate::serial_println!("Registering a hook");
        $crate::task::keyboard::_register_hook($item);
    }
    
}
