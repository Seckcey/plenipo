//! Plenipo's log files (Phase 13): what Plenipo itself did and what went wrong, kept in
//! `logs\plenipo.log` in its own folder so a problem can be looked into after the fact.
//!
//! - **Rotation:** a file stops at [`MAX_FILE_BYTES`]; it then becomes `plenipo.1.log`, the one
//!   before it `plenipo.2.log`, and so on, and only [`KEEP_FILES`] files are kept.
//! - **Never a secret:** every line passes the secret filter first (recognizable secrets from
//!   the start, and the values of the owner's stored secrets once the Vault is open).
//! - **Never what the owner types:** nothing that carries terminal input, task text, or a
//!   program's output is logged; lines are Plenipo's own messages.
//!
//! Crates log through the `log` crate; the desktop app installs [`install`] as its logger.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// The current log file's name.
pub const FILE_NAME: &str = "plenipo.log";
/// A log file is rotated when it reaches this size.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// How many log files are kept, the current one included.
pub const KEEP_FILES: usize = 5;
/// A single line longer than this is cut (a runaway message never fills the file).
const MAX_LINE_CHARS: usize = 4_000;

/// Hides secrets in a line before it is written.
pub type Filter = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// The log files in one folder.
pub struct LogFiles {
    dir: PathBuf,
    max_bytes: u64,
    keep: usize,
    writer: Mutex<Writer>,
    filter: RwLock<Filter>,
    /// Also print each line (debug builds, which have a console).
    echo: bool,
}

#[derive(Default)]
struct Writer {
    file: Option<File>,
    size: u64,
    /// A rotation failed (another program has the file open): not tried again before this.
    rotate_after: Option<std::time::Instant>,
}

/// After a failed rotation, how long to keep writing to the current file before trying again.
const ROTATE_RETRY: std::time::Duration = std::time::Duration::from_secs(60);

/// Only recognizable secrets, until the Vault's filter replaces it.
fn pattern_filter() -> Filter {
    let redactor = plenipo_guard::redact::Redactor::new(Vec::new());
    Arc::new(move |text: &str| redactor.redact(text).into_owned())
}

impl LogFiles {
    /// Open (or start) the log files in `dir`.
    pub fn open(dir: &Path) -> io::Result<Self> {
        Self::with_limits(dir, MAX_FILE_BYTES, KEEP_FILES)
    }

    pub fn with_limits(dir: &Path, max_bytes: u64, keep: usize) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let logs = Self {
            dir: dir.to_path_buf(),
            max_bytes: max_bytes.max(1),
            keep: keep.max(1),
            writer: Mutex::new(Writer::default()),
            filter: RwLock::new(pattern_filter()),
            echo: cfg!(debug_assertions),
        };
        let mut writer = logs.lock();
        logs.reopen(&mut writer)?;
        if writer.size >= logs.max_bytes {
            logs.rotate(&mut writer)?;
        }
        drop(writer);
        Ok(logs)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Writer> {
        self.writer.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The folder the log files are in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Use `filter` for every line from now on (the Vault's, which also knows the owner's
    /// stored secrets).
    pub fn set_filter(&self, filter: Filter) {
        *self.filter.write().unwrap_or_else(|p| p.into_inner()) = filter;
    }

    /// The log files there are, the current one first.
    pub fn files(&self) -> Vec<PathBuf> {
        (0..self.keep)
            .map(|n| self.path(n))
            .filter(|p| p.is_file())
            .collect()
    }

    fn path(&self, n: usize) -> PathBuf {
        if n == 0 {
            self.dir.join(FILE_NAME)
        } else {
            self.dir.join(format!("plenipo.{n}.log"))
        }
    }

    fn reopen(&self, writer: &mut Writer) -> io::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path(0))?;
        writer.size = file.metadata().map(|m| m.len()).unwrap_or(0);
        writer.file = Some(file);
        Ok(())
    }

    /// `plenipo.log` becomes `plenipo.1.log`, and so on; the oldest is deleted. The current
    /// file moves first: when it cannot (another program has it open on Windows), nothing is
    /// deleted or moved, and writing goes on in the current file.
    fn rotate(&self, writer: &mut Writer) -> io::Result<()> {
        writer.file = None;
        let moving = self.dir.join(format!("{FILE_NAME}.rotating"));
        if let Err(e) = fs::rename(self.path(0), &moving) {
            self.reopen(writer)?;
            return Err(e);
        }
        if self.keep > 1 {
            let _ = fs::remove_file(self.path(self.keep - 1));
            for n in (1..self.keep - 1).rev() {
                let from = self.path(n);
                if from.exists() {
                    let _ = fs::rename(&from, self.path(n + 1));
                }
            }
            let _ = fs::rename(&moving, self.path(1));
        }
        let _ = fs::remove_file(&moving);
        self.reopen(writer)
    }

    /// Write one line: time, level, where it came from, and the message with secrets hidden.
    pub fn write(&self, level: Level, target: &str, message: &str) {
        let filter = Arc::clone(&self.filter.read().unwrap_or_else(|p| p.into_inner()));
        let mut text = filter(message).replace(['\r', '\n'], " ⏎ ");
        if text.chars().count() > MAX_LINE_CHARS {
            text = text.chars().take(MAX_LINE_CHARS).collect::<String>() + " …(cut)";
        }
        let when = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
        let line = format!("{when} {level:<5} {target}: {text}\n");
        if self.echo {
            eprint!("{line}");
        }
        let mut writer = self.lock();
        if writer.file.is_none() && self.reopen(&mut writer).is_err() {
            return;
        }
        let may_rotate = writer
            .rotate_after
            .is_none_or(|at| std::time::Instant::now() >= at);
        if writer.size + line.len() as u64 > self.max_bytes && writer.size > 0 && may_rotate {
            match self.rotate(&mut writer) {
                Ok(()) => writer.rotate_after = None,
                Err(e) => {
                    if self.echo {
                        eprintln!("[plenipo] could not rotate the log files: {e}");
                    }
                    writer.rotate_after = Some(std::time::Instant::now() + ROTATE_RETRY);
                    if writer.file.is_none() {
                        return;
                    }
                }
            }
        }
        if let Some(file) = writer.file.as_mut() {
            if file.write_all(line.as_bytes()).is_ok() {
                writer.size += line.len() as u64;
            }
        }
    }

    pub fn flush(&self) {
        if let Some(file) = self.lock().file.as_mut() {
            let _ = file.flush();
        }
    }
}

/// Plenipo's own messages at Info and above; other libraries' warnings and errors only.
fn wanted(metadata: &Metadata<'_>) -> bool {
    metadata.level() <= Level::Warn
        || (metadata.level() <= Level::Info && metadata.target().starts_with("plenipo"))
}

struct Logger(Arc<LogFiles>);

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        wanted(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            self.0
                .write(record.level(), record.target(), &record.args().to_string());
        }
    }

    fn flush(&self) {
        self.0.flush();
    }
}

static INSTALLED: OnceLock<Arc<LogFiles>> = OnceLock::new();

/// Make the log files in `dir` Plenipo's log. Only the first call installs; later calls (and
/// a folder that cannot be used) return what is installed, if anything.
pub fn install(dir: &Path) -> Option<Arc<LogFiles>> {
    if let Some(logs) = INSTALLED.get() {
        return Some(Arc::clone(logs));
    }
    let logs = match LogFiles::open(dir) {
        Ok(logs) => Arc::new(logs),
        Err(e) => {
            eprintln!(
                "[plenipo] the log files in {} cannot be used: {e}",
                dir.display()
            );
            return None;
        }
    };
    if log::set_boxed_logger(Box::new(Logger(Arc::clone(&logs)))).is_ok() {
        log::set_max_level(LevelFilter::Info);
        let _ = INSTALLED.set(Arc::clone(&logs));
        Some(logs)
    } else {
        INSTALLED.get().cloned()
    }
}

/// The installed log files, if any.
pub fn installed() -> Option<Arc<LogFiles>> {
    INSTALLED.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn lines_are_written_with_the_time_level_and_source() {
        let dir = tempfile::tempdir().unwrap();
        let logs = LogFiles::open(dir.path()).unwrap();
        logs.write(
            Level::Warn,
            "plenipo_runtime::supervisor",
            "a program stopped",
        );
        logs.flush();
        let text = read(&dir.path().join(FILE_NAME));
        assert!(
            text.contains("WARN  plenipo_runtime::supervisor: a program stopped"),
            "{text}"
        );
        // The date comes first, with the offset from UTC.
        assert!(text.as_bytes()[4] == b'-' && text.contains(':'));
    }

    #[test]
    fn files_rotate_and_only_the_newest_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let logs = LogFiles::with_limits(dir.path(), 200, 3).unwrap();
        for n in 0..40 {
            logs.write(Level::Info, "plenipo", &format!("line number {n:03}"));
        }
        logs.flush();
        let files = logs.files();
        assert_eq!(files.len(), 3, "{files:?}");
        assert!(!dir.path().join("plenipo.3.log").exists());
        for f in &files {
            assert!(fs::metadata(f).unwrap().len() <= 200, "{f:?} is too big");
        }
        // The newest line is in the current file; the oldest ones are gone.
        assert!(read(&files[0]).contains("line number 039"));
        let all: String = files.iter().map(|f| read(f)).collect();
        assert!(!all.contains("line number 000"));
    }

    #[test]
    fn a_file_that_cannot_move_keeps_the_older_files_and_every_line() {
        let dir = tempfile::tempdir().unwrap();
        let logs = LogFiles::with_limits(dir.path(), 200, 3).unwrap();
        fs::write(dir.path().join("plenipo.1.log"), "older").unwrap();
        fs::write(dir.path().join("plenipo.2.log"), "oldest").unwrap();
        // As when another program holds plenipo.log open on Windows: it cannot be moved.
        let blocker = dir.path().join(format!("{FILE_NAME}.rotating"));
        fs::create_dir(&blocker).unwrap();
        fs::write(blocker.join("x"), "").unwrap();
        for n in 0..40 {
            logs.write(Level::Info, "plenipo", &format!("line number {n:03}"));
        }
        logs.flush();
        assert_eq!(read(&dir.path().join("plenipo.1.log")), "older");
        assert_eq!(read(&dir.path().join("plenipo.2.log")), "oldest");
        let current = read(&dir.path().join(FILE_NAME));
        for n in 0..40 {
            assert!(
                current.contains(&format!("line number {n:03}")),
                "line {n} kept"
            );
        }
    }

    #[test]
    fn a_big_file_from_before_is_rotated_at_start() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE_NAME), "x".repeat(500)).unwrap();
        let logs = LogFiles::with_limits(dir.path(), 200, 3).unwrap();
        assert_eq!(read(&dir.path().join(FILE_NAME)), "");
        assert_eq!(read(&dir.path().join("plenipo.1.log")).len(), 500);
        assert_eq!(logs.files().len(), 2);
    }

    #[test]
    fn secrets_never_reach_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let logs = LogFiles::open(dir.path()).unwrap();
        // Recognizable secrets are hidden from the start.
        logs.write(
            Level::Warn,
            "plenipo",
            "push failed with token ghp_abcdefghijklmnopqrstuvwxyz0123456789",
        );
        // Stored secrets are hidden once the Vault's filter is in place.
        logs.set_filter(Arc::new(|t: &str| t.replace("hunter2-owner", "[hidden]")));
        logs.write(
            Level::Warn,
            "plenipo",
            "server said hunter2-owner was wrong",
        );
        logs.flush();
        let text = read(&dir.path().join(FILE_NAME));
        assert!(
            !text.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"),
            "{text}"
        );
        assert!(text.contains(plenipo_guard::redact::MARKER), "{text}");
        assert!(!text.contains("hunter2-owner"), "{text}");
        assert!(text.contains("server said [hidden] was wrong"));
    }

    #[test]
    fn one_message_is_one_line_and_long_ones_are_cut() {
        let dir = tempfile::tempdir().unwrap();
        let logs = LogFiles::open(dir.path()).unwrap();
        logs.write(Level::Error, "plenipo", "first\nsecond\r\nthird");
        logs.write(Level::Error, "plenipo", &"y".repeat(10_000));
        logs.flush();
        let text = read(&dir.path().join(FILE_NAME));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(
            lines[0].ends_with("first ⏎ second ⏎  ⏎ third"),
            "{}",
            lines[0]
        );
        assert!(lines[1].ends_with("…(cut)"));
        assert!(lines[1].len() < 4_200);
    }

    #[test]
    fn only_plenipos_messages_and_others_warnings_are_kept() {
        let meta = |level, target| Metadata::builder().level(level).target(target).build();
        assert!(wanted(&meta(Level::Info, "plenipo_desktop_lib::recovery")));
        assert!(wanted(&meta(Level::Warn, "tao::platform")));
        assert!(!wanted(&meta(Level::Info, "tao::platform")));
        assert!(!wanted(&meta(Level::Debug, "plenipo_runtime")));
    }
}
