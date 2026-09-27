//! Windows: each terminal's shell lives in its own kill-on-close Job Object, as Plenipo's other
//! programs do (`plenipo_runtime`'s supervisor). Closing the terminal, or Plenipo ending for any
//! reason, closes the job's handle, and Windows ends the shell and every program it started.
//! Also: whether Plenipo itself runs as administrator (then no terminal opens, ADR-031).

use std::os::windows::io::RawHandle;

use win32job::{ExtendedLimitInfo, Job as Win32Job};

/// A kill-on-close Job Object.
pub struct Job(Win32Job);

impl Job {
    /// A new job whose programs end when it is dropped (or Plenipo ends). `None` if Windows
    /// would not make one.
    pub fn new() -> Option<Self> {
        let mut limits = ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        Win32Job::create_with_limit_info(&limits).ok().map(Self)
    }

    /// Put a program in the job (the programs it starts follow it there).
    pub fn adopt(&self, process: Option<RawHandle>) -> bool {
        process.is_some_and(|p| self.0.assign_process(p as isize).is_ok())
    }
}

/// Whether Plenipo runs as administrator (an elevated token).
pub fn is_elevated() -> bool {
    is_elevated::is_elevated()
}
