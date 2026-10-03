//! The keeper (Phase 23, ADR-157): on a Mac and Linux, one small helper per Plenipo that ends
//! every worker's program group if Plenipo goes away without stopping them. Windows' job object
//! does this already, so there the keeper never starts.
//!
//! Plenipo starts its own program again with [`SWITCH`]. Each supervisor tells the keeper, over
//! the keeper's standard input, each program group it starts (`+<group>`) and each that ends
//! (`-<group>`). When that input closes, because Plenipo quit, crashed, or was killed, the keeper
//! asks every group still listed to stop, waits [`GRACE`], ends what is left for certain, and
//! exits. A normal quit has stopped the work already, so its list is empty then. Then it ends any
//! program still carrying this Plenipo's mark, which a program that left its group keeps
//! (ADR-158); Plenipo gives the keeper its part of the mark when it starts it.

use std::collections::BTreeSet;
use std::io::{BufRead, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::Duration;

/// The switch that starts Plenipo's own program as the keeper.
pub const SWITCH: &str = "--plenipo-keeper";
/// How long the keeper waits between asking the groups to stop and ending them.
pub const GRACE: Duration = Duration::from_secs(2);

/// Keeper mode: when `args` (a program's arguments, its own name first) ask for it, run as the
/// keeper until its input closes, and give the exit code. The argument after the switch is this
/// Plenipo's part of the mark (ADR-158); without a usable one, only the groups are ended.
pub fn maybe_run_from_args(args: impl IntoIterator<Item = String>) -> Option<i32> {
    let mut args = args.into_iter().skip(1);
    if args.next()? != SWITCH {
        return None;
    }
    let instance = args.next().filter(|i| crate::marks::is_instance(i));
    Some(run(std::io::stdin().lock(), instance.as_deref()))
}

/// The keeper itself: read `+<group>` and `-<group>` lines until `input` ends, then end every
/// group still listed, and every program carrying `instance`'s mark. A line it cannot read is
/// skipped; group 0 and 1 (every program of this user, and the system's first program) are
/// never listed.
pub fn run(input: impl BufRead, instance: Option<&str>) -> i32 {
    let mut groups = BTreeSet::new();
    for line in input.lines() {
        let Ok(line) = line else { break };
        let (add, id) = match line.trim().split_at_checked(1) {
            Some(("+", id)) => (true, id),
            Some(("-", id)) => (false, id),
            _ => continue,
        };
        match id.parse::<u32>() {
            Ok(id) if id > 1 && add => {
                groups.insert(id);
            }
            Ok(id) => {
                groups.remove(&id);
            }
            Err(_) => {}
        }
    }
    end_groups(&groups);
    if let Some(instance) = instance {
        crate::marks::end_marked(
            |mark| crate::marks::of_instance(mark, instance),
            crate::marks::GRACE,
        );
    }
    0
}

/// Ask each group to stop, wait, then end what is left. The system lets the keeper signal only
/// its own user's programs.
#[cfg(unix)]
fn end_groups(groups: &BTreeSet<u32>) {
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::Pid;

    let groups: Vec<Pid> = groups
        .iter()
        .filter_map(|&g| i32::try_from(g).ok())
        .map(Pid::from_raw)
        .collect();
    if groups.is_empty() {
        return;
    }
    for &group in &groups {
        let _ = killpg(group, Signal::SIGTERM);
    }
    std::thread::sleep(GRACE);
    for &group in &groups {
        let _ = killpg(group, Signal::SIGKILL);
    }
}

#[cfg(not(unix))]
fn end_groups(_groups: &BTreeSet<u32>) {}

// ---- Plenipo's side ------------------------------------------------------------------------------

/// The keeper Plenipo started, and every group it was told about that is still running.
struct Keeper {
    program: PathBuf,
    line: Option<Line>,
    groups: BTreeSet<u32>,
}

/// A running keeper and the way in to it.
struct Line {
    child: Child,
    input: ChildStdin,
}

static KEEPER: OnceLock<Mutex<Keeper>> = OnceLock::new();

/// Start the keeper, running `program` (Plenipo's own) with [`SWITCH`]. Plenipo calls this once,
/// as it starts; on Windows it does nothing. Until it is called, supervisors tell no keeper (as
/// in tests that do not test it).
pub fn start(program: &Path) -> std::io::Result<()> {
    if cfg!(windows) {
        return Ok(());
    }
    let line = spawn(program)?;
    let _ = KEEPER.set(Mutex::new(Keeper {
        program: program.to_owned(),
        line: Some(line),
        groups: BTreeSet::new(),
    }));
    Ok(())
}

fn spawn(program: &Path) -> std::io::Result<Line> {
    let mut command = Command::new(program);
    command
        .arg(SWITCH)
        .arg(crate::marks::instance())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // A process group of its own: Ctrl+C in a terminal, or a signal sent to Plenipo's
        // group, never reaches it.
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let input = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("the keeper has no input"))?;
    Ok(Line { child, input })
}

/// A program group the keeper is told about while it runs: listed when made, and taken off the
/// list when dropped (the supervisor holds one for each program it starts).
pub(crate) struct Kept(u32);

impl Kept {
    pub(crate) fn new(group: u32) -> Self {
        tell(group, true);
        Self(group)
    }
}

impl Drop for Kept {
    fn drop(&mut self) {
        tell(self.0, false);
    }
}

/// Tell the keeper, if there is one, that `group` started (`add`) or ended. If the keeper has
/// gone, start a new one and tell it every group still running (ADR-157).
fn tell(group: u32, add: bool) {
    let Some(keeper) = KEEPER.get() else {
        return;
    };
    let mut keeper = keeper.lock().unwrap_or_else(PoisonError::into_inner);
    if add {
        keeper.groups.insert(group);
    } else {
        keeper.groups.remove(&group);
    }
    let sign = if add { '+' } else { '-' };
    let told = keeper.line.as_mut().is_some_and(|line| {
        writeln!(line.input, "{sign}{group}")
            .and_then(|()| line.input.flush())
            .is_ok()
    });
    if told {
        return;
    }
    if let Some(mut gone) = keeper.line.take() {
        let _ = gone.child.kill();
        let _ = gone.child.wait();
    }
    let Ok(mut line) = spawn(&keeper.program) else {
        return;
    };
    let all: String = keeper.groups.iter().map(|g| format!("+{g}\n")).collect();
    if line
        .input
        .write_all(all.as_bytes())
        .and_then(|()| line.input.flush())
        .is_ok()
    {
        keeper.line = Some(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_alone_starts_keeper_mode() {
        let args = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(maybe_run_from_args(args(&["plenipo"])), None);
        assert_eq!(maybe_run_from_args(args(&["plenipo", "--quit"])), None);
        assert_eq!(
            maybe_run_from_args(args(&["plenipo", "--plenipo-keeper-x"])),
            None
        );
        assert_eq!(
            maybe_run_from_args(args(&["plenipo", "--quit", SWITCH])),
            None
        );
    }

    #[test]
    fn an_empty_list_ends_at_once_and_bad_lines_are_skipped() {
        // Nothing listed: no signal is sent and there is no wait.
        let started = std::time::Instant::now();
        let input = "+0\n+1\n+x\nhello\n+42\n-42\n-7\n\n";
        assert_eq!(run(std::io::Cursor::new(input), None), 0);
        assert!(started.elapsed() < GRACE);
        // A mark nobody carries: nothing to end, no wait either.
        let started = std::time::Instant::now();
        let unused = "0123456789abcdef0123456789abcdef";
        assert_eq!(run(std::io::Cursor::new(""), Some(unused)), 0);
        assert!(started.elapsed() < crate::marks::GRACE);
    }
}
