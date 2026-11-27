//! The thread identity a queue compares against.
//!
//! Win32 answers this with `GetCurrentThreadId`; a host that only runs the queue
//! tests uses `ThreadId`, which has the same equality and no numeric conversion.
//! Keeping the difference here is what lets the queue logic be tested off Windows.

#[cfg(windows)]
pub type Id = u32;

#[cfg(windows)]
pub fn current() -> Id {
    // Safety: the function reads the current thread's id and has no preconditions.
    unsafe { windows::Win32::System::Threading::GetCurrentThreadId() }
}

#[cfg(not(windows))]
pub type Id = std::thread::ThreadId;

#[cfg(not(windows))]
pub fn current() -> Id {
    std::thread::current().id()
}
