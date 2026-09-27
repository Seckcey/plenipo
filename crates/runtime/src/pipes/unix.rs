//! Unix: the child's ends become its descriptors 3 and 4 between fork and exec.

// The one place in this crate that may use unsafe code (see the crate's lints): the hook that
// runs in the new process before the program does, and one system call in it.
#![allow(unsafe_code)]

use std::io;
use std::os::fd::AsRawFd as _;

use super::ChildEnds;

/// Make `command` place the child's ends at descriptors 3 (read) and 4 (write) in the new
/// process. The ends must stay open in Plenipo until the command has spawned.
pub(crate) fn inherit(command: &mut tokio::process::Command, ends: &ChildEnds) {
    let moves = [(ends.read.as_raw_fd(), 3), (ends.write.as_raw_fd(), 4)];
    // SAFETY: the hook runs in the child between fork and exec, where only async-signal-safe
    // calls are allowed. `dup2` is one, and the hook does nothing else: no allocation, no locks,
    // no other library calls. Both source descriptors are 5 or higher (`ExtraPipes::create`),
    // so neither move can close the other's source. `dup2` clears close-on-exec on 3 and 4, and
    // the sources close at exec as they were opened to.
    unsafe {
        command.pre_exec(move || {
            for (from, to) in moves {
                if libc::dup2(from, to) == -1 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}
