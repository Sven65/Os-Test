use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use core::{
    pin::Pin,
    task::{Poll, Context},
    sync::atomic::{AtomicBool, Ordering},
};
use futures_util::{
    stream::{Stream, StreamExt},
    task::AtomicWaker,
};
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
use pc_keyboard::{layouts, DecodedKey, HandleControl, PS2Keyboard, ScancodeSet1};
use pc_keyboard::layouts::AnyLayout;
use crate::{print, println, serial_println};
use crate::shell::{pass_to_shell, prompt};

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

static CTRLC_FLAG: AtomicBool = AtomicBool::new(false);
static CTRL_HELD: AtomicBool = AtomicBool::new(false);

pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if queue.push(scancode).is_err() {
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            WAKER.wake();
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
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

        if let Ok(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Ok(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            Err(_) => Poll::Pending,
        }
    }
}

static FOCUSED_INPUT: OnceCell<ArrayQueue<DecodedKey>> = OnceCell::uninit();
pub static HAS_FOCUS: AtomicBool = AtomicBool::new(false);

pub struct InputFocus {
    queue: &'static ArrayQueue<DecodedKey>,
}

impl InputFocus {
    pub fn acquire() -> Self {
        FOCUSED_INPUT.try_init_once(|| ArrayQueue::new(100)).ok();
        HAS_FOCUS.store(true, Ordering::SeqCst);
        InputFocus {
            queue: FOCUSED_INPUT.try_get().unwrap(),
        }
    }

    pub fn next_key(&self) -> DecodedKey {
        loop {
            if let Ok(key) = self.queue.pop() {
                return key;
            }
            core::hint::spin_loop();
        }
    }

    pub fn poll_key(&self) -> Option<DecodedKey> {
        self.queue.pop().ok()
    }
}

impl Drop for InputFocus {
    fn drop(&mut self) {
        HAS_FOCUS.store(false, Ordering::SeqCst);
    }
}

pub struct KeyboardHooks {
    hooks: Vec<fn()>,
}

lazy_static! {
    pub static ref KEYBOARD_HOOKS: Mutex<KeyboardHooks> = Mutex::new(KeyboardHooks {
        hooks: Vec::new(),
    });
}

pub fn _register_hook(hook: fn()) {
    serial_println!("Registering hook {:#?}", hook);
    KEYBOARD_HOOKS.lock().hooks.push(hook);
}

#[macro_export]
macro_rules! register_kb_hook {
    ($item:expr) => {
        $crate::serial_println!("Registering a hook");
        $crate::task::keyboard::_register_hook($item);
    }
}

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();

    let layout_str = crate::CONFIG.lock().keyboard_layout.clone();
    let kb_layout = match layout_str.as_str() {
        "uk"      => AnyLayout::Uk105Key(layouts::Uk105Key),
        "de"      => AnyLayout::De105Key(layouts::De105Key),
        "azerty"  => AnyLayout::Azerty(layouts::Azerty),
        "dvorak"  => AnyLayout::Dvorak104Key(layouts::Dvorak104Key),
        "colemak" => AnyLayout::Colemak(layouts::Colemak),
        "dvp"     => AnyLayout::DVP104Key(layouts::DVP104Key),
        "fise"    => AnyLayout::FiSe105Key(layouts::FiSe105Key),
        "no"      => AnyLayout::No105Key(layouts::No105Key),
        "jis"     => AnyLayout::Jis109Key(layouts::Jis109Key),
        _         => AnyLayout::Us104Key(layouts::Us104Key),
    };
    let mut keyboard = PS2Keyboard::new(ScancodeSet1::new(), kb_layout, HandleControl::MapLettersToUnicode);

    let mut buff: Vec<u8> = Vec::new();

    prompt();

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                if HAS_FOCUS.load(Ordering::SeqCst) {
                    if let Ok(q) = FOCUSED_INPUT.try_get() {
                        q.push(key).ok();
                    }
                    continue;
                }

                match key {
                    DecodedKey::Unicode(character) => {
                        match character {
                            '\x08' => {
                                if !buff.is_empty() {
                                    print!("\x7f");
                                    buff.pop();
                                }
                            }
                            '\x03' => {
                                // Ctrl+C — interrupt current operation
                                crate::task::executor::SUPPRESS_PROMPT.store(false, Ordering::SeqCst);
                                if HAS_FOCUS.load(Ordering::SeqCst) {
                                    // Signal focused program to stop
                                    if let Ok(q) = FOCUSED_INPUT.try_get() {
                                        q.push(DecodedKey::Unicode('\x03')).ok();
                                    }
                                } else {
                                    // Cancel current shell input
                                    buff.clear();
                                    print!("^C\n");
                                    prompt();
                                }
                            }
                            '\n' | '\r' => {
                                print!("\n");
                                pass_to_shell(buff);
                                buff = Vec::new();
                                if !crate::task::executor::SUPPRESS_PROMPT.load(Ordering::SeqCst)
                                    && !HAS_FOCUS.load(Ordering::SeqCst) {
                                    prompt();
                                }
                            }
                            c => {
                                // Properly encode multi-byte chars into UTF-8 bytes
                                let mut bytes = [0u8; 4];
                                let s = c.encode_utf8(&mut bytes);
                                for b in s.bytes() {
                                    buff.push(b);
                                }
                                print!("{}", c);
                            }
                        }
                    }
                    DecodedKey::RawKey(_) => {
                        for f in &*KEYBOARD_HOOKS.lock().hooks {
                            f();
                        }
                    }
                }
            }
        }
    }
}


pub fn process_pending_scancodes() {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        while let Ok(scancode) = queue.pop() {
            if scancode == 0x1d {
                CTRL_HELD.store(true, Ordering::SeqCst);
            } else if scancode == 0x9d {
                CTRL_HELD.store(false, Ordering::SeqCst);
            } else if scancode == 0x2e && CTRL_HELD.load(Ordering::SeqCst) {
                CTRLC_FLAG.store(true, Ordering::SeqCst);
            }
        }
    }
}

pub fn check_ctrlc() -> bool {
    CTRLC_FLAG.load(Ordering::SeqCst)
}

pub fn clear_ctrlc() {
    CTRLC_FLAG.store(false, Ordering::SeqCst);
}