//! Programs a worker runs: commands, PowerShell scripts, and git. Each runs through the
//! supervisor like every other program Plenipo starts: its own process tree, a cleared
//! environment, a time limit, cancellation, and a recorded run. Never through a shell.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use plenipo_runtime::{ExecutionState, LaunchSpec, Supervisor};
use tokio::sync::mpsc;

/// Output returned to the worker (the start and the end are kept when it is longer).
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const HEAD_BYTES: usize = 16 * 1024;

/// Variables from Plenipo's own environment that development programs need, passed on when
/// set. None of them is a credential; everything else is withheld.
pub const DEV_ENV: &[&str] = &[
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "GOPATH",
    "GOROOT",
    "GOCACHE",
    "GOMODCACHE",
    "JAVA_HOME",
    "GRADLE_USER_HOME",
    "M2_HOME",
    "MAVEN_HOME",
    "NODE_PATH",
    "NVM_DIR",
    "NVM_HOME",
    "NVM_SYMLINK",
    "PNPM_HOME",
    "VOLTA_HOME",
    "VIRTUAL_ENV",
    "CONDA_PREFIX",
    "PYENV_ROOT",
    "DOTNET_ROOT",
    "LOCALAPPDATA",
    "APPDATA",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "PROGRAMDATA",
    "TERM",
    "LOGNAME",
];

/// What a finished program left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub execution_id: String,
    pub state: ExecutionState,
    pub exit_code: Option<i32>,
    pub output: String,
    pub duration_ms: u64,
}

impl Ran {
    pub fn succeeded(&self) -> bool {
        self.state == ExecutionState::Succeeded
    }

    /// How it ended, in a line.
    pub fn ending(&self, timeout: Duration) -> String {
        let secs = self.duration_ms as f64 / 1000.0;
        match self.state {
            ExecutionState::Succeeded => format!("Finished (exit code 0) in {secs:.1}s."),
            ExecutionState::Failed => match self.exit_code {
                Some(c) => format!("Failed with exit code {c} after {secs:.1}s."),
                None => format!("Failed after {secs:.1}s."),
            },
            ExecutionState::TimedOut => format!(
                "Stopped: it reached its time limit of {} seconds.",
                timeout.as_secs()
            ),
            ExecutionState::Cancelled => {
                "Stopped before it finished (the task ended or its permissions were revoked)."
                    .into()
            }
            other => format!("Ended ({other:?})."),
        }
    }
}

/// Everything a program run needs.
pub struct Run<'a> {
    pub label: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub working_dir: &'a Path,
    pub env: Vec<(String, String)>,
    pub stdin: Option<Vec<u8>>,
    pub timeout: Duration,
}

/// Find `program` on Plenipo's PATH: a bare name only (`cargo`, `npm`). On Windows the
/// extensions Windows runs directly are tried (`.exe`, `.cmd`, `.bat`, `.com`) — never
/// PowerShell scripts.
pub fn find_on_path(program: &str) -> Option<PathBuf> {
    find_in(program, std::env::var_os("PATH"))
}

pub fn find_in(program: &str, path: Option<OsString>) -> Option<PathBuf> {
    if program.contains(['/', '\\']) || program.is_empty() {
        return None;
    }
    let names: Vec<String> = if cfg!(windows) {
        let lower = program.to_ascii_lowercase();
        if [".exe", ".cmd", ".bat", ".com"]
            .iter()
            .any(|e| lower.ends_with(e))
        {
            vec![program.to_owned()]
        } else {
            [".exe", ".cmd", ".bat", ".com"]
                .iter()
                .map(|e| format!("{program}{e}"))
                .collect()
        }
    } else {
        vec![program.to_owned()]
    };
    for dir in std::env::split_paths(&path?) {
        if !dir.is_absolute() {
            continue;
        }
        for n in &names {
            let candidate = dir.join(n);
            if is_runnable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(unix)]
fn is_runnable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_runnable(path: &Path) -> bool {
    path.is_file()
}

/// Development variables passed on from Plenipo's environment (see [`DEV_ENV`]).
pub fn dev_env() -> Vec<(String, String)> {
    DEV_ENV
        .iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| ((*k).to_owned(), v)))
        .collect()
}

/// Keep the start and the end of long output.
fn clip(mut text: String) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text;
    }
    let mut head = HEAD_BYTES;
    while !text.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = text.len() - (MAX_OUTPUT_BYTES - HEAD_BYTES);
    while !text.is_char_boundary(tail) {
        tail += 1;
    }
    let omitted = tail - head;
    let end = text.split_off(tail);
    text.truncate(head);
    format!("{text}\n… {omitted} bytes of output omitted …\n{end}")
}

/// Run a program to its end and collect its output. `started` receives the execution ID as
/// soon as it exists (so it can be stopped).
pub async fn run(
    supervisor: &Supervisor,
    run: Run<'_>,
    started: impl FnOnce(&str),
) -> Result<Ran, String> {
    let executable = supervisor
        .allow_executable(&run.executable)
        .map_err(|e| format!("{} cannot be run: {e}", run.executable.display()))?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let record = supervisor
        .launch(LaunchSpec {
            profile_id: "capability.program".into(),
            label: run.label,
            executable,
            args: run.args,
            env: run.env,
            working_dir: run.working_dir.to_path_buf(),
            max_runtime: run.timeout,
            stdin: run.stdin,
            max_line_bytes: Some(64 * 1024),
            observer: Some(tx),
            agent: None,
        })
        .await
        .map_err(|e| format!("the program could not be started: {e}"))?;
    started(&record.id);
    let mut output = String::new();
    let mut dropped = 0usize;
    while let Some(line) = rx.recv().await {
        // Keep memory bounded: beyond four times the returned size, only the tail matters.
        if output.len() > 4 * MAX_OUTPUT_BYTES {
            let mut cut = output.len() - 2 * MAX_OUTPUT_BYTES;
            while !output.is_char_boundary(cut) {
                cut += 1;
            }
            dropped += cut;
            output.drain(..cut);
        }
        output.push_str(&line.text);
        output.push('\n');
    }
    let done = supervisor
        .wait(&record.id)
        .await
        .map_err(|e| format!("the program's end could not be read: {e}"))?;
    let mut output = clip(output);
    if dropped > 0 && !output.contains("bytes of output omitted") {
        output = format!("… earlier output omitted …\n{output}");
    }
    Ok(Ran {
        execution_id: record.id,
        state: done.state,
        exit_code: done.exit_code,
        output,
        duration_ms: done
            .ended_at
            .unwrap_or(done.started_at)
            .saturating_sub(done.started_at),
    })
}

/// PowerShell on this computer: Windows PowerShell on Windows, `pwsh` elsewhere (or on
/// Windows when Windows PowerShell is missing).
pub fn powershell() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Some(root) = std::env::var_os("SystemRoot") {
            let p = PathBuf::from(root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    find_on_path("pwsh")
}

/// Arguments that make PowerShell read the script from stdin without a profile or prompts.
pub fn powershell_args() -> Vec<String> {
    [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        "-",
    ]
    .map(String::from)
    .to_vec()
}

/// Git's fixed options: no pager, no colors, never prompt for a password.
pub fn git_args(workspace: &Path, op: &[String]) -> Vec<String> {
    let mut args = vec![
        "-C".to_owned(),
        workspace.display().to_string(),
        "--no-pager".into(),
        "-c".into(),
        "color.ui=false".into(),
        "-c".into(),
        "core.quotepath=false".into(),
    ];
    args.extend(op.iter().cloned());
    args
}

/// Variables git gets so it never waits for input.
pub fn git_env() -> Vec<(String, String)> {
    vec![
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GCM_INTERACTIVE".into(), "never".into()),
        ("GIT_OPTIONAL_LOCKS".into(), "0".into()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_output_keeps_both_ends() {
        let text = format!("{}{}", "a".repeat(100_000), "END");
        let clipped = clip(text);
        assert!(clipped.len() < 70_000);
        assert!(clipped.starts_with("aaa") && clipped.ends_with("END"));
        assert!(clipped.contains("bytes of output omitted"));
        assert_eq!(clip("short".into()), "short");
    }

    #[test]
    fn path_lookup_takes_bare_names_only() {
        let dir = tempfile::tempdir().unwrap();
        let name = if cfg!(windows) { "tool.exe" } else { "tool" };
        let file = dir.path().join(name);
        std::fs::write(&file, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = Some(dir.path().as_os_str().to_owned());
        assert_eq!(find_in("tool", path.clone()), Some(file));
        assert_eq!(find_in("missing", path.clone()), None);
        assert_eq!(find_in("../tool", path.clone()), None);
        assert_eq!(find_in("", path), None);
    }
}
