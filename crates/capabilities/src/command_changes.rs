//! Watch shows changes made by commands too (Phase 25, item 3.2; ADR-250). Before a command or a
//! git step runs in a working copy, Plenipo notes its files (their size and time, and the text of
//! the small ones); after, it compares. Each file the command made or changed shows in Watch as
//! "made by a command", with its lines marked when its text before was noted. Files Guard keeps
//! private are never read, and the folders a command fills by itself (`.git`, `node_modules`,
//! build output) are left out.

use std::collections::HashMap;
use std::fs::Metadata;
use std::path::Path;
use std::time::SystemTime;

use plenipo_guard::paths::blocked_by;

use crate::watch::{Before, Written};

/// Folders left out: version control, packages, and build output.
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    ".next",
    ".nuxt",
    ".venv",
    "venv",
    "__pycache__",
    ".cache",
    ".gradle",
    ".idea",
    ".vs",
    ".turbo",
];
/// A working copy with more files than this is not compared (it would take too long).
const MAX_FILES: usize = 20_000;
/// The text of a file this size or smaller is noted before a command.
const MAX_TEXT: u64 = 256 * 1024;
/// At most this much text is noted before one command.
const TEXT_BUDGET: u64 = 16 * 1024 * 1024;
/// At most this many changes are shown for one command.
pub const MAX_CHANGES: usize = 50;

/// The working copy's files before a command.
#[derive(Debug, Default)]
pub struct Snapshot {
    files: HashMap<String, Seen>,
    /// Every file was noted (not too many).
    complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    len: u64,
    modified: Option<SystemTime>,
    text: Option<String>,
}

/// One file a command made or changed: its path in the working copy (with `/`) and the file
/// before and after (secrets not yet hidden).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub rel: String,
    pub written: Written,
}

/// Note the files in `root`, leaving out those `blocked` (Guard's private files).
pub fn take(root: &Path, blocked: &[String]) -> Snapshot {
    let mut files = HashMap::new();
    let mut budget = TEXT_BUDGET;
    let mut complete = true;
    walk(root, "", blocked, &mut |rel, meta, path| {
        if files.len() >= MAX_FILES {
            complete = false;
            return false;
        }
        let text = if meta.len() <= MAX_TEXT && meta.len() <= budget {
            std::fs::read(path)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
        } else {
            None
        };
        if let Some(t) = &text {
            budget = budget.saturating_sub(t.len() as u64);
        }
        files.insert(
            rel,
            Seen {
                len: meta.len(),
                modified: meta.modified().ok(),
                text,
            },
        );
        true
    });
    Snapshot { files, complete }
}

/// The files made or changed in `root` since `before` (at most [`MAX_CHANGES`]). Nothing when
/// the working copy had too many files to note.
pub fn since(before: &Snapshot, root: &Path, blocked: &[String]) -> Vec<Made> {
    if !before.complete {
        return Vec::new();
    }
    let mut out = Vec::new();
    walk(root, "", blocked, &mut |rel, meta, path| {
        if out.len() >= MAX_CHANGES {
            return false;
        }
        let seen = before.files.get(&rel);
        if seen.is_some_and(|s| s.len == meta.len() && s.modified == meta.modified().ok()) {
            return true;
        }
        let after = (meta.len() <= MAX_TEXT)
            .then(|| std::fs::read(path).ok())
            .flatten();
        let text = after.and_then(|b| String::from_utf8(b).ok());
        let before_text = match seen {
            None => Before::Missing,
            Some(Seen { text: Some(t), .. }) => Before::Text(t.clone()),
            Some(Seen { len, .. }) => Before::Unshown {
                bytes: *len,
                binary: false,
            },
        };
        let written = match text {
            // Only its time changed: not a change.
            Some(t) if matches!(&before_text, Before::Text(b) if *b == t) => return true,
            Some(t) => Written::new(before_text, t),
            None => Written {
                before: Before::Unshown {
                    bytes: seen.map_or(0, |s| s.len),
                    // Small enough to read, but not text.
                    binary: meta.len() <= MAX_TEXT,
                },
                after: String::new(),
                after_bytes: meta.len(),
            },
        };
        out.push(Made { rel, written });
        true
    });
    out
}

/// Each file under `dir` (not following links), with its path from `root`; `visit` returns
/// `false` to stop.
fn walk(
    dir: &Path,
    prefix: &str,
    blocked: &[String],
    visit: &mut dyn FnMut(String, &Metadata, &Path) -> bool,
) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return true;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if blocked_by(blocked, &rel).is_some() {
            continue;
        }
        if kind.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            if !walk(&entry.path(), &rel, blocked, visit) {
                return false;
            }
        } else if kind.is_file() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !visit(rel, &meta, &entry.path()) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_file_a_command_makes_or_changes_shows_and_a_private_one_never_does() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/app.ts", "one\ntwo\n");
        write(root, "README.md", "Hello\n");
        write(root, ".env", "TOKEN=1\n");
        write(root, "node_modules/x/index.js", "x\n");
        let blocked = vec![".env".to_owned()];
        let before = take(root, &blocked);
        assert!(
            !before.files.contains_key(".env"),
            "a private file is never read"
        );
        assert!(!before.files.contains_key("node_modules/x/index.js"));

        // "The command": changes one file, makes one, changes a private one and a package.
        std::thread::sleep(std::time::Duration::from_millis(20));
        write(root, "src/app.ts", "one\nTWO\nthree\n");
        write(root, "src/new.ts", "fresh\n");
        write(root, ".env", "TOKEN=2\n");
        write(root, "node_modules/x/index.js", "y\n");
        write(root, "README.md", "Hello\n"); // the same text again: not a change

        let made = since(&before, root, &blocked);
        let paths: Vec<&str> = made.iter().map(|m| m.rel.as_str()).collect();
        assert_eq!(paths, ["src/app.ts", "src/new.ts"]);
        assert_eq!(made[0].written.before, Before::Text("one\ntwo\n".into()));
        assert_eq!(made[0].written.after, "one\nTWO\nthree\n");
        assert_eq!(made[1].written.before, Before::Missing);
    }

    #[test]
    fn a_working_copy_with_too_many_files_is_not_compared() {
        let before = Snapshot {
            files: HashMap::new(),
            complete: false,
        };
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.txt", "a");
        assert!(since(&before, dir.path(), &[]).is_empty());
    }
}
