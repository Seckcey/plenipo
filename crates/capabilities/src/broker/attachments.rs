//! Files on an objective (Phase 21, ADR-093 §19–§22): the owner drops files on an objective,
//! from Plenipo's Files panel or from anywhere on the PC.
//!
//! - A file inside the objective's own project folder is **named**, not copied, when its workers
//!   see it there as it is: committed and unchanged, or the project works in its own folder.
//!   Anything else (a change not committed yet, a new file) is copied.
//! - Every other file is **copied** when the objective is given, into Plenipo's own folder for
//!   it (`attachments/<ID>` in the data folder), so the original is never touched again. The
//!   objective's text lists the files and carries a mark (`[plenipo-files:<ID>]`) that says
//!   where the copies are.
//! - When a worker of the objective starts working in a folder (its working copy, or the project
//!   folder when it works in place), Plenipo puts the copies in an `attachments` folder there,
//!   once for each folder, never replacing anything already there, and never through a link.
//!   Git leaves that folder out (`info/exclude`), so the copies are never committed or pushed.
//! - A blocked file never goes on an objective, and nothing does while a worker uses the screen,
//!   mouse, and keyboard.
//! - Recorded: `objective.files_attached` (each file's name and size, whether it was copied) and
//!   `objective.files_delivered`; never a file's contents or where it came from on the PC.

use std::path::{Path, PathBuf};

use serde_json::json;

use plenipo_guard::paths::blocked_by;

use super::Broker;
use crate::error::{BrokerError, Result};

/// The most files on one objective, the largest one, and all of them together.
pub const MAX_FILES: usize = 20;
pub const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 100 * 1024 * 1024;
/// The folder the copies go into, in the objective's working folder.
pub const FOLDER: &str = "attachments";
/// Where the copies are, in the objective's text.
const MARK: &str = "[plenipo-files:";
/// Which folders have had the copies already.
const DELIVERED: &str = ".delivered.json";
/// Who does this.
const OWNER: &str = "owner";

/// The files put on an objective, ready before it is given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Staged {
    /// Where the copies are kept (`None`: nothing was copied).
    pub id: Option<String>,
    /// Copied files: their names (as the workers find them) and sizes.
    pub copied: Vec<(String, u64)>,
    /// Files in the project folder, named by their path inside it.
    pub named: Vec<String>,
}

impl Staged {
    pub fn is_empty(&self) -> bool {
        self.copied.is_empty() && self.named.is_empty()
    }

    /// The objective's text, with its files.
    pub fn objective(&self, objective: &str) -> String {
        let mut out = objective.trim().to_owned();
        if !self.named.is_empty() {
            out.push_str("\n\nLook at these files in the project: ");
            out.push_str(&self.named.join(", "));
            out.push('.');
        }
        if let (Some(id), false) = (&self.id, self.copied.is_empty()) {
            let names: Vec<&str> = self.copied.iter().map(|(n, _)| n.as_str()).collect();
            out.push_str(&format!(
                "\n\nThe owner gave you copies of these files, in the folder `{FOLDER}` of your \
                 working folder: {}. They are not part of the project unless you add them. \
                 {MARK}{id}]",
                names.join(", ")
            ));
        }
        out
    }
}

/// The mark's ID in an objective's text.
fn mark_of(objective: &str) -> Option<&str> {
    let at = objective.rfind(MARK)? + MARK.len();
    let id = objective.get(at..at + 32)?;
    (objective.get(at + 32..at + 33) == Some("]") && id.chars().all(|c| c.is_ascii_hexdigit()))
        .then_some(id)
}

/// A name for a copy that is not taken yet in `names` ("report (2).pdf").
fn free_name(name: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t.eq_ignore_ascii_case(name)) {
        return name.to_owned();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (2..)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|candidate| !taken.iter().any(|t| t.eq_ignore_ascii_case(candidate)))
        .unwrap_or_else(|| name.to_owned())
}

impl Broker {
    /// Get the files for an objective of `project_id` ready: name those inside its folder, copy
    /// the rest. `sources`: files the owner chose (from the Files panel, already checked, or
    /// dropped from File Explorer).
    pub fn stage_files(&self, project_id: &str, sources: &[PathBuf]) -> Result<Staged> {
        if sources.is_empty() {
            return Ok(Staged::default());
        }
        if sources.len() > MAX_FILES {
            return Err(BrokerError::Invalid(format!(
                "An objective can have up to {MAX_FILES} files."
            )));
        }
        if self.inner.control.status().desktop_active() {
            return Err(BrokerError::Invalid(
                "A worker is using the screen, mouse, and keyboard. Take over first, then put \
                 files on the objective."
                    .into(),
            ));
        }
        let project = self
            .ledger()
            .project(project_id)?
            .ok_or_else(|| BrokerError::Invalid("That project is not there any more.".into()))?;
        let folder = project
            .local_path
            .as_deref()
            .and_then(|p| dunce::canonicalize(p).ok());
        let blocked = self.inner.guard.config()?.blocked_files;
        let refuse_blocked = |shown: &str| {
            BrokerError::Invalid(format!(
                "{shown} is a blocked file: workers may not read it, so it cannot go on an \
                 objective."
            ))
        };
        // Workers get a working copy made from the last commit: a file there only if it is
        // committed and unchanged; any other is copied.
        let git = self
            .inner
            .git
            .as_ref()
            .filter(|_| project.branch_per_objective);
        let as_committed = |rel: &str| -> bool {
            match (git, folder.as_ref()) {
                (None, _) | (_, None) => true,
                (Some(git), Some(f)) => git
                    .run(f, &["status", "--porcelain", "--ignored", "--", rel])
                    .is_ok_and(|out| out.trim().is_empty()),
            }
        };
        let mut staged = Staged::default();
        let mut copies: Vec<(PathBuf, String, u64)> = Vec::new();
        let mut total = 0u64;
        for source in sources {
            let real = dunce::canonicalize(source).map_err(|_| {
                BrokerError::Invalid(format!("{} is not there any more.", shown(source)))
            })?;
            let meta = std::fs::metadata(&real).map_err(|_| {
                BrokerError::Invalid(format!("{} is not there any more.", shown(source)))
            })?;
            if !meta.is_file() {
                return Err(BrokerError::Invalid(format!(
                    "{} is a folder. Put its files on the objective instead.",
                    shown(source)
                )));
            }
            if let Some(rel) = folder
                .as_ref()
                .and_then(|f| real.strip_prefix(f).ok())
                .filter(|rel| !rel.as_os_str().is_empty())
            {
                let rel = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                if blocked_by(&blocked, &rel).is_some() {
                    return Err(refuse_blocked(&rel));
                }
                if as_committed(&rel) {
                    if !staged.named.contains(&rel) {
                        staged.named.push(rel);
                    }
                    continue;
                }
            }
            if meta.len() > MAX_FILE_BYTES {
                return Err(BrokerError::Invalid(format!(
                    "{} is larger than 25 MB.",
                    shown(source)
                )));
            }
            total += meta.len();
            if total > MAX_TOTAL_BYTES {
                return Err(BrokerError::Invalid(
                    "The files on one objective can be up to 100 MB in all.".into(),
                ));
            }
            let taken: Vec<String> = copies.iter().map(|(_, n, _)| n.clone()).collect();
            let name = real
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.') || n.len() > 1)
                .unwrap_or_else(|| "file".into());
            if blocked_by(&blocked, &name).is_some() {
                return Err(refuse_blocked(&name));
            }
            let name = free_name(&name, &taken);
            // Where workers find it must not be a blocked name either.
            if blocked_by(&blocked, &format!("{FOLDER}/{name}")).is_some() {
                return Err(refuse_blocked(&name));
            }
            copies.push((real, name, meta.len()));
        }
        if copies.is_empty() {
            return Ok(staged);
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        let dir = self.inner.config.attachments_dir.join(&id);
        let copy = || -> std::io::Result<()> {
            std::fs::create_dir_all(&dir)?;
            for (from, name, _) in &copies {
                std::fs::copy(from, dir.join(name))?;
            }
            Ok(())
        };
        if let Err(e) = copy() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(BrokerError::Invalid(format!(
                "Plenipo could not copy the files ({e})."
            )));
        }
        staged.copied = copies.into_iter().map(|(_, n, s)| (n, s)).collect();
        staged.id = Some(id);
        Ok(staged)
    }

    /// Drop copies made for an objective that was not given after all.
    pub fn discard_files(&self, staged: &Staged) {
        if let Some(id) = &staged.id {
            let _ = std::fs::remove_dir_all(self.inner.config.attachments_dir.join(id));
        }
    }

    /// Record the files on the objective's task.
    pub fn record_files(&self, task_id: &str, staged: &Staged) {
        if staged.is_empty() {
            return;
        }
        let mut files: Vec<serde_json::Value> = staged
            .named
            .iter()
            .map(|path| json!({ "name": self.redact(path), "copied": false }))
            .collect();
        files.extend(staged.copied.iter().map(
            |(name, size)| json!({ "name": self.redact(name), "size": size, "copied": true }),
        ));
        self.event(
            Some(task_id),
            OWNER,
            "objective.files_attached",
            json!({ "files": files }),
        );
    }

    /// Put an objective's copied files in the folder a worker of it works in: once for each
    /// folder, into `attachments`, never replacing anything there and never through a link.
    /// Add `/attachments/` to the folder's git exclude list (`info/exclude`, shared by the
    /// repository's working copies), once. Nothing happens outside a git repository.
    fn keep_out_of_git(&self, folder: &Path) {
        let Some(git) = &self.inner.git else { return };
        let Ok(out) = git.run(folder, &["rev-parse", "--git-path", "info/exclude"]) else {
            return;
        };
        let path = PathBuf::from(out.trim());
        let exclude = if path.is_absolute() {
            path
        } else {
            folder.join(path)
        };
        let line = format!("/{FOLDER}/");
        let now = std::fs::read_to_string(&exclude).unwrap_or_default();
        if now.lines().any(|l| l.trim() == line) {
            return;
        }
        let write = || -> std::io::Result<()> {
            if let Some(dir) = exclude.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut text = now.clone();
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str("# Files the owner put on objectives (Plenipo)\n");
            text.push_str(&line);
            text.push('\n');
            std::fs::write(&exclude, text)
        };
        if let Err(e) = write() {
            self.notice(format!(
                "Git could not be told to leave out the files on an objective: {e}"
            ));
        }
    }

    pub(super) fn deliver_files(&self, task_id: &str, folder: &Path) {
        let Ok(root) = self.ledger().task_root(task_id) else {
            return;
        };
        let Some(id) = mark_of(&root.objective) else {
            return;
        };
        let dir = self.inner.config.attachments_dir.join(id);
        if !dir.is_dir() {
            return;
        }
        let record = dir.join(DELIVERED);
        let mut done: Vec<String> = std::fs::read(&record)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let here = folder.display().to_string();
        if done.contains(&here) {
            return;
        }
        let target = folder.join(FOLDER);
        // The copies are the owner's, not the project's: git leaves them out, so a worker adding
        // everything never commits or pushes them.
        self.keep_out_of_git(folder);
        match std::fs::symlink_metadata(&target) {
            Ok(m) if !m.is_dir() || m.file_type().is_symlink() => {
                self.notice(format!(
                    "The files for an objective were not put in {} (something else has that \
                     name there).",
                    target.display()
                ));
                return;
            }
            Ok(_) => {}
            Err(_) => {
                if std::fs::create_dir(&target).is_err() {
                    return;
                }
            }
        }
        let mut count = 0u32;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(std::result::Result::ok) {
                let name = entry.file_name();
                if name == DELIVERED || !entry.path().is_file() {
                    continue;
                }
                let dest = target.join(&name);
                if std::fs::symlink_metadata(&dest).is_ok() {
                    continue;
                }
                if std::fs::copy(entry.path(), &dest).is_ok() {
                    count += 1;
                }
            }
        }
        done.push(here);
        let _ = std::fs::write(&record, serde_json::to_vec(&done).unwrap_or_default());
        self.event(
            Some(&root.id),
            OWNER,
            "objective.files_delivered",
            json!({ "count": count, "folder": FOLDER }),
        );
    }
}

/// A file's name for a message (never its folder on the PC).
fn shown(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| "That file".into(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mark_is_found_only_when_whole() {
        let id = "0123456789abcdef0123456789abcdef";
        let text = Staged {
            id: Some(id.into()),
            copied: vec![("a.pdf".into(), 3)],
            named: vec!["src/app.ts".into()],
        }
        .objective("Fix the page");
        assert!(text.starts_with("Fix the page\n\nLook at these files in the project: src/app.ts."));
        assert!(text.contains("folder `attachments`"));
        assert_eq!(mark_of(&text), Some(id));
        assert_eq!(mark_of("[plenipo-files:0123]"), None);
        assert_eq!(
            mark_of("[plenipo-files:0123456789abcdef0123456789abcdeg]"),
            None
        );
        assert_eq!(mark_of("nothing"), None);
    }

    #[test]
    fn copies_get_names_of_their_own() {
        assert_eq!(free_name("a.pdf", &[]), "a.pdf");
        assert_eq!(free_name("a.pdf", &["A.PDF".into()]), "a (2).pdf");
        assert_eq!(
            free_name("a.pdf", &["a.pdf".into(), "a (2).pdf".into()]),
            "a (3).pdf"
        );
        assert_eq!(free_name("README", &["README".into()]), "README (2)");
    }
}
