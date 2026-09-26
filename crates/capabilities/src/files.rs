//! File tools, carried out by Plenipo inside the project folder. Paths arrive already resolved
//! and allowed by Guard; these functions only do the work and describe the result.

use std::fs;
use std::io::Read as _;
use std::path::Path;

use plenipo_guard::paths::blocked_by;
use plenipo_guard::redact::has_marker;
use plenipo_guard::{Resolved, Workspace};

/// Largest file `read_file` opens.
pub const MAX_READ_FILE: u64 = 10 * 1024 * 1024;
/// Largest file `search_text` looks into.
const MAX_SEARCH_FILE: u64 = 1024 * 1024;
const MAX_ENTRIES: usize = 500;
const MAX_MATCHES: usize = 200;
const MAX_SEARCHED_FILES: usize = 20_000;
/// Folders `search_text` skips.
const SKIPPED: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
];

type Out = Result<String, String>;

fn io(what: &str, e: &std::io::Error) -> String {
    format!("{what}: {e}")
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|b| *b == 0)
}

fn refuse_marker(text: &str) -> Result<(), String> {
    if has_marker(text) {
        return Err(
            "the text contains a part Plenipo hid because it looked like a secret \
                    ([hidden by Plenipo: …]); writing it would destroy the real value. Change \
                    only the parts around it with edit_file."
                .into(),
        );
    }
    Ok(())
}

pub fn list(dir: &Resolved) -> Out {
    let meta = fs::metadata(&dir.abs).map_err(|e| io(dir.shown(), &e))?;
    if !meta.is_dir() {
        return Err(format!("{} is a file, not a folder", dir.shown()));
    }
    let mut entries: Vec<(bool, String, u64)> = fs::read_dir(&dir.abs)
        .map_err(|e| io(dir.shown(), &e))?
        .filter_map(Result::ok)
        .map(|e| {
            let m = e.metadata().ok();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (
                is_dir,
                e.file_name().to_string_lossy().into_owned(),
                m.map_or(0, |m| m.len()),
            )
        })
        .collect();
    entries.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    let total = entries.len();
    let mut out = format!("{} ({total} entries)\n", dir.shown());
    for (is_dir, name, size) in entries.iter().take(MAX_ENTRIES) {
        if *is_dir {
            out.push_str(&format!("{name}/\n"));
        } else {
            out.push_str(&format!("{name}  ({size} bytes)\n"));
        }
    }
    if total > MAX_ENTRIES {
        out.push_str(&format!("… and {} more\n", total - MAX_ENTRIES));
    }
    Ok(out)
}

pub fn read(file: &Resolved, offset: usize, limit: usize) -> Out {
    let meta = fs::metadata(&file.abs).map_err(|e| io(file.shown(), &e))?;
    if meta.is_dir() {
        return Err(format!("{} is a folder; use list_directory", file.shown()));
    }
    if meta.len() > MAX_READ_FILE {
        return Err(format!(
            "{} is {} bytes; files over {MAX_READ_FILE} bytes cannot be read",
            file.shown(),
            meta.len()
        ));
    }
    let bytes = fs::read(&file.abs).map_err(|e| io(file.shown(), &e))?;
    if is_binary(&bytes) {
        return Ok(format!(
            "{} is a binary file ({} bytes).",
            file.shown(),
            bytes.len()
        ));
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let start = offset.saturating_sub(1).min(total);
    let end = start.saturating_add(limit).min(total);
    let mut out = if start == 0 && end == total {
        format!("{} ({total} lines)\n", file.shown())
    } else {
        format!("{} (lines {}–{end} of {total})\n", file.shown(), start + 1)
    };
    for line in &lines[start..end] {
        out.push_str(line);
        out.push('\n');
    }
    if end < total {
        out.push_str(&format!(
            "… {} more lines (read on with offset {})\n",
            total - end,
            end + 1
        ));
    }
    Ok(out)
}

pub fn search(
    workspace: &Workspace,
    start: &Resolved,
    query: &str,
    case_sensitive: bool,
    blocked: &[String],
) -> Out {
    let needle = if case_sensitive {
        query.to_owned()
    } else {
        query.to_lowercase()
    };
    let mut matches = Vec::new();
    let mut files = 0usize;
    let mut stack = vec![start.abs.clone()];
    let root = workspace.root();
    while let Some(path) = stack.pop() {
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let rel = rel_of(root, &path);
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase());
            if path != start.abs && name.as_deref().is_some_and(|n| SKIPPED.contains(&n)) {
                continue;
            }
            if let Ok(read) = fs::read_dir(&path) {
                let mut children: Vec<_> = read.filter_map(Result::ok).map(|e| e.path()).collect();
                children.sort();
                stack.extend(children.into_iter().rev());
            }
            continue;
        }
        if blocked_by(blocked, &rel).is_some() || meta.len() > MAX_SEARCH_FILE {
            continue;
        }
        files += 1;
        if files > MAX_SEARCHED_FILES || matches.len() >= MAX_MATCHES {
            break;
        }
        let mut bytes = Vec::new();
        if fs::File::open(&path)
            .and_then(|mut f| f.read_to_end(&mut bytes))
            .is_err()
            || is_binary(&bytes)
        {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        for (i, line) in text.lines().enumerate() {
            let hay = if case_sensitive {
                line.to_owned()
            } else {
                line.to_lowercase()
            };
            if hay.contains(&needle) {
                let shown: String = line.trim().chars().take(300).collect();
                matches.push(format!("{rel}:{}: {shown}", i + 1));
                if matches.len() >= MAX_MATCHES {
                    break;
                }
            }
        }
    }
    if matches.is_empty() {
        return Ok(format!(
            "No lines contain {query:?} under {}.",
            start.shown()
        ));
    }
    let more = if matches.len() >= MAX_MATCHES {
        format!("\n(stopped at {MAX_MATCHES} matches)")
    } else {
        String::new()
    };
    Ok(format!(
        "{} matching line(s):\n{}{more}",
        matches.len(),
        matches.join("\n")
    ))
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| {
            p.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default()
}

pub fn write(file: &Resolved, content: &str) -> Out {
    refuse_marker(content)?;
    if file.abs.is_dir() {
        return Err(format!("{} is a folder", file.shown()));
    }
    if file.rel.is_empty() {
        return Err("name a file inside the project folder".into());
    }
    if let Some(parent) = file.abs.parent() {
        fs::create_dir_all(parent).map_err(|e| io(file.shown(), &e))?;
    }
    let existed = file.abs.exists();
    fs::write(&file.abs, content).map_err(|e| io(file.shown(), &e))?;
    Ok(format!(
        "{} {} ({} bytes).",
        if existed { "Replaced" } else { "Created" },
        file.shown(),
        content.len()
    ))
}

pub fn edit(file: &Resolved, old: &str, new: &str, all: bool) -> Out {
    refuse_marker(new)?;
    let bytes = fs::read(&file.abs).map_err(|e| io(file.shown(), &e))?;
    if is_binary(&bytes) {
        return Err(format!("{} is a binary file", file.shown()));
    }
    let text =
        String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8 text", file.shown()))?;
    let count = text.matches(old).count();
    if count == 0 {
        return Err(format!(
            "the text to replace was not found in {} (it must match exactly, including spaces; \
             parts Plenipo hid as secrets cannot be matched)",
            file.shown()
        ));
    }
    if count > 1 && !all {
        return Err(format!(
            "the text to replace appears {count} times in {}; give more of it so it appears once, \
             or set replaceAll",
            file.shown()
        ));
    }
    let updated = if all {
        text.replace(old, new)
    } else {
        text.replacen(old, new, 1)
    };
    fs::write(&file.abs, updated).map_err(|e| io(file.shown(), &e))?;
    Ok(format!("Edited {} ({count} replacement(s)).", file.shown()))
}

pub fn move_path(from: &Resolved, to: &Resolved) -> Out {
    if !from.exists || from.rel.is_empty() {
        return Err(format!("{} does not exist", from.shown()));
    }
    if to.exists || to.rel.is_empty() {
        return Err(format!("{} already exists", to.shown()));
    }
    if let Some(parent) = to.abs.parent() {
        fs::create_dir_all(parent).map_err(|e| io(to.shown(), &e))?;
    }
    fs::rename(&from.abs, &to.abs).map_err(|e| io(from.shown(), &e))?;
    Ok(format!("Moved {} to {}.", from.shown(), to.shown()))
}

pub fn delete(path: &Resolved) -> Out {
    if path.rel.is_empty() {
        return Err("the project folder itself cannot be deleted".into());
    }
    let meta = fs::symlink_metadata(&path.abs).map_err(|e| io(path.shown(), &e))?;
    if meta.is_dir() {
        fs::remove_dir(&path.abs).map_err(|e| {
            format!(
                "{} could not be deleted (only empty folders can be): {e}",
                path.shown()
            )
        })?;
    } else {
        fs::remove_file(&path.abs).map_err(|e| io(path.shown(), &e))?;
    }
    Ok(format!("Deleted {}.", path.shown()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proj");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("node_modules/x")).unwrap();
        fs::write(
            root.join("src/main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        fs::write(root.join("node_modules/x/index.js"), "hello").unwrap();
        fs::write(root.join(".env"), "HELLO=secret").unwrap();
        fs::write(root.join("data.bin"), [0u8, 1, 2]).unwrap();
        let ws = Workspace::open(&root.display().to_string()).unwrap();
        (dir, ws)
    }

    #[test]
    fn list_read_and_search() {
        let (_d, ws) = setup();
        let out = list(&ws.resolve(".").unwrap()).unwrap();
        assert!(out.contains("src/") && out.contains(".env"), "{out}");
        let out = read(&ws.resolve("src/main.rs").unwrap(), 2, 1).unwrap();
        assert!(
            out.contains("lines 2–2 of 3") && out.contains("println"),
            "{out}"
        );
        assert!(read(&ws.resolve("data.bin").unwrap(), 1, 10)
            .unwrap()
            .contains("binary"));
        assert!(read(&ws.resolve("src").unwrap(), 1, 10).is_err());
        let blocked = vec![".env".to_owned()];
        let out = search(&ws, &ws.resolve(".").unwrap(), "HELLO", false, &blocked).unwrap();
        assert!(out.contains("src/main.rs:2:"), "{out}");
        assert!(
            !out.contains("node_modules"),
            "build folders are skipped: {out}"
        );
        assert!(!out.contains(".env"), "blocked files are skipped: {out}");
    }

    #[test]
    fn write_edit_move_delete() {
        let (_d, ws) = setup();
        let f = ws.resolve("docs/new.md").unwrap();
        assert!(write(&f, "one two two").unwrap().starts_with("Created"));
        let f = ws.resolve("docs/new.md").unwrap();
        assert!(edit(&f, "two", "2", false).is_err(), "ambiguous");
        assert!(edit(&f, "three", "3", false).is_err());
        edit(&f, "two", "2", true).unwrap();
        assert_eq!(fs::read_to_string(&f.abs).unwrap(), "one 2 2");
        assert!(write(&f, "x [hidden by Plenipo: API key] y").is_err());
        assert!(edit(&f, "one", "[hidden by Plenipo: token]", false).is_err());
        let to = ws.resolve("docs/renamed.md").unwrap();
        move_path(&f, &to).unwrap();
        assert!(move_path(
            &ws.resolve("src/main.rs").unwrap(),
            &ws.resolve("docs/renamed.md").unwrap()
        )
        .is_err());
        assert!(delete(&ws.resolve("docs").unwrap()).is_err(), "not empty");
        delete(&ws.resolve("docs/renamed.md").unwrap()).unwrap();
        delete(&ws.resolve("docs").unwrap()).unwrap();
        assert!(delete(&ws.resolve(".").unwrap()).is_err());
    }
}
