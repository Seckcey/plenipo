//! The owner's terminal (Phase 12, ADR-031): a shell on this PC, or on one of the owner's servers,
//! in the panel at the bottom of Plenipo's window.
//!
//! The owner is in charge: Guard does not check what the owner types, and no approval cards are
//! shown for it. Nothing the owner types or sees is recorded (it can hold a password typed at a
//! `sudo` prompt); the broker records only that a terminal opened and closed. A server's pinned ID
//! is still checked before Plenipo signs in, and its sign-in comes from the Vault and is never
//! shown (`broker/terminal.rs`).
//!
//! Workers never reach any of this. These are not tools: no AI tool is offered them, the tool
//! relay refuses any tool name it does not know, and only the owner's main window can call the
//! desktop commands that use them. Workers keep running one command at a time with `ssh_run`,
//! through Guard.
//!
//! This module holds the mechanics: finding the shells, running one in a pseudo terminal
//! (ConPTY on Windows, through `portable-pty`), and relaying a server's shell channel.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use russh::ChannelMsg;
use tokio::sync::mpsc as tmpsc;

use crate::dto::{ShellOption, TerminalShell};
use crate::ssh::{Connection, ShellChannel};

#[cfg(windows)]
mod job;

/// Output is gathered for this long before it goes to the screen, so a flood of small writes
/// becomes a few larger ones.
const GATHER: Duration = Duration::from_millis(4);
/// Most output sent to the screen at once.
const MAX_CHUNK: usize = 64 * 1024;
/// How long a shell on this PC has, once it has ended, to let its last output through. Windows'
/// pseudo console closes at once when its input is closed first; should it ever not, the
/// terminal still ends, within the time Plenipo gives its terminals when it quits.
const LAST_OUTPUT: Duration = Duration::from_secs(3);

/// Where output goes (to the screen), and how a terminal ended.
pub type Output = Arc<dyn Fn(&[u8]) + Send + Sync>;
pub type Ended = Box<dyn FnOnce(Ending) + Send>;

/// How a terminal ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ending {
    /// In plain words: "the shell ended", "you closed it".
    pub why: String,
    /// The shell's exit code, when it gave one.
    pub code: Option<i32>,
}

/// A terminal's size, in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl Size {
    /// A size within reason (xterm.js never asks for less than 1 × 1).
    pub fn clamped(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.clamp(2, 1000),
            rows: rows.clamp(1, 500),
        }
    }

    fn pty(self) -> PtySize {
        PtySize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

// ---- The shells on this PC ------------------------------------------------------------------------

/// A shell's program and the arguments it starts with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellProgram {
    pub label: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    /// `None`: a shell, with Plenipo's own environment, as the owner's terminal always has.
    /// `Some`: a program that is not a shell (an AI tool's sign-in tab, ADR-058 §2), with exactly
    /// these variables and nothing else; it ends as "<label> ended".
    pub env: Option<Vec<(std::ffi::OsString, std::ffi::OsString)>>,
}

pub fn shell_label(shell: TerminalShell) -> &'static str {
    match shell {
        TerminalShell::WindowsPowerShell => "Windows PowerShell",
        TerminalShell::PowerShell7 => "PowerShell 7",
        TerminalShell::CommandPrompt => "Command Prompt",
        TerminalShell::YourShell => "Your shell",
        TerminalShell::Zsh => "zsh",
        TerminalShell::Bash => "bash",
        TerminalShell::Fish => "fish",
    }
}

/// The shells this system offers, in order: Windows' three, or a Mac's and Linux's four (Phase
/// 23, ADR-150).
const fn offered() -> &'static [TerminalShell] {
    if cfg!(windows) {
        &[
            TerminalShell::WindowsPowerShell,
            TerminalShell::PowerShell7,
            TerminalShell::CommandPrompt,
        ]
    } else {
        &[
            TerminalShell::YourShell,
            TerminalShell::Zsh,
            TerminalShell::Bash,
            TerminalShell::Fish,
        ]
    }
}

/// The shell a choice means on this system: a choice no shell here answers to (a Windows one on
/// a Mac, or the default on a Mac or Linux PC) is this system's first one.
pub fn effective_shell(shell: TerminalShell) -> TerminalShell {
    if offered().contains(&shell) {
        shell
    } else {
        offered()[0]
    }
}

/// Who a terminal on this PC runs as, in this system's words (ADR-155).
pub fn runs_as() -> &'static str {
    if cfg!(windows) {
        "as your own Windows user — never as administrator"
    } else {
        "as yourself — never as root"
    }
}

/// Shells that start as login shells with `-l`, as a Mac's Terminal starts them.
#[cfg(target_os = "macos")]
const LOGIN_SHELLS: &[&str] = &["zsh", "bash", "fish", "sh", "ksh", "dash", "tcsh", "csh"];

fn existing(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

/// A program on the PATH.
fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .filter(|dir| dir.is_absolute())
            .find_map(|dir| existing(dir.join(name)))
    })
}

/// Where each shell is on Windows (`None`: not installed).
#[cfg(windows)]
fn find_shell(shell: TerminalShell) -> Option<PathBuf> {
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    match shell {
        TerminalShell::WindowsPowerShell => {
            existing(system_root.join(r"System32\WindowsPowerShell\v1.0\powershell.exe"))
        }
        TerminalShell::PowerShell7 => on_path("pwsh.exe").or_else(|| {
            std::env::var_os("ProgramFiles")
                .and_then(|p| existing(PathBuf::from(p).join(r"PowerShell\7\pwsh.exe")))
        }),
        TerminalShell::CommandPrompt => std::env::var_os("ComSpec")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .and_then(existing)
            .or_else(|| existing(system_root.join(r"System32\cmd.exe"))),
        _ => None,
    }
}

/// Where each shell is on a Mac or a Linux PC (`None`: not installed). Homebrew's newer bash and
/// fish come before the system's.
#[cfg(not(windows))]
fn find_shell(shell: TerminalShell) -> Option<PathBuf> {
    let first = |paths: &[&str]| paths.iter().map(PathBuf::from).find_map(existing);
    match shell {
        TerminalShell::YourShell => Some(own_shell()),
        TerminalShell::Zsh => first(&[
            "/bin/zsh",
            "/usr/bin/zsh",
            "/opt/homebrew/bin/zsh",
            "/usr/local/bin/zsh",
        ]),
        TerminalShell::Bash => first(&[
            "/opt/homebrew/bin/bash",
            "/usr/local/bin/bash",
            "/bin/bash",
            "/usr/bin/bash",
        ]),
        TerminalShell::Fish => first(&[
            "/opt/homebrew/bin/fish",
            "/usr/local/bin/fish",
            "/usr/bin/fish",
            "/bin/fish",
        ]),
        _ => None,
    }
}

/// The owner's own shell on a Mac or a Linux PC (`$SHELL`), or bash, or `/bin/sh`.
#[cfg(not(windows))]
pub fn own_shell() -> PathBuf {
    std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .and_then(existing)
        .or_else(|| on_path("bash"))
        .unwrap_or_else(|| PathBuf::from("/bin/sh"))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The shells the owner can pick on this system, and whether this PC has each. The owner's own
/// shell on a Mac or Linux says which it is: "Your shell (zsh)".
pub fn shell_options() -> Vec<ShellOption> {
    offered()
        .iter()
        .map(|&shell| {
            let path = find_shell(shell);
            let label = match &path {
                Some(p) if shell == TerminalShell::YourShell => {
                    format!("{} ({})", shell_label(shell), file_name(p))
                }
                _ => shell_label(shell).to_owned(),
            };
            ShellOption {
                shell,
                label,
                installed: path.is_some(),
                path: path.map(|p| p.display().to_string()),
            }
        })
        .collect()
}

/// The program for `shell` on this PC (a choice another system made means this system's first
/// shell). On a Mac it starts as a login shell, as the Mac's Terminal starts it, so it reads the
/// settings Homebrew and others add (`~/.zprofile`).
pub fn shell_program(shell: TerminalShell) -> Result<ShellProgram, String> {
    let shell = effective_shell(shell);
    let program = find_shell(shell).ok_or_else(|| {
        format!(
            "{} is not installed on this PC; pick another shell in Settings → Terminal",
            shell_label(shell)
        )
    })?;
    let name = file_name(&program);
    let args = match shell {
        TerminalShell::WindowsPowerShell | TerminalShell::PowerShell7 => vec!["-NoLogo".into()],
        #[cfg(target_os = "macos")]
        _ if LOGIN_SHELLS.contains(&name.as_str()) => vec!["-l".into()],
        _ => Vec::new(),
    };
    let label = if shell == TerminalShell::YourShell && !name.is_empty() {
        name
    } else {
        shell_label(shell).to_owned()
    };
    Ok(ShellProgram {
        label,
        program,
        args,
        env: None,
    })
}

/// The owner's home folder, where a terminal on this PC starts.
pub fn home_folder() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_dir())
}

/// Whether Plenipo itself runs as administrator: on Windows with an elevated token, and on a
/// Mac or a Linux PC as root (ADR-150). If a Mac or Linux PC cannot say who Plenipo runs as,
/// the answer is yes, so nothing starts that might be root.
pub fn runs_as_administrator() -> bool {
    #[cfg(windows)]
    {
        job::is_elevated()
    }
    #[cfg(unix)]
    {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
        let Ok(me) = sysinfo::get_current_pid() else {
            return true;
        };
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[me]),
            true,
            ProcessRefreshKind::nothing().with_user(UpdateKind::Always),
        );
        system
            .process(me)
            .and_then(|p| p.effective_user_id())
            .is_none_or(|uid| **uid == 0)
    }
    #[cfg(not(any(windows, unix)))]
    {
        true
    }
}

/// Why a terminal on this PC must not start: Plenipo itself runs as administrator (or root), so
/// the shell would too (ADR-031: never as administrator; ADR-150: never as root).
pub fn refuse_elevated() -> Result<(), String> {
    if runs_as_administrator() {
        return Err(if cfg!(windows) {
            "Plenipo is running as administrator, so a terminal would be too. Close Plenipo and \
             start it normally (not \"Run as administrator\") to use the terminal."
                .into()
        } else {
            "Plenipo is running as root, so a terminal would be too. Close Plenipo and start it \
             as yourself (not with sudo) to use the terminal."
                .into()
        });
    }
    Ok(())
}

// ---- A shell on this PC -----------------------------------------------------------------------------

/// A shell running in a pseudo terminal on this PC.
pub struct LocalShell {
    input: mpsc::Sender<Option<Vec<u8>>>,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    closing: Arc<Mutex<Option<String>>>,
    /// Windows: the shell's kill-on-close job; dropping it ends the shell and its programs.
    #[cfg(windows)]
    job: Arc<Mutex<Option<job::Job>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Start `shell` in `cwd`, sized `size`. Its output goes to `output` as it arrives; `ended` is
/// called once, after the last output, when it ends (at most `LAST_OUTPUT` later).
pub fn start_local(
    shell: &ShellProgram,
    cwd: Option<&Path>,
    size: Size,
    output: Output,
    ended: Ended,
) -> Result<LocalShell, String> {
    let pair = native_pty_system()
        .openpty(size.pty())
        .map_err(|e| format!("Plenipo could not open a terminal ({e})"))?;
    // A program that is not a shell (an AI tool's sign-in tab) ends as itself: "Codex's sign-in
    // ended", not "the shell ended".
    let ends_as = match shell.env {
        Some(_) => format!("{} ended", shell.label),
        None => "the shell ended".to_owned(),
    };
    let mut command = CommandBuilder::new(&shell.program);
    command.args(&shell.args);
    if let Some(dir) = cwd {
        command.cwd(dir);
    }
    if let Some(env) = &shell.env {
        command.env_clear();
        for (name, value) in env {
            command.env(name, value);
        }
    }
    // What the shell may expect of the screen part (xterm.js): colors, and 256 of them.
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| format!("{} could not be started ({e})", shell.label))?;
    // The shell holds the other end now; this copy would keep the terminal open after it ends.
    drop(pair.slave);
    #[cfg(windows)]
    let job = {
        let job = job::Job::new();
        if let Some(j) = &job {
            j.adopt(child.as_raw_handle());
        }
        Arc::new(Mutex::new(job))
    };
    // On a Mac and Linux the shell leads a session of its own; whatever it leaves running in it
    // is ended when it ends (below).
    #[cfg(unix)]
    let leader = child.process_id();
    let killer = child.clone_killer();
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Plenipo could not read the terminal ({e})"));
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Plenipo could not write to the terminal ({e})"));
    let (mut reader, mut writer) = match (reader, writer) {
        (Ok(r), Ok(w)) => (r, w),
        (Err(e), _) | (_, Err(e)) => {
            let _ = child.kill();
            return Err(e);
        }
    };
    let master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>> =
        Arc::new(Mutex::new(Some(pair.master)));
    let closing: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    // Output: read as it comes (blocking), gathered briefly, then sent to the screen.
    let (chunks, gathered) = mpsc::channel::<Vec<u8>>();
    let read_thread = std::thread::Builder::new()
        .name("plenipo-terminal-read".into())
        .spawn(move || {
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if chunks.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        })
        .map_err(|e| format!("Plenipo could not start the terminal ({e})"))?;
    let pump = std::thread::Builder::new()
        .name("plenipo-terminal-out".into())
        .spawn(move || {
            while let Ok(first) = gathered.recv() {
                let mut chunk = first;
                while chunk.len() < MAX_CHUNK {
                    match gathered.recv_timeout(GATHER) {
                        Ok(more) => chunk.extend_from_slice(&more),
                        Err(_) => break,
                    }
                }
                output(&chunk);
            }
        })
        .map_err(|e| format!("Plenipo could not start the terminal ({e})"))?;

    // Typing: written in order, on a thread of its own (a busy shell may take a while to read).
    // `None` closes the way in, once the shell has ended.
    let (input, typed) = mpsc::channel::<Option<Vec<u8>>>();
    std::thread::Builder::new()
        .name("plenipo-terminal-write".into())
        .spawn(move || {
            while let Ok(Some(bytes)) = typed.recv() {
                if writer
                    .write_all(&bytes)
                    .and_then(|()| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
        })
        .map_err(|e| format!("Plenipo could not start the terminal ({e})"))?;

    // The end: wait for the shell, close the terminal (on Windows the output only ends then),
    // let the last output through, and say how it ended.
    {
        let master = Arc::clone(&master);
        let closing = Arc::clone(&closing);
        let typing = input.clone();
        #[cfg(windows)]
        let job = Arc::clone(&job);
        std::thread::Builder::new()
            .name("plenipo-terminal-wait".into())
            .spawn(move || {
                let status = child.wait();
                // Programs the shell left running end with it: on Windows through its job, on a
                // Mac and Linux through its session (Phase 23, ADR-150).
                #[cfg(windows)]
                drop(lock(&job).take());
                #[cfg(unix)]
                if let Some(leader) = leader {
                    end_session(leader);
                }
                // The way in closes first. As it starts, Windows' pseudo console asks the
                // screen where the cursor is and waits for the answer; one closed before the
                // screen answered would go on waiting instead of closing.
                let _ = typing.send(None);
                let pty = lock(&master).take();
                let (done, last_output) = mpsc::channel::<()>();
                let closer = std::thread::Builder::new()
                    .name("plenipo-terminal-close".into())
                    .spawn(move || {
                        drop(pty);
                        let _ = read_thread.join();
                        let _ = pump.join();
                        let _ = done.send(());
                    });
                if closer.is_ok() && last_output.recv_timeout(LAST_OUTPUT).is_err() {
                    log::warn!("a terminal's last output did not come; it ends anyway");
                }
                let code = status
                    .as_ref()
                    .ok()
                    .and_then(|s| i32::try_from(s.exit_code()).ok());
                let why = lock(&closing).clone().unwrap_or(ends_as);
                ended(Ending { why, code });
            })
            .map_err(|e| format!("Plenipo could not start the terminal ({e})"))?;
    }
    Ok(LocalShell {
        input,
        master,
        killer: Mutex::new(killer),
        closing,
        #[cfg(windows)]
        job,
    })
}

/// How long the programs left in a closed terminal have to stop before they are ended.
#[cfg(unix)]
const SESSION_GRACE: Duration = Duration::from_secs(1);

/// End every program still in the session the terminal's shell led: asked to stop, then, a
/// moment later, ended for certain, as Windows' job object ends them (Phase 23, ADR-150). A
/// program that started a session of its own has left the terminal and is not touched, and the
/// system lets Plenipo signal only the owner's own programs.
#[cfg(unix)]
fn end_session(leader: u32) {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System};

    let own = std::process::id();
    if leader <= 1 || leader == own {
        return;
    }
    let session = Pid::from_u32(leader);
    let members = |system: &System| -> Vec<Pid> {
        system
            .processes()
            .values()
            .filter(|p| p.session_id() == Some(session) && p.pid().as_u32() != own)
            .map(sysinfo::Process::pid)
            .collect()
    };
    let mut system = System::new();
    let refresh = |system: &mut System| {
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
    };
    refresh(&mut system);
    let left = members(&system);
    if left.is_empty() {
        return;
    }
    for pid in &left {
        if let Some(p) = system.process(*pid) {
            let _ = p.kill_with(Signal::Term);
        }
    }
    std::thread::sleep(SESSION_GRACE);
    refresh(&mut system);
    for pid in members(&system) {
        if let Some(p) = system.process(pid) {
            let _ = p.kill_with(Signal::Kill);
        }
    }
}

impl LocalShell {
    pub fn write(&self, bytes: &[u8]) -> Result<(), String> {
        self.input
            .send(Some(bytes.to_vec()))
            .map_err(|_| "the terminal has ended".to_owned())
    }

    pub fn resize(&self, size: Size) -> Result<(), String> {
        match lock(&self.master).as_ref() {
            Some(m) => m
                .resize(size.pty())
                .map_err(|e| format!("the terminal could not be resized ({e})")),
            None => Err("the terminal has ended".into()),
        }
    }

    /// End the shell (and the programs it started): `why` is what the owner is told.
    pub fn close(&self, why: &str) {
        lock(&self.closing).get_or_insert_with(|| why.to_owned());
        let _ = lock(&self.killer).kill();
        #[cfg(windows)]
        drop(lock(&self.job).take());
    }
}

// ---- A shell on a server ----------------------------------------------------------------------------

enum RemoteInput {
    Bytes(Vec<u8>),
    Resize(Size),
    Close(String),
}

/// The owner's shell on a server: the channel Plenipo opened after checking the server's ID and
/// signing in (`ssh::Connection::open_shell`).
pub struct RemoteShell {
    input: tmpsc::UnboundedSender<RemoteInput>,
}

/// How long a server has to close the channel after the owner closes the terminal.
const REMOTE_CLOSE: Duration = Duration::from_secs(2);

/// Relay a server's shell channel: its output to `output`, the owner's typing to it. When it
/// ends (the shell exits, the owner closes it, or the connection is lost), the connection is
/// closed and `ended` is called once.
pub fn start_remote(
    connection: Connection,
    channel: ShellChannel,
    output: Output,
    ended: Ended,
) -> RemoteShell {
    let (input, mut typed) = tmpsc::unbounded_channel::<RemoteInput>();
    let ShellChannel {
        mut read,
        write,
        early,
    } = channel;
    tokio::spawn(async move {
        if !early.is_empty() {
            output(&early);
        }
        let mut code: Option<i32> = None;
        let mut signal: Option<String> = None;
        let mut closing: Option<String> = None;
        let mut deadline: Option<tokio::time::Instant> = None;
        loop {
            tokio::select! {
                msg = read.wait() => match msg {
                    Some(ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. }) => {
                        output(&data);
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        code = i32::try_from(exit_status).ok();
                    }
                    Some(ChannelMsg::ExitSignal { signal_name, .. }) => {
                        signal = Some(format!("{signal_name:?}"));
                    }
                    Some(ChannelMsg::Close) | None => break,
                    Some(_) => {}
                },
                next = typed.recv(), if closing.is_none() => match next {
                    Some(RemoteInput::Bytes(bytes)) => {
                        let _ = write.data_bytes(bytes).await;
                    }
                    Some(RemoteInput::Resize(size)) => {
                        let _ = write
                            .window_change(u32::from(size.cols), u32::from(size.rows), 0, 0)
                            .await;
                    }
                    Some(RemoteInput::Close(why)) => {
                        closing = Some(why);
                        let _ = write.eof().await;
                        let _ = write.close().await;
                        deadline = Some(tokio::time::Instant::now() + REMOTE_CLOSE);
                    }
                    None => {
                        closing = Some("Plenipo closed it".into());
                        let _ = write.close().await;
                        deadline = Some(tokio::time::Instant::now() + REMOTE_CLOSE);
                    }
                },
                () = async {
                    match deadline {
                        Some(d) => tokio::time::sleep_until(d).await,
                        None => std::future::pending::<()>().await,
                    }
                } => break,
            }
        }
        let lost =
            closing.is_none() && code.is_none() && signal.is_none() && connection.is_closed();
        connection.close("the terminal closed").await;
        let why = match (closing, signal, lost) {
            (Some(why), _, _) => why,
            (None, Some(signal), _) => format!("the shell was ended by the signal {signal}"),
            (None, None, true) => "the connection to the server was lost".into(),
            (None, None, false) => "the shell ended".into(),
        };
        ended(Ending { why, code });
    });
    RemoteShell { input }
}

impl RemoteShell {
    pub fn write(&self, bytes: &[u8]) -> Result<(), String> {
        self.input
            .send(RemoteInput::Bytes(bytes.to_vec()))
            .map_err(|_| "the terminal has ended".to_owned())
    }

    pub fn resize(&self, size: Size) -> Result<(), String> {
        self.input
            .send(RemoteInput::Resize(size))
            .map_err(|_| "the terminal has ended".to_owned())
    }

    pub fn close(&self, why: &str) {
        let _ = self.input.send(RemoteInput::Close(why.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_stay_within_reason() {
        assert_eq!(Size::clamped(0, 0), Size { cols: 2, rows: 1 });
        assert_eq!(Size::clamped(80, 24), Size { cols: 80, rows: 24 });
        assert_eq!(
            Size::clamped(u16::MAX, u16::MAX),
            Size {
                cols: 1000,
                rows: 500
            }
        );
    }

    #[test]
    fn every_shell_has_a_plain_name_and_one_is_the_default() {
        let options = shell_options();
        assert_eq!(TerminalShell::default(), TerminalShell::WindowsPowerShell);
        let labels: Vec<_> = options.iter().map(|o| o.label.as_str()).collect();
        if cfg!(windows) {
            assert_eq!(
                labels,
                ["Windows PowerShell", "PowerShell 7", "Command Prompt"]
            );
        } else {
            // Each system's own (ADR-155): the owner's shell, named, then zsh, bash, and fish.
            assert_eq!(labels.len(), 4, "{labels:?}");
            assert!(labels[0].starts_with("Your shell"), "{labels:?}");
            assert_eq!(labels[1..], ["zsh", "bash", "fish"]);
            assert!(
                options[0].installed,
                "the owner's own shell is always there"
            );
            assert_eq!(
                effective_shell(TerminalShell::default()),
                TerminalShell::YourShell
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_the_users_own_shell_is_used() {
        let program = shell_program(TerminalShell::CommandPrompt).unwrap();
        assert!(program.program.is_absolute(), "{program:?}");
        assert!(!program.label.is_empty());
    }
}
