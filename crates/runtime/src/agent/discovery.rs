//! Finding runtime CLIs, running short probes (version, sign-in), and building the child
//! environment for a runtime. Paths come only from these rules — never from the UI.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt as _};

use crate::agent::adapter::{ProbeOutput, RuntimeAdapter};

/// Longest probe output kept per stream.
const MAX_PROBE_OUTPUT: usize = 64 * 1024;

/// The parts of Plenipo's environment that discovery reads. Captured once so tests can
/// supply their own.
#[derive(Debug, Clone, Default)]
pub struct HostEnv {
    pub path: Option<OsString>,
    /// `HOME` (Unix) or `USERPROFILE` (Windows).
    pub home: Option<PathBuf>,
    /// Windows `%APPDATA%` (npm's global prefix lives here).
    pub appdata: Option<PathBuf>,
    /// Values of variables adapters may pass through; everything else is ignored.
    vars: Vec<(String, String)>,
}

impl HostEnv {
    /// Capture the current process environment.
    pub fn current() -> Self {
        let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        Self {
            path: std::env::var_os("PATH"),
            home: std::env::var_os(home_var).map(PathBuf::from),
            appdata: std::env::var_os("APPDATA").map(PathBuf::from),
            vars: std::env::vars().collect(),
        }
    }

    /// An environment with only these locations (tests, tools).
    pub fn new(path: Option<OsString>, home: Option<PathBuf>, appdata: Option<PathBuf>) -> Self {
        Self {
            path,
            home,
            appdata,
            vars: Vec::new(),
        }
    }

    /// Replace the captured variables (tests).
    pub fn with_vars(mut self, vars: Vec<(String, String)>) -> Self {
        self.vars = vars;
        self
    }

    pub fn var(&self, name: &str) -> Option<&str> {
        self.vars
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Absolute PATH entries, in order. Relative entries (`.`, empty) are skipped so the
    /// current directory can never supply an executable.
    pub fn path_dirs(&self) -> Vec<PathBuf> {
        self.path
            .as_ref()
            .map(|p| {
                std::env::split_paths(p)
                    .filter(|d| d.is_absolute())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Where a runtime's CLI was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    Found(PathBuf),
    /// Only launchers Plenipo will not run were found (e.g. Windows npm shims).
    Unsupported(Vec<PathBuf>),
    NotFound,
}

/// Search PATH, then the adapter's well-known locations.
pub fn locate(adapter: &dyn RuntimeAdapter, host: &HostEnv) -> Located {
    let name = adapter.executable_name();
    let mut unusable = Vec::new();
    for dir in host.path_dirs() {
        for candidate in candidates_in(&dir, name) {
            match adapter.resolve(&candidate) {
                Some(exe) if is_runnable(&exe) => return Located::Found(exe),
                _ => unusable.push(candidate),
            }
        }
    }
    for candidate in adapter.known_locations(host) {
        if !candidate.is_file() {
            continue;
        }
        match adapter.resolve(&candidate) {
            Some(exe) if is_runnable(&exe) => return Located::Found(exe),
            _ => unusable.push(candidate),
        }
    }
    if unusable.is_empty() {
        Located::NotFound
    } else {
        Located::Unsupported(unusable)
    }
}

/// Files named like the runtime in `dir`. Windows: `.exe`, then launchers (`.cmd`, `.ps1`,
/// `.bat`) that an adapter may map to a real executable. Unix: the plain name.
fn candidates_in(dir: &Path, name: &str) -> Vec<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        ["exe", "cmd", "ps1", "bat"]
            .iter()
            .map(|ext| format!("{name}.{ext}"))
            .collect()
    } else {
        vec![name.to_owned()]
    };
    names
        .into_iter()
        .map(|n| dir.join(n))
        .filter(|p| p.is_file())
        .collect()
}

fn is_runnable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    }
}

/// Target triple used by npm packages that vendor native binaries.
pub fn npm_target_triple() -> Option<&'static str> {
    use std::env::consts::{ARCH, OS};
    Some(match (OS, ARCH) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => return None,
    })
}

/// The runtime's child environment: declared fixed values plus pass-through variables that
/// are set in Plenipo's environment. The supervisor adds the OS baseline; nothing else from
/// Plenipo's environment (credentials included) reaches the child.
pub fn runtime_env(adapter: &dyn RuntimeAdapter, host: &HostEnv) -> Vec<(String, String)> {
    let mut env = adapter.fixed_env();
    for name in adapter.passthrough_env() {
        if env.iter().any(|(k, _)| k == name) {
            continue;
        }
        if let Some(value) = host.var(name) {
            env.push((name.to_owned(), value.to_owned()));
        }
    }
    env
}

/// Run a short command (version or sign-in status) in its own process tree with the given
/// environment. Never fails: problems are described in the output.
pub async fn run_probe(
    executable: &Path,
    args: &[String],
    env: &[(String, String)],
    working_dir: &Path,
    timeout: Duration,
) -> ProbeOutput {
    let mut command =
        crate::supervisor::wrapped_command(executable, args, env, working_dir, Stdio::null());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return ProbeOutput {
                spawn_error: Some(e.to_string()),
                ..ProbeOutput::default()
            }
        }
    };
    let stdout = child.stdout().take();
    let stderr = child.stderr().take();
    let out = tokio::spawn(read_capped(stdout));
    let err = tokio::spawn(read_capped(stderr));
    let status = tokio::time::timeout(timeout, child.wait()).await;
    let timed_out = status.is_err();
    if timed_out {
        let _ = child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
    }
    let collect = |h: tokio::task::JoinHandle<String>| async move {
        tokio::time::timeout(Duration::from_secs(5), h)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default()
    };
    ProbeOutput {
        exit_code: status.ok().and_then(Result::ok).and_then(|s| s.code()),
        stdout: collect(out).await,
        stderr: collect(err).await,
        timed_out,
        spawn_error: None,
    }
}

async fn read_capped<R: AsyncRead + Unpin>(reader: Option<R>) -> String {
    let Some(mut reader) = reader else {
        return String::new();
    };
    let mut kept = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            // Keep reading past the cap so the child never blocks on a full pipe.
            Ok(n) => {
                let room = MAX_PROBE_OUTPUT.saturating_sub(kept.len());
                kept.extend_from_slice(&chunk[..n.min(room)]);
            }
        }
    }
    String::from_utf8_lossy(&kept).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_entries_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let joined =
            std::env::join_paths([PathBuf::from("."), PathBuf::from("rel"), dir.path().into()])
                .unwrap();
        let host = HostEnv {
            path: Some(joined),
            ..HostEnv::default()
        };
        assert_eq!(host.path_dirs(), [dir.path().to_path_buf()]);
    }

    #[test]
    fn host_vars_lookup() {
        let host = HostEnv::default().with_vars(vec![("A".into(), "1".into())]);
        assert_eq!(host.var("A"), Some("1"));
        assert_eq!(host.var("B"), None);
    }
}
