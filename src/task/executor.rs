use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc};
use core::task::Waker;
use crossbeam_queue::ArrayQueue;
use alloc::task::Wake;
use core::sync::atomic::AtomicBool;
use conquer_once::spin::OnceCell;

use core::task::{Context, Poll};

pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}


static SPAWN_QUEUE: OnceCell<ArrayQueue<Task>> = OnceCell::uninit();
pub static SUPPRESS_PROMPT: AtomicBool = AtomicBool::new(false);

pub fn spawn_task(task: Task) {
    SPAWN_QUEUE
        .try_init_once(|| ArrayQueue::new(100))
        .ok();
    SPAWN_QUEUE.try_get().unwrap().push(task).ok();
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(100)),
            waker_cache: BTreeMap::new(),
        }
    }

    pub fn run(&mut self) -> ! {
        SPAWN_QUEUE.try_init_once(|| ArrayQueue::new(100)).ok();

        loop {
            // Drain spawn queue first
            if let Ok(queue) = SPAWN_QUEUE.try_get() {
                while let Some(task) = queue.pop() {
                    self.spawn(task);
                }
            }

            self.run_ready_tasks();

            // Drain again after running tasks, in case tasks spawned more tasks
            if let Ok(queue) = SPAWN_QUEUE.try_get() {
                while let Some(task) = queue.pop() {
                    self.spawn(task);
                }
            }

            self.sleep_if_idle();
        }
    }
	
	pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        self.task_queue.push(task_id).expect("queue full");
    }

	fn sleep_if_idle(&self) {
		use x86_64::instructions::interrupts::{self, enable_and_hlt};

        interrupts::disable();
        if self.task_queue.is_empty() {
            enable_and_hlt();
        } else {
            interrupts::enable();
        }
	}

	fn run_ready_tasks(&mut self) {
        // destructure `self` to avoid borrow checker errors
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // task no longer exists
            };
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cached waker
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
	fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }

    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task_queue full");
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}
