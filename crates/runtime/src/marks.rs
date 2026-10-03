//! Programs that leave their group (Phase 23, ADR-158). On a Mac and Linux a worker's program
//! leads a process group, and Plenipo (or the keeper, after a crash, ADR-157) ends the whole
//! group. A program can start a session of its own (`setsid`, a daemon such as a build tool's
//! server, `tmux`), which takes it out of the group; Windows' job object would still hold it.
//!
//! So every program Plenipo starts on a Mac and Linux carries a mark in its environment,
//! `PLENIPO_RUN=<this Plenipo>/<the run>`, which the programs it starts inherit. When a run ends
//! or is stopped, Plenipo ends any of the owner's programs still carrying that run's mark; when
//! Plenipo itself goes away, the keeper ends every program carrying this Plenipo's mark. A
//! program that clears its own environment is not found, and the tool server never serves it
//! either: it no longer descends from the AI tool (ADR-034).

use std::sync::OnceLock;
use std::time::Duration;

/// The variable that carries the mark.
pub const MARK: &str = "PLENIPO_RUN";
/// How long a marked program has to stop when asked, before it is ended for certain.
pub const GRACE: Duration = Duration::from_secs(1);

/// This Plenipo's own part of every mark: random, so that two Plenipos, or one after a restart,
/// never end each other's programs.
pub fn instance() -> &'static str {
    static INSTANCE: OnceLock<String> = OnceLock::new();
    INSTANCE.get_or_init(|| uuid::Uuid::new_v4().simple().to_string())
}

/// Whether `text` could be an instance (what [`instance`] makes: 32 lowercase hex digits).
pub fn is_instance(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The mark for one run: `<instance>/<run>`.
pub fn for_run(run: &str) -> String {
    format!("{}/{run}", instance())
}

/// Whether `mark` belongs to `instance` (any of its runs).
pub fn of_instance(mark: &str, instance: &str) -> bool {
    mark.strip_prefix(instance)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// End every program of this user, other than this one, whose mark `matches`: asked to stop
/// first, then, `grace` later, ended for certain if it still carries the mark. Returns how many
/// were asked. The system lets Plenipo signal only the owner's own programs, and only those
/// whose environment it can read.
#[cfg(unix)]
pub fn end_marked(matches: impl Fn(&str) -> bool, grace: Duration) -> usize {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, Signal, System, UpdateKind};

    let own = std::process::id();
    let refresh = |system: &mut System| {
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_user(UpdateKind::Always)
                .with_environ(UpdateKind::Always),
        );
    };
    let marked = |system: &System| -> Vec<sysinfo::Pid> {
        let me = system
            .process(sysinfo::Pid::from_u32(own))
            .and_then(sysinfo::Process::user_id)
            .cloned();
        system
            .processes()
            .values()
            .filter(|p| p.pid().as_u32() != own && p.pid().as_u32() > 1)
            .filter(|p| me.is_some() && p.user_id() == me.as_ref())
            .filter(|p| {
                p.environ().iter().any(|entry| {
                    entry
                        .to_str()
                        .and_then(|e| e.strip_prefix(MARK))
                        .and_then(|e| e.strip_prefix('='))
                        .is_some_and(&matches)
                })
            })
            .map(sysinfo::Process::pid)
            .collect()
    };
    let mut system = System::new();
    refresh(&mut system);
    let found = marked(&system);
    if found.is_empty() {
        return 0;
    }
    for pid in &found {
        if let Some(p) = system.process(*pid) {
            let _ = p.kill_with(Signal::Term);
        }
    }
    std::thread::sleep(grace);
    refresh(&mut system);
    for pid in marked(&system) {
        if let Some(p) = system.process(pid) {
            let _ = p.kill_with(Signal::Kill);
        }
    }
    found.len()
}

/// Windows' job object holds every program a worker starts, so nothing is marked there.
#[cfg(not(unix))]
pub fn end_marked(_matches: impl Fn(&str) -> bool, _grace: Duration) -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_name_this_plenipo_and_the_run() {
        let me = instance();
        assert!(is_instance(me), "{me}");
        assert_eq!(instance(), me, "one instance for the life of the program");
        let mark = for_run("abc-123");
        assert_eq!(mark, format!("{me}/abc-123"));
        assert!(of_instance(&mark, me));
        assert!(!of_instance(&format!("{me}x/abc"), me));
        assert!(!of_instance(me, me));
        assert!(!of_instance("other/abc", me));
        for bad in ["", "ABCDEF0123456789abcdef0123456789", "0123", "../etc"] {
            assert!(!is_instance(bad), "{bad}");
        }
    }

    /// Nothing carries a mark nobody made: the sweep finds nothing and does not wait.
    #[test]
    fn an_unused_mark_ends_nothing_at_once() {
        let started = std::time::Instant::now();
        assert_eq!(end_marked(|m| m == "no-such-mark/at-all", GRACE), 0);
        assert!(started.elapsed() < GRACE);
    }
}
