//! Windows: a program that gets extra pipes is started with `CreateProcessW` directly, because
//! the C runtime's descriptor table (`STARTUPINFOW.lpReserved2`, the only way a program's
//! descriptors 3 and 4 come to exist on Windows) is nothing `std::process::Command` can fill in.
//! Everything else matches the supervisor's usual start: the program runs in its own job object
//! that stops the whole tree when the job closes (kill on drop), starts suspended and is resumed
//! once it is in the job, gets the cleared environment, no console window, an empty stdin, and
//! stdout and stderr that Plenipo reads as usual.

// The one place in this crate that may use unsafe code (see the crate's lints): calls into
// Windows, each with the reason it is sound written next to it.
#![allow(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::future::Future;
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::io::{
    AsHandle as _, AsRawHandle as _, BorrowedHandle, FromRawHandle as _, OwnedHandle,
};
use std::os::windows::process::ExitStatusExt as _;
use std::path::Path;
use std::pin::Pin;
use std::process::ExitStatus;

use process_wrap::tokio::ChildWrapper;
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::watch;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Threading::{
    CreateProcessW, GetCurrentProcess, GetExitCodeProcess, ResumeThread, TerminateProcess,
    WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, INFINITE,
    PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOW,
};

use super::ChildEnds;

/// The C runtime's flags for an inherited descriptor: open, and a device (`NUL`) or a pipe.
const FOPEN: u8 = 0x01;
const FPIPE: u8 = 0x08;
const FDEV: u8 = 0x40;

/// How a program that got extra pipes ended: its exit status, or the system error that kept
/// Plenipo from learning it.
type Exit = Result<ExitStatus, i32>;

fn raw(handle: &OwnedHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}

/// Start `executable` with `args`, `env`, and `working_dir`, its descriptors 3 and 4 being the
/// child's `ends`. Returns the child and the pipes its stdout and stderr go to.
pub(crate) fn spawn(
    executable: &Path,
    args: &[String],
    env: &[(OsString, OsString)],
    working_dir: &Path,
    ends: ChildEnds,
) -> io::Result<(PipedChild, File, File)> {
    let program = wide(executable.as_os_str())?;
    let mut line = command_line(executable, args)?;
    let block = environment_block(env)?;
    let dir = wide(working_dir.as_os_str())?;
    // The same standard streams as every other program the supervisor starts: an empty stdin,
    // and stdout and stderr that Plenipo reads.
    let stdin = File::open("NUL")?;
    let (out_read, out_write) = io::pipe()?;
    let (err_read, err_write) = io::pipe()?;
    // Copies the child may inherit, made only now and closed right after the start, so that
    // no other program Plenipo starts meanwhile can inherit them too.
    let inherited: [(OwnedHandle, u8); 5] = [
        (inheritable(stdin.as_handle())?, FOPEN | FDEV),
        (inheritable(out_write.as_handle())?, FOPEN | FPIPE),
        (inheritable(err_write.as_handle())?, FOPEN | FPIPE),
        (inheritable(ends.read.as_handle())?, FOPEN | FPIPE),
        (inheritable(ends.write.as_handle())?, FOPEN | FPIPE),
    ];
    let mut table = descriptor_table(&inherited);
    let startup = STARTUPINFOW {
        cb: u32::try_from(std::mem::size_of::<STARTUPINFOW>()).expect("a small struct"),
        dwFlags: STARTF_USESTDHANDLES,
        cbReserved2: u16::try_from(table.len()).expect("five descriptors"),
        lpReserved2: table.as_mut_ptr(),
        hStdInput: raw(&inherited[0].0),
        hStdOutput: raw(&inherited[1].0),
        hStdError: raw(&inherited[2].0),
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    // SAFETY: every pointer is to a live, NUL-terminated buffer of this function (the command
    // line is writable, as the call requires), the handles in `startup` are open, and `info`
    // is a valid place for the result.
    unsafe {
        CreateProcessW(
            PCWSTR(program.as_ptr()),
            Some(PWSTR(line.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW | CREATE_SUSPENDED,
            Some(block.as_ptr().cast()),
            PCWSTR(dir.as_ptr()),
            &startup,
            &mut info,
        )?;
    }
    // The child has its own copies now.
    drop(inherited);
    drop((ends, stdin, out_write, err_write));
    // SAFETY: fresh handles from `CreateProcessW`, owned here and nowhere else.
    let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess.0) };
    let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread.0) };
    let job = match job_around(&process) {
        Ok(job) => job,
        Err(e) => {
            // SAFETY: an open process handle.
            let _ = unsafe { TerminateProcess(raw(&process), 1) };
            return Err(e);
        }
    };
    // SAFETY: the primary thread's handle, still open; the process was created suspended.
    if unsafe { ResumeThread(raw(&thread)) } == u32::MAX {
        let e = io::Error::last_os_error();
        // SAFETY: an open job handle.
        let _ = unsafe { TerminateJobObject(raw(&job), 1) };
        return Err(e);
    }
    drop(thread);
    let child = PipedChild::new(process, job, info.dwProcessId)?;
    Ok((
        child,
        File::from(OwnedHandle::from(out_read)),
        File::from(OwnedHandle::from(err_read)),
    ))
}

/// A copy of `handle` that a child process may inherit.
fn inheritable(handle: BorrowedHandle<'_>) -> io::Result<OwnedHandle> {
    let mut copy = HANDLE(std::ptr::null_mut());
    // SAFETY: both process handles are this process's own, `handle` is open, and `copy` is a
    // valid place for the new handle.
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            HANDLE(handle.as_raw_handle()),
            GetCurrentProcess(),
            &mut copy,
            0,
            true,
            DUPLICATE_SAME_ACCESS,
        )?;
    }
    // SAFETY: a handle `DuplicateHandle` just made, owned here and nowhere else.
    Ok(unsafe { OwnedHandle::from_raw_handle(copy.0) })
}

/// The C runtime's inherited-descriptor table: a count, one flag byte per descriptor, then one
/// handle per descriptor, packed. Descriptors 0 to 2 are stdin, stdout, and stderr; 3 and 4
/// are the extra pipes.
fn descriptor_table(handles: &[(OwnedHandle, u8)]) -> Vec<u8> {
    let count = i32::try_from(handles.len()).expect("a handful of descriptors");
    let mut table = Vec::with_capacity(4 + handles.len() * (1 + std::mem::size_of::<usize>()));
    table.extend_from_slice(&count.to_ne_bytes());
    table.extend(handles.iter().map(|(_, flags)| *flags));
    for (handle, _) in handles {
        table.extend_from_slice(&(handle.as_raw_handle() as usize).to_ne_bytes());
    }
    table
}

/// A new job object that stops every process in it when its handle closes, with `process` in
/// it.
fn job_around(process: &OwnedHandle) -> io::Result<OwnedHandle> {
    // SAFETY: a new, unnamed job object; the handle it returns is owned here and nowhere else.
    let job = unsafe { OwnedHandle::from_raw_handle(CreateJobObjectW(None, PCWSTR::null())?.0) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: `limits` is a live struct of the size passed; both handles are open.
    unsafe {
        SetInformationJobObject(
            raw(&job),
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                .expect("a small struct"),
        )?;
        AssignProcessToJobObject(raw(&job), raw(process))?;
    }
    Ok(job)
}

/// `text` as UTF-16 with a NUL at the end, refusing a NUL inside (it would cut the text short).
fn wide(text: &OsStr) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = text.encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a NUL byte in a program's arguments or environment",
        ));
    }
    wide.push(0);
    Ok(wide)
}

/// The command line as `CommandLineToArgvW` (and so the program) reads it back: the program
/// quoted, then each argument, with the standard library's quoting rules.
fn command_line(program: &Path, args: &[String]) -> io::Result<Vec<u16>> {
    let mut line = Vec::new();
    append_argument(&mut line, program.as_os_str(), true)?;
    for arg in args {
        line.push(u16::from(b' '));
        append_argument(&mut line, OsStr::new(arg), false)?;
    }
    line.push(0);
    Ok(line)
}

fn append_argument(line: &mut Vec<u16>, arg: &OsStr, force_quotes: bool) -> io::Result<()> {
    const QUOTE: u16 = b'"' as u16;
    const BACKSLASH: u16 = b'\\' as u16;
    let mut wide = wide(arg)?;
    wide.pop();
    let quote = force_quotes
        || wide.is_empty()
        || wide
            .iter()
            .any(|&c| c == u16::from(b' ') || c == u16::from(b'\t'));
    if quote {
        line.push(QUOTE);
    }
    let mut backslashes = 0;
    for &c in &wide {
        if c == BACKSLASH {
            backslashes += 1;
        } else {
            if c == QUOTE {
                // Doubled backslashes plus one, so the quote itself stays.
                line.extend(std::iter::repeat_n(BACKSLASH, backslashes + 1));
            }
            backslashes = 0;
        }
        line.push(c);
    }
    if quote {
        // Doubled backslashes before the closing quote, so it stays a closing quote.
        line.extend(std::iter::repeat_n(BACKSLASH, backslashes));
        line.push(QUOTE);
    }
    Ok(())
}

/// `NAME=value` strings, each ended by a NUL, then one more NUL.
fn environment_block(env: &[(OsString, OsString)]) -> io::Result<Vec<u16>> {
    let mut block = Vec::new();
    for (name, value) in env {
        let mut entry = OsString::from(name);
        entry.push("=");
        entry.push(value);
        block.extend(wide(&entry)?);
    }
    block.push(0);
    if env.is_empty() {
        block.push(0);
    }
    Ok(block)
}

/// A program started with extra pipes, as the supervisor sees every child it runs: its process
/// ID, its end, and how to stop it.
#[derive(Debug)]
pub(crate) struct PipedChild {
    process: OwnedHandle,
    /// Closing it stops whatever is left of the process tree (kill on drop, as for every other
    /// program the supervisor starts).
    job: OwnedHandle,
    pid: u32,
    exit: watch::Receiver<Option<Exit>>,
    // Never open: the child's streams are read from its own pipes.
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
}

impl PipedChild {
    fn new(process: OwnedHandle, job: OwnedHandle, pid: u32) -> io::Result<Self> {
        let (tx, exit) = watch::channel(None);
        let waited = process.try_clone()?;
        std::thread::Builder::new()
            .name("plenipo-piped-child".into())
            .spawn(move || {
                let _ = tx.send(Some(wait_for(&waited)));
            })?;
        Ok(Self {
            process,
            job,
            pid,
            exit,
            stdin: None,
            stdout: None,
            stderr: None,
        })
    }
}

/// Block until the process ends, and read how it ended.
fn wait_for(process: &OwnedHandle) -> Exit {
    let os_error = || io::Error::last_os_error().raw_os_error().unwrap_or(0);
    // SAFETY: an open process handle this thread owns.
    if unsafe { WaitForSingleObject(raw(process), INFINITE) } != WAIT_OBJECT_0 {
        return Err(os_error());
    }
    let mut code = 0u32;
    // SAFETY: an open process handle; `code` is a valid place for the result.
    match unsafe { GetExitCodeProcess(raw(process), &mut code) } {
        Ok(()) => Ok(ExitStatus::from_raw(code)),
        Err(_) => Err(os_error()),
    }
}

fn status(exit: Exit) -> io::Result<ExitStatus> {
    exit.map_err(io::Error::from_raw_os_error)
}

impl ChildWrapper for PipedChild {
    fn inner(&self) -> &dyn ChildWrapper {
        self
    }

    fn inner_mut(&mut self) -> &mut dyn ChildWrapper {
        self
    }

    fn into_inner(self: Box<Self>) -> Box<dyn ChildWrapper> {
        self
    }

    fn process_handle(&self) -> Option<BorrowedHandle<'_>> {
        Some(self.process.as_handle())
    }

    fn stdin(&mut self) -> &mut Option<ChildStdin> {
        &mut self.stdin
    }

    fn stdout(&mut self) -> &mut Option<ChildStdout> {
        &mut self.stdout
    }

    fn stderr(&mut self) -> &mut Option<ChildStderr> {
        &mut self.stderr
    }

    fn id(&self) -> Option<u32> {
        Some(self.pid)
    }

    fn start_kill(&mut self) -> io::Result<()> {
        // SAFETY: the job handle stays open as long as `self` lives.
        unsafe { TerminateJobObject(raw(&self.job), 1) }.map_err(io::Error::from)
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.exit.borrow().map(status).transpose()
    }

    fn wait(&mut self) -> Pin<Box<dyn Future<Output = io::Result<ExitStatus>> + Send + '_>> {
        Box::pin(async {
            let exit = self
                .exit
                .wait_for(Option::is_some)
                .await
                .map_err(|_| io::Error::other("lost track of the program"))?;
            status(exit.expect("waited for the end"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(program: &str, args: &[&str]) -> String {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let mut wide = command_line(Path::new(program), &args).unwrap();
        assert_eq!(wide.pop(), Some(0));
        String::from_utf16(&wide).unwrap()
    }

    #[test]
    fn the_command_line_quotes_as_windows_reads_it_back() {
        assert_eq!(
            line(
                r"C:\Program Files\Edge\msedge.exe",
                &["--flag", "a b", "", r"x\"]
            ),
            r#""C:\Program Files\Edge\msedge.exe" --flag "a b" "" x\"#
        );
        assert_eq!(
            line("p.exe", &[r#"say "hi""#, r#"tail\"#, r#"a\"b"#]),
            r#""p.exe" "say \"hi\"" tail\ a\\\"b"#
        );
        assert!(command_line(Path::new("p.exe"), &["a\0b".into()]).is_err());
    }

    #[test]
    fn the_environment_block_ends_in_two_nuls() {
        let block =
            environment_block(&[("A".into(), "1".into()), ("B".into(), "x y".into())]).unwrap();
        assert_eq!(String::from_utf16(&block).unwrap(), "A=1\0B=x y\0\0");
        assert_eq!(environment_block(&[]).unwrap(), [0, 0]);
    }

    #[test]
    fn the_descriptor_table_is_packed() {
        let (a, _) = io::pipe().unwrap();
        let (b, _) = io::pipe().unwrap();
        let handles = [
            (OwnedHandle::from(a), FOPEN | FDEV),
            (OwnedHandle::from(b), FOPEN | FPIPE),
        ];
        let table = descriptor_table(&handles);
        assert_eq!(table.len(), 4 + 2 + 2 * std::mem::size_of::<usize>());
        assert_eq!(&table[..4], &2i32.to_ne_bytes());
        assert_eq!(&table[4..6], &[0x41, 0x09]);
        let second = &table[6 + std::mem::size_of::<usize>()..];
        assert_eq!(
            second,
            &(handles[1].0.as_raw_handle() as usize).to_ne_bytes()
        );
    }
}
