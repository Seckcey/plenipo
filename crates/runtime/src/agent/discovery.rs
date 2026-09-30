//! Finding runtime CLIs, running short probes (version, sign-in), and building the child
//! environment for a runtime. Paths come only from these rules — never from the UI.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt as _};

use crate::agent::adapter::{Framing, ProbeOutput, RuntimeAdapter};

/// Longest probe output kept per stream.
const MAX_PROBE_OUTPUT: usize = 64 * 1024;
/// The longest line kept from a tool's talk check (a longer one is read and cut, so a tool that
/// never ends its line cannot fill Plenipo's memory).
const MAX_TALK_LINE: usize = 1024 * 1024;

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
    /// Machine-wide install directories (e.g. `/usr/local/bin`); empty for test hosts.
    system_dirs: Vec<PathBuf>,
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
            system_dirs: if cfg!(unix) {
                vec!["/usr/local/bin".into(), "/opt/homebrew/bin".into()]
            } else {
                Vec::new()
            },
        }
    }

    /// An environment with only these locations (tests, tools): no machine-wide directories.
    pub fn new(path: Option<OsString>, home: Option<PathBuf>, appdata: Option<PathBuf>) -> Self {
        Self {
            path,
            home,
            appdata,
            vars: Vec::new(),
            system_dirs: Vec::new(),
        }
    }

    pub fn system_dirs(&self) -> &[PathBuf] {
        &self.system_dirs
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
    run_probe_with(executable, args, env, working_dir, None, timeout).await
}

/// [`run_probe`], with `input` written to the program's standard input, which is then closed (a
/// paid AI tool's key check gets its key there, ADR-085: never an argument or a variable).
pub async fn run_probe_with(
    executable: &Path,
    args: &[String],
    env: &[(String, String)],
    working_dir: &Path,
    input: Option<&[u8]>,
    timeout: Duration,
) -> ProbeOutput {
    let stdin = if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    let mut command = crate::supervisor::wrapped_command(executable, args, env, working_dir, stdin);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return ProbeOutput {
                spawn_error: Some(e.to_string()),
                ..ProbeOutput::default()
            }
        }
    };
    if let (Some(bytes), Some(mut pipe)) = (input, child.stdin().take()) {
        use tokio::io::AsyncWriteExt as _;
        let bytes = bytes.to_vec();
        tokio::spawn(async move {
            let _ = pipe.write_all(&bytes).await;
            let _ = pipe.shutdown().await;
        });
    }
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

/// Talk to an AI tool for a moment (ADR-060): write `lines` (JSON-RPC messages, framed as
/// `framing` says) to its standard input, read its answers until each request numbered in
/// `answers` has one (or `timeout` passes), then close its input and let it end. Nothing else
/// is written: the tool's own requests, if any, are never answered. Never fails: problems are
/// described in the output, whose `stdout` holds the messages the tool wrote, one per line
/// whatever the framing.
#[allow(clippy::too_many_arguments)]
pub async fn run_talk(
    executable: &Path,
    args: &[String],
    env: &[(String, String)],
    working_dir: &Path,
    lines: &[String],
    answers: &[u64],
    framing: Framing,
    timeout: Duration,
) -> ProbeOutput {
    use tokio::io::{AsyncWriteExt as _, BufReader};

    let mut command =
        crate::supervisor::wrapped_command(executable, args, env, working_dir, Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return ProbeOutput {
                spawn_error: Some(e.to_string()),
                ..ProbeOutput::default()
            }
        }
    };
    let mut stdin = child.stdin().take();
    let stdout = child.stdout().take();
    let err = tokio::spawn(read_capped(child.stderr().take()));
    // What the tool wrote, kept even when the time runs out (the answers it gave still count).
    let mut kept = String::new();
    let talk = async {
        if let Some(input) = stdin.as_mut() {
            for line in lines {
                let framed = match framing {
                    Framing::Lines => format!("{line}\n"),
                    Framing::Headers => format!("Content-Length: {}\r\n\r\n{line}", line.len()),
                };
                let sent =
                    input.write_all(framed.as_bytes()).await.is_ok() && input.flush().await.is_ok();
                if !sent {
                    break;
                }
            }
        }
        let mut waiting: Vec<u64> = answers.to_vec();
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout);
            while !waiting.is_empty() {
                let next = match framing {
                    Framing::Lines => capped_line(&mut reader, MAX_TALK_LINE).await,
                    Framing::Headers => framed_message(&mut reader, MAX_TALK_LINE).await,
                };
                let Some(line) = next else {
                    break;
                };
                let answered = serde_json::from_str::<serde_json::Value>(line.trim())
                    .ok()
                    .filter(|v| v.get("result").is_some() || v.get("error").is_some())
                    .and_then(|v| v.get("id").and_then(serde_json::Value::as_u64))
                    .filter(|id| waiting.contains(id));
                // An answer Plenipo waits for is always kept; anything else while there is room.
                if answered.is_some() || kept.len() + line.len() < MAX_PROBE_OUTPUT {
                    kept.push_str(&line);
                    kept.push('\n');
                }
                if let Some(id) = answered {
                    waiting.retain(|w| *w != id);
                }
            }
        }
    };
    let timed_out = tokio::time::timeout(timeout, talk).await.is_err();
    let stdout = kept;
    // Done: the way in closes, so the tool can end by itself; if it does not, it is ended.
    drop(stdin);
    let status = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
    let exit_code = match status {
        Ok(Ok(s)) => s.code(),
        _ => {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
            None
        }
    };
    let stderr = tokio::time::timeout(Duration::from_secs(5), err)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    ProbeOutput {
        exit_code,
        stdout,
        stderr,
        timed_out,
        spawn_error: None,
    }
}

/// One message framed by a `Content-Length` header (ADR-083), on one line (its line ends made
/// spaces, which JSON allows between values); a message longer than `max` bytes is read and
/// dropped (an empty line). `None` at the end of the output, or when the headers are not ones
/// Plenipo reads.
async fn framed_message<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Option<String> {
    let mut length: Option<usize> = None;
    loop {
        let header = capped_line(reader, 1024).await?;
        let header = header.trim();
        if header.is_empty() {
            // Blank lines before a message's headers are skipped.
            if length.is_some() {
                break;
            }
            continue;
        }
        // Only `Name: value` lines are headers; anything else (a stray line the tool printed,
        // or the body of a message that had no length) is skipped, so the next message's
        // headers are still found.
        let Some((name, value)) = header.split_once(':').filter(|(n, _)| {
            !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        }) else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            length = Some(value.trim().parse().ok()?);
        }
    }
    let length = length?;
    let mut body = vec![0u8; length.min(max)];
    reader.read_exact(&mut body).await.ok()?;
    if length > max {
        // Too long to keep: read to its end, keep nothing.
        let mut rest = (&mut *reader).take((length - max) as u64);
        tokio::io::copy(&mut rest, &mut tokio::io::sink())
            .await
            .ok()?;
        return Some(String::new());
    }
    Some(String::from_utf8_lossy(&body).replace(['\r', '\n'], " "))
}

/// One line from `reader`, without its end; at most `max` bytes of it are kept (the rest of a
/// longer line is read and dropped). `None` at the end of the output.
async fn capped_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Option<String> {
    use tokio::io::AsyncBufReadExt as _;
    let mut line = Vec::new();
    let mut any = false;
    loop {
        let (used, ended) = {
            let Ok(buf) = reader.fill_buf().await else {
                break;
            };
            if buf.is_empty() {
                break;
            }
            any = true;
            let end = buf.iter().position(|b| *b == b'\n');
            let chunk = &buf[..end.unwrap_or(buf.len())];
            let room = max.saturating_sub(line.len());
            line.extend_from_slice(&chunk[..chunk.len().min(room)]);
            (end.map_or(buf.len(), |e| e + 1), end.is_some())
        };
        reader.consume(used);
        if ended {
            break;
        }
    }
    any.then(|| {
        String::from_utf8_lossy(&line)
            .trim_end_matches('\r')
            .to_owned()
    })
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
    #[tokio::test]
    async fn framed_messages_are_found_past_stray_lines() {
        let body = r#"{"jsonrpc":"2.0","id":2,"result":{"ok":true}}"#;
        let input = format!(
            "a banner line\r\nContent-Type: application/json\r\n\r\n{{\"lost\":1}}\r\n\
             Content-Length: {}\r\n\r\n{body}Content-Length: 2\r\n\r\n{{}}",
            body.len()
        );
        let mut reader = tokio::io::BufReader::new(input.as_bytes());
        assert_eq!(
            super::framed_message(&mut reader, 1024).await.as_deref(),
            Some(body)
        );
        assert_eq!(
            super::framed_message(&mut reader, 1024).await.as_deref(),
            Some("{}")
        );
        assert_eq!(super::framed_message(&mut reader, 1024).await, None);
        // Too long to keep: read past, kept as nothing, and the next one still found.
        let long = "Content-Length: 5\r\n\r\nabcdeContent-Length: 2\r\n\r\n{}";
        let mut reader = tokio::io::BufReader::new(long.as_bytes());
        assert_eq!(
            super::framed_message(&mut reader, 3).await.as_deref(),
            Some("")
        );
        assert_eq!(
            super::framed_message(&mut reader, 3).await.as_deref(),
            Some("{}")
        );
    }

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
