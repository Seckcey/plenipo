//! Working copies (Phase 8, ADR-016): a branch and a git worktree for each objective, made from
//! the project's current commit, so an objective's workers never touch the owner's own checkout
//! and two objectives never share files.
//!
//! These are Plenipo's own git operations, not a worker's: they run with the repository's hooks
//! turned off (a hook is the project's code, and making a working copy must not run it), with a
//! cleared environment, never asking for a password, and with a time limit.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use plenipo_ledger::{CommitInfo, FileChange, WorkspaceFacts};

use crate::programs;

/// Longest one of Plenipo's own git operations may take.
const TIMEOUT: Duration = Duration::from_secs(60);
/// Most commits and files recorded for a branch.
const MAX_COMMITS: usize = 50;
const MAX_FILES: usize = 500;

/// A git repository as its folder sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The repository's top folder.
    pub top: PathBuf,
    /// Where the folder is inside it, with `/` (`""`: the top).
    pub subfolder: String,
    /// The current commit.
    pub head: String,
    /// The current branch, if the checkout is on one.
    pub branch: Option<String>,
}

/// Runs git for Plenipo itself.
#[derive(Debug, Clone)]
pub struct Git {
    executable: PathBuf,
    /// An empty folder given to git as its hooks folder, so no hook runs.
    no_hooks: PathBuf,
}

impl Git {
    /// Git from PATH; `no_hooks` is created empty.
    pub fn find(no_hooks: PathBuf) -> Option<Self> {
        let executable = programs::find_on_path("git")?;
        std::fs::create_dir_all(&no_hooks).ok()?;
        Some(Self {
            executable,
            no_hooks,
        })
    }

    /// Run git in `dir`; its output, or a one-line reason. The broker's git tools use it too,
    /// to look at a repository (what a `git add` would stage, what is staged, what a diff or
    /// a push covers) before Guard decides.
    pub(crate) fn run(&self, dir: &Path, args: &[&str]) -> Result<String, String> {
        let mut command = Command::new(&self.executable);
        command
            .arg("-C")
            .arg(dir)
            .args([
                "--no-pager",
                "-c",
                "color.ui=false",
                "-c",
                "core.quotepath=false",
            ])
            .arg("-c")
            .arg(format!("core.hooksPath={}", self.no_hooks.display()))
            .args(args)
            .env_clear()
            .envs(plenipo_runtime::policy::build_child_env(
                &programs::git_env(),
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("git could not be started: {e}"))?;
        let read = |pipe: Option<Box<dyn std::io::Read + Send>>| {
            std::thread::spawn(move || {
                let mut out = Vec::new();
                if let Some(mut p) = pipe {
                    let _ = p.read_to_end(&mut out);
                }
                out
            })
        };
        let stdout = read(child.stdout.take().map(|p| Box::new(p) as Box<_>));
        let stderr = read(child.stderr.take().map(|p| Box::new(p) as Box<_>));
        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("git {} took too long", args.join(" ")));
                }
                Err(e) => return Err(format!("git failed: {e}")),
            }
        };
        let out = String::from_utf8_lossy(&stdout.join().unwrap_or_default()).into_owned();
        let err = String::from_utf8_lossy(&stderr.join().unwrap_or_default()).into_owned();
        if status.success() {
            Ok(out)
        } else {
            let why = err
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("git failed")
                .to_owned();
            Err(why)
        }
    }

    /// The repository `folder` is in, if it is one with at least one commit and the folder is
    /// part of that commit. A folder inside a repository it is not committed to (ignored, not
    /// yet added, or a build folder of some other project) has no working copy: one would not
    /// contain its files.
    pub fn repository(&self, folder: &Path) -> Option<Repo> {
        let lines = self
            .run(folder, &["rev-parse", "--show-toplevel", "--show-prefix"])
            .ok()?;
        let mut lines = lines.lines();
        let top = PathBuf::from(lines.next()?.trim());
        let subfolder = lines
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches('/')
            .to_owned();
        let head = self
            .run(folder, &["rev-parse", "--verify", "-q", "HEAD^{commit}"])
            .ok()?
            .trim()
            .to_owned();
        if head.is_empty() {
            return None;
        }
        if !subfolder.is_empty() {
            let kind = self
                .run(&top, &["cat-file", "-t", &format!("{head}:{subfolder}")])
                .ok()?;
            if kind.trim() != "tree" {
                return None;
            }
        }
        let branch = self
            .run(folder, &["symbolic-ref", "--short", "-q", "HEAD"])
            .ok()
            .map(|b| b.trim().to_owned())
            .filter(|b| !b.is_empty());
        Some(Repo {
            top,
            subfolder,
            head,
            branch,
        })
    }

    /// Make a working copy at `path` on a new branch `branch` from `base` (a commit).
    pub fn add_worktree(
        &self,
        repo_top: &Path,
        path: &Path,
        branch: &str,
        base: &str,
    ) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        let path = path.display().to_string();
        self.run(
            repo_top,
            &["worktree", "add", "--quiet", "-b", branch, &path, base],
        )
        .map(|_| ())
    }

    /// Remove a working copy's folder (its branch stays in the repository).
    pub fn remove_worktree(&self, repo_top: &Path, path: &Path) -> Result<(), String> {
        let shown = path.display().to_string();
        let removed = self.run(repo_top, &["worktree", "remove", "--force", &shown]);
        let _ = self.run(repo_top, &["worktree", "prune"]);
        match removed {
            Ok(_) => Ok(()),
            // Already gone (removed by hand): nothing left to do.
            Err(_) if !path.exists() => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// The current commit of a working copy.
    pub fn head(&self, worktree: &Path) -> Result<String, String> {
        Ok(self
            .run(worktree, &["rev-parse", "HEAD"])?
            .trim()
            .to_owned())
    }

    /// What is on a working copy's branch: commits since `base`, and the files changed —
    /// committed or not.
    pub fn facts(
        &self,
        worktree: &Path,
        base: &str,
        branch: &str,
        now: u64,
    ) -> Result<WorkspaceFacts, String> {
        let head = self.head(worktree)?;
        let range = format!("{base}..HEAD");
        let commits: Vec<CommitInfo> = self
            .run(
                worktree,
                &[
                    "log",
                    "--format=%h%x09%s",
                    "-n",
                    &MAX_COMMITS.to_string(),
                    &range,
                ],
            )?
            .lines()
            .filter_map(|l| {
                let (hash, subject) = l.split_once('\t')?;
                Some(CommitInfo {
                    hash: hash.trim().to_owned(),
                    subject: subject.trim().to_owned(),
                })
            })
            .collect();
        let mut files: Vec<FileChange> = numstat(
            &self.run(
                worktree,
                &["diff", "--numstat", "--no-renames", base, "HEAD"],
            )?,
            true,
        );
        // Changes not committed: tracked files against HEAD, then new files.
        let uncommitted_tracked = numstat(
            &self.run(worktree, &["diff", "--numstat", "--no-renames", "HEAD"])?,
            false,
        );
        let status = self.run(
            worktree,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--no-renames",
            ],
        )?;
        let entries: Vec<(&str, &str)> = status
            .split('\0')
            .filter(|e| e.len() > 3)
            .map(|e| (&e[..2], &e[3..]))
            .collect();
        for change in uncommitted_tracked {
            match files.iter_mut().find(|f| f.path == change.path) {
                Some(f) => *f = change,
                None => files.push(change),
            }
        }
        for (code, path) in &entries {
            if *code == "??" && !files.iter().any(|f| f.path == *path) {
                files.push(FileChange {
                    path: (*path).to_owned(),
                    added: None,
                    removed: None,
                    committed: false,
                });
            }
        }
        files.truncate(MAX_FILES);
        let remote = format!("refs/remotes/origin/{branch}");
        let pushed = self
            .run(worktree, &["rev-parse", "--verify", "-q", &remote])
            .is_ok_and(|r| r.trim() == head);
        Ok(WorkspaceFacts {
            head: Some(head),
            commits,
            files,
            uncommitted: u32::try_from(entries.len()).unwrap_or(u32::MAX),
            pushed,
            checked_at: Some(now),
        })
    }
}

/// `git diff --numstat` lines: `added<TAB>removed<TAB>path` (`-` for a binary file).
fn numstat(text: &str, committed: bool) -> Vec<FileChange> {
    text.lines()
        .filter_map(|l| {
            let mut parts = l.splitn(3, '\t');
            let added = parts.next()?.trim().parse().ok();
            let removed = parts.next()?.trim().parse().ok();
            let path = parts.next()?.trim();
            (!path.is_empty()).then(|| FileChange {
                path: path.to_owned(),
                added,
                removed,
                committed,
            })
        })
        .collect()
}

/// The branch for an objective: `plenipo/<a few words of it>-<its ID>`.
pub fn branch_name(objective: &str, correlation_id: &str) -> String {
    let first = objective
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let mut slug = String::new();
    for c in first.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.len() >= 40 {
            break;
        }
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "objective" } else { slug };
    format!("plenipo/{slug}-{}", short_id(correlation_id))
}

/// The first eight letters and digits of an ID, lower case.
pub fn short_id(id: &str) -> String {
    id.chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_names_are_short_safe_and_unique_per_objective() {
        assert_eq!(
            branch_name(
                "Add input validation to the signup form!\nMore details",
                "1A2B3C4D-5e6f"
            ),
            "plenipo/add-input-validation-to-the-signup-form-1a2b3c4d"
        );
        assert_eq!(branch_name("  ??? ", "wf-99"), "plenipo/objective-wf99");
        let long = branch_name(&"word ".repeat(40), "abcdef0123456789");
        assert!(long.len() <= "plenipo/".len() + 41 + 9, "{long}");
        assert!(!long.contains("--"), "{long}");
        assert_eq!(branch_name("Ünïcode café", "x1"), "plenipo/n-code-caf-x1");
    }

    fn git_in(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    #[test]
    fn only_a_folder_committed_to_its_repository_gets_working_copies() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().join("repo");
        std::fs::create_dir_all(top.join("site")).unwrap();
        std::fs::create_dir_all(top.join("build").join("tmp")).unwrap();
        std::fs::write(top.join("site").join("index.html"), "hi\n").unwrap();
        std::fs::write(top.join(".gitignore"), "build/\n").unwrap();
        git_in(&top, &["init", "-q", "-b", "main"]);
        git_in(&top, &["config", "user.name", "Test"]);
        git_in(&top, &["config", "user.email", "test@example.com"]);
        git_in(&top, &["add", "-A"]);
        git_in(&top, &["commit", "-q", "-m", "Start"]);
        std::fs::create_dir_all(top.join("new")).unwrap();
        std::fs::write(top.join("new").join("a.txt"), "not added\n").unwrap();
        let git = Git::find(dir.path().join("hooks")).expect("git is installed");

        let repo = git.repository(&top).expect("the top folder");
        assert_eq!(repo.subfolder, "");
        assert_eq!(repo.branch.as_deref(), Some("main"));
        let site = git
            .repository(&top.join("site"))
            .expect("a committed subfolder");
        assert_eq!(site.subfolder, "site");
        // An ignored folder (say, another project's build folder), and one not yet committed.
        assert_eq!(git.repository(&top.join("build").join("tmp")), None);
        assert_eq!(git.repository(&top.join("new")), None);
        // Not a repository at all.
        let plain = tempfile::tempdir().unwrap();
        assert_eq!(git.repository(plain.path()), None);
    }

    #[test]
    fn numstat_reads_binary_files_and_paths_with_spaces() {
        let files = numstat("3\t1\tsrc/a b.rs\n-\t-\tlogo.png\n", true);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/a b.rs");
        assert_eq!((files[0].added, files[0].removed), (Some(3), Some(1)));
        assert_eq!((files[1].added, files[1].removed), (None, None));
        assert!(files.iter().all(|f| f.committed));
    }
}
