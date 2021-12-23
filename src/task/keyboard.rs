use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use core::{pin::Pin, task::{Poll, Context}};
use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;
use alloc::vec::Vec;

use crate::println;
use crate::shell::{pass_to_shell, prompt};


use futures_util::stream::StreamExt;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use crate::print;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

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
                        //print!("{:?}", key),
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
