//! Harmless diagnostic child-process scenarios.
//!
//! Plenipo runs its own executable with `--plenipo-diagnostic=<scenario>` to exercise the
//! supervisor end to end without depending on any external program. These scenarios only
//! print text, sleep, and exit; `tree` additionally starts one copy of itself.

use std::io::Write as _;
use std::time::Duration;

pub const FLAG: &str = "--plenipo-diagnostic=";
pub const GREETING_VAR: &str = "PLENIPO_DIAGNOSTIC_GREETING";

/// Safety cap so a diagnostic child can never run forever on its own.
const MAX_HEARTBEATS: u32 = 1200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    /// stdout + stderr lines over ~2s, prints the greeting variable, exits 0.
    Echo,
    /// stdout line, stderr error, exits 3.
    Failure,
    /// Heartbeat every 500 ms until killed (capped at 10 minutes).
    LongRunning,
    /// Prints the names of all environment variables it received, exits 0.
    Env,
    /// Starts a long-running copy of itself, prints its PID, then heartbeats.
    Tree,
    /// 5,000 fast lines plus one 100 KB line, exits 0.
    Burst,
    /// Reads stdin to the end, echoes each line as `stdin: <line>`, then the byte count.
    Stdin,
}

impl Scenario {
    pub fn name(self) -> &'static str {
        match self {
            Self::Echo => "echo",
            Self::Failure => "failure",
            Self::LongRunning => "long-running",
            Self::Env => "env",
            Self::Tree => "tree",
            Self::Burst => "burst",
            Self::Stdin => "stdin",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Echo,
            Self::Failure,
            Self::LongRunning,
            Self::Env,
            Self::Tree,
            Self::Burst,
            Self::Stdin,
        ]
        .into_iter()
        .find(|s| s.name() == name)
    }
}

/// Command-line argument selecting `scenario`.
pub fn arg(scenario: Scenario) -> String {
    format!("{FLAG}{}", scenario.name())
}

/// If `args` request a diagnostic scenario, run it and return the exit code.
/// Returns `None` for a normal launch. Unknown scenarios exit 64.
pub fn maybe_run_from_args(args: impl IntoIterator<Item = String>) -> Option<i32> {
    let requested = args.into_iter().nth(1)?;
    let name = requested.strip_prefix(FLAG)?;
    Some(match Scenario::parse(name) {
        Some(scenario) => run(scenario),
        None => {
            eprintln!("unknown diagnostic scenario: {name}");
            64
        }
    })
}

pub fn run(scenario: Scenario) -> i32 {
    match scenario {
        Scenario::Echo => echo(),
        Scenario::Failure => {
            out("starting simulated work");
            err("error: simulated failure");
            3
        }
        Scenario::LongRunning => heartbeat("heartbeat"),
        Scenario::Env => {
            let mut names: Vec<String> = std::env::vars_os()
                .map(|(k, _)| k.to_string_lossy().into_owned())
                .collect();
            names.sort();
            for name in names {
                out(&format!("env:{name}"));
            }
            0
        }
        Scenario::Tree => tree(),
        Scenario::Burst => {
            for i in 1..=5000 {
                out(&format!("burst line {i}"));
            }
            out(&"x".repeat(100_000));
            out("burst done");
            0
        }
        Scenario::Stdin => {
            let mut input = String::new();
            if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut input) {
                err(&format!("cannot read stdin: {e}"));
                return 1;
            }
            for line in input.lines() {
                out(&format!("stdin: {line}"));
            }
            out(&format!("stdin bytes: {}", input.len()));
            0
        }
    }
}

fn out(line: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{line}");
    let _ = stdout.flush();
}

fn err(line: &str) {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{line}");
    let _ = stderr.flush();
}

fn echo() -> i32 {
    match std::env::var(GREETING_VAR) {
        Ok(greeting) => out(&format!("greeting: {greeting}")),
        Err(_) => out("greeting: (not provided)"),
    }
    for i in 1..=10 {
        out(&format!("stdout line {i} of 10"));
        if i % 3 == 0 {
            err(&format!("stderr notice {i}"));
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    out("echo complete");
    0
}

fn heartbeat(label: &str) -> i32 {
    for i in 1..=MAX_HEARTBEATS {
        out(&format!("{label} {i}"));
        std::thread::sleep(Duration::from_millis(500));
    }
    0
}

fn tree() -> i32 {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            err(&format!("cannot locate executable: {e}"));
            return 1;
        }
    };
    match std::process::Command::new(exe)
        .arg(arg(Scenario::LongRunning))
        .spawn()
    {
        Ok(child) => {
            out(&format!("child-pid:{}", child.id()));
            heartbeat("parent heartbeat")
        }
        Err(e) => {
            err(&format!("cannot start child: {e}"));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_names_round_trip() {
        for s in [
            Scenario::Echo,
            Scenario::Failure,
            Scenario::LongRunning,
            Scenario::Env,
            Scenario::Tree,
            Scenario::Burst,
            Scenario::Stdin,
        ] {
            assert_eq!(Scenario::parse(s.name()), Some(s));
        }
        assert_eq!(Scenario::parse("rm -rf"), None);
    }

    #[test]
    fn normal_launch_is_not_diagnostic() {
        assert_eq!(maybe_run_from_args(["app".to_string()]), None);
        assert_eq!(
            maybe_run_from_args(["app".to_string(), "--other".to_string()]),
            None
        );
    }

    #[test]
    fn unknown_scenario_exits_64() {
        assert_eq!(
            maybe_run_from_args(["app".to_string(), format!("{FLAG}nope")]),
            Some(64)
        );
    }
}
