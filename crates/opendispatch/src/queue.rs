//! The queue: tasks by priority, and the one poster that asks the loop to run them.
//!
//! Nothing here touches Win32. The platform behind [`Poster`] posts a private
//! window message; the window procedure calls [`Queue::run_pending`], which runs
//! the tasks in priority order. A host without a message loop supplies a fake
//! poster and calls `run_pending` itself, which is how the ordering is tested.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::thread;

/// A unit of work the queue owns until it runs.
pub type Task = Box<dyn FnOnce() + Send + 'static>;

/// What the queue asks the platform to do.
pub trait Poster: Send + Sync {
    /// Ask the owning loop to run the queue. `false` means the post was refused.
    fn post(&self) -> bool;
    /// Ask the owning loop to leave. Called from the shutdown task.
    fn quit(&self);
}

/// The three buckets `DispatcherQueuePriority` maps to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Priority {
    Low,
    Normal,
    High,
}

impl Priority {
    /// Map the ABI value, where the metadata uses -10, 0 and 10.
    pub fn from_i32(value: i32) -> Self {
        match value {
            ..0 => Priority::Low,
            1.. => Priority::High,
            0 => Priority::Normal,
        }
    }

    fn index(self) -> usize {
        match self {
            Priority::Low => 0,
            Priority::Normal => 1,
            Priority::High => 2,
        }
    }
}

struct State {
    tasks: [VecDeque<Task>; 3],
    shutting_down: bool,
    /// Whether a post is outstanding. One post covers any number of tasks.
    posted: bool,
}

/// A queue bound to the thread that created it.
pub struct Queue {
    owner: thread::Id,
    poster: Arc<dyn Poster>,
    state: Mutex<State>,
}

impl Queue {
    pub fn new(owner: thread::Id, poster: Arc<dyn Poster>) -> Arc<Self> {
        Arc::new(Self {
            owner,
            poster,
            state: Mutex::new(State {
                tasks: [VecDeque::new(), VecDeque::new(), VecDeque::new()],
                shutting_down: false,
                posted: false,
            }),
        })
    }

    /// Add work at `priority`. `false` is the queue refusing after shutdown.
    pub fn enqueue(&self, priority: Priority, task: Task) -> bool {
        self.push(priority, task, false)
    }

    /// Add the one task that runs after shutdown has been accepted. This is the
    /// only push a closed queue takes, so the shutdown event is not dropped.
    pub fn enqueue_shutdown(&self, task: Task) -> bool {
        self.push(Priority::Normal, task, true)
    }

    fn push(&self, priority: Priority, task: Task, during_shutdown: bool) -> bool {
        let mut state = self.state.lock().unwrap();
        if state.shutting_down && !during_shutdown {
            return false;
        }
        let bucket = priority.index();
        state.tasks[bucket].push_back(task);
        if !state.posted {
            state.posted = self.poster.post();
            if !state.posted {
                state.tasks[bucket].pop_back();
                return false;
            }
        }
        true
    }

    /// Stop accepting work and leave the loop. The caller may still post one
    /// shutdown task first; see [`Queue::enqueue_shutdown`].
    pub fn begin_shutdown(&self) {
        self.state.lock().unwrap().shutting_down = true;
    }

    #[cfg(test)]
    pub fn is_shutting_down(&self) -> bool {
        self.state.lock().unwrap().shutting_down
    }

    /// Run every task currently queued, highest priority first. A task that
    /// enqueues more work is served before this returns, which is what one
    /// dispatched private message is expected to do.
    pub fn run_pending(&self) {
        loop {
            let task = {
                let mut state = self.state.lock().unwrap();
                let next = (0..3)
                    .rev()
                    .find_map(|bucket| state.tasks[bucket].pop_front());
                match next {
                    Some(task) => task,
                    None => {
                        // No gap between this lock and `push`: a new task either
                        // arrives before it and is taken above, or sees the post
                        // flag cleared and posts for itself.
                        state.posted = false;
                        return;
                    }
                }
            };
            task();
        }
    }

    /// Whether the calling thread is the one this queue was created on.
    pub fn has_thread_access(&self) -> bool {
        thread::current() == self.owner
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct Fake {
        posts: AtomicUsize,
        quits: AtomicUsize,
        refuse: AtomicUsize,
    }

    impl Fake {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                posts: AtomicUsize::new(0),
                quits: AtomicUsize::new(0),
                refuse: AtomicUsize::new(0),
            })
        }
    }

    impl Poster for Fake {
        fn post(&self) -> bool {
            self.posts.fetch_add(1, Ordering::SeqCst);
            self.refuse.load(Ordering::SeqCst) == 0
        }

        fn quit(&self) {
            self.quits.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn queue(poster: Arc<Fake>) -> Arc<Queue> {
        Queue::new(thread::current(), poster)
    }

    fn marker(log: &Arc<Mutex<Vec<&'static str>>>, name: &'static str) -> Task {
        let log = log.clone();
        Box::new(move || log.lock().unwrap().push(name))
    }

    #[test]
    fn high_priority_runs_before_low_even_when_enqueued_last() {
        let poster = Fake::new();
        let queue = queue(poster.clone());
        let log = Arc::new(Mutex::new(Vec::new()));

        assert!(queue.enqueue(Priority::Normal, marker(&log, "normal")));
        assert!(queue.enqueue(Priority::Low, marker(&log, "low")));
        assert!(queue.enqueue(Priority::High, marker(&log, "high")));
        assert_eq!(poster.posts.load(Ordering::SeqCst), 1);

        queue.run_pending();
        assert_eq!(*log.lock().unwrap(), ["high", "normal", "low"]);
        // The batch is one post, and a later enqueue asks for a new one.
        assert_eq!(poster.posts.load(Ordering::SeqCst), 1);
        assert!(queue.enqueue(Priority::Normal, marker(&log, "next")));
        assert_eq!(poster.posts.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn work_enqueued_from_a_task_runs_in_the_same_batch() {
        let poster = Fake::new();
        let queue = queue(poster.clone());
        let log = Arc::new(Mutex::new(Vec::new()));
        let inside = queue.clone();
        let nested = log.clone();

        queue.enqueue(
            Priority::Normal,
            Box::new(move || {
                nested.lock().unwrap().push("first");
                inside.enqueue(
                    Priority::High,
                    Box::new(move || nested.lock().unwrap().push("nested")),
                );
            }),
        );
        queue.run_pending();
        assert_eq!(*log.lock().unwrap(), ["first", "nested"]);
        assert_eq!(poster.posts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_queue_knows_its_own_thread() {
        let queue = queue(Fake::new());
        assert!(queue.has_thread_access());

        let worker = queue.clone();
        let other = std::thread::spawn(move || worker.has_thread_access())
            .join()
            .unwrap();
        assert!(!other);
    }

    #[test]
    fn a_closed_queue_takes_only_the_shutdown_task() {
        let poster = Fake::new();
        let queue = queue(poster.clone());
        queue.begin_shutdown();
        assert!(!queue.enqueue(Priority::Normal, Box::new(|| {})));
        assert!(queue.enqueue_shutdown(Box::new(|| {})));
        assert!(queue.is_shutting_down());
    }

    #[test]
    fn a_refused_post_keeps_the_task_out_of_the_queue() {
        let poster = Fake::new();
        poster.refuse.store(1, Ordering::SeqCst);
        let queue = queue(poster);
        let log = Arc::new(Mutex::new(Vec::new()));

        assert!(!queue.enqueue(Priority::Normal, marker(&log, "lost")));
        queue.run_pending();
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn a_quit_reaches_the_poster_once() {
        let poster = Fake::new();
        let queue = queue(poster.clone());
        queue.begin_shutdown();
        poster.quit();
        assert_eq!(poster.quits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn priority_values_map_to_the_metadata_constants() {
        assert_eq!(Priority::from_i32(-10), Priority::Low);
        assert_eq!(Priority::from_i32(0), Priority::Normal);
        assert_eq!(Priority::from_i32(10), Priority::High);
    }
}
