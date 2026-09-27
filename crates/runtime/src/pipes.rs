//! Two extra pipes a program can inherit as its file descriptors 3 and 4 (Phase 10, ADR-020):
//! Plenipo's browser speaks the DevTools protocol over them, so there is no network port for
//! another program on the computer to find and connect to.
//!
//! [`ExtraPipes::create`] makes the pipes. The child's ends go into a
//! [`LaunchSpec`](crate::LaunchSpec); [`PipeEnds`] are Plenipo's own ends, which block like
//! files. The supervisor hands the child its ends when it starts it: on Unix by moving them
//! onto descriptors 3 and 4 in the new process before it runs the program (`pre_exec`); on
//! Windows by starting the program itself (`CreateProcessW`) with the C runtime's descriptor
//! table filled in, the way a program's standard descriptors reach it — something
//! `std::process::Command` cannot do. Right after the start, Plenipo closes its copies of the
//! child's ends, so that the child's exit shows as the end of the pipe.

use std::io::{self, PipeReader, PipeWriter};
use std::sync::{Arc, Mutex};

#[cfg(unix)]
pub(crate) mod unix;
#[cfg(windows)]
pub(crate) mod windows;

/// The child's ends: what it reads as descriptor 3 and writes as descriptor 4.
#[derive(Debug)]
pub(crate) struct ChildEnds {
    pub(crate) read: PipeReader,
    pub(crate) write: PipeWriter,
}

/// Plenipo's ends of the two pipes: `writer` reaches the child's descriptor 3, `reader` gets
/// what the child writes to 4. Both block, like files.
#[derive(Debug)]
pub struct PipeEnds {
    pub writer: PipeWriter,
    pub reader: PipeReader,
}

/// The child's ends, for a [`LaunchSpec`](crate::LaunchSpec). Cloning a spec shares them; the
/// supervisor takes them once, at spawn.
#[derive(Debug, Clone)]
pub struct ExtraPipes(Arc<Mutex<Option<ChildEnds>>>);

impl ExtraPipes {
    /// Two new pipes: the child's ends, and Plenipo's.
    pub fn create() -> io::Result<(Self, PipeEnds)> {
        // Plenipo writes, the child reads (its descriptor 3).
        let (read, writer) = io::pipe()?;
        // The child writes (its descriptor 4), Plenipo reads.
        let (reader, write) = io::pipe()?;
        #[cfg(unix)]
        let (read, write) = (
            clear_of_targets(read, PipeReader::try_clone)?,
            clear_of_targets(write, PipeWriter::try_clone)?,
        );
        Ok((
            Self(Arc::new(Mutex::new(Some(ChildEnds { read, write })))),
            PipeEnds { writer, reader },
        ))
    }

    pub(crate) fn take(&self) -> Option<ChildEnds> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).take()
    }
}

/// A child's end with a descriptor number of 5 or more, so that moving the two ends onto 3 and
/// 4 in the child can never close one of them first. The lower-numbered copies stay open until
/// a high enough one exists, so that each new copy lands higher.
#[cfg(unix)]
fn clear_of_targets<T: std::os::fd::AsRawFd>(
    end: T,
    copy: impl Fn(&T) -> io::Result<T>,
) -> io::Result<T> {
    let mut end = end;
    let mut low = Vec::new();
    while end.as_raw_fd() < 5 {
        let higher = copy(&end)?;
        low.push(end);
        end = higher;
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read as _, Write as _};

    #[test]
    fn the_ends_are_connected_and_taken_once() {
        let (pipes, mut ends) = ExtraPipes::create().unwrap();
        let shared = pipes.clone();
        let mut child = shared.take().expect("the child's ends");
        assert!(pipes.take().is_none(), "taken once, for one spawn");
        ends.writer.write_all(b"to the child").unwrap();
        let mut text = [0u8; 12];
        child.read.read_exact(&mut text).unwrap();
        assert_eq!(&text, b"to the child");
        child.write.write_all(b"from the child").unwrap();
        drop(child);
        let mut back = String::new();
        ends.reader.read_to_string(&mut back).unwrap();
        assert_eq!(back, "from the child");
    }

    #[cfg(unix)]
    #[test]
    fn the_childs_ends_never_sit_on_3_or_4() {
        use std::os::fd::AsRawFd as _;
        let (pipes, _ends) = ExtraPipes::create().unwrap();
        let child = pipes.take().unwrap();
        assert!(child.read.as_raw_fd() >= 5 && child.write.as_raw_fd() >= 5);
    }
}
