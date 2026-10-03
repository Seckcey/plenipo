//! The git tools and the blocked-files list. The file tools name every path they touch, so
//! Guard refuses a file on the owner's blocked list before anything happens. A git tool names
//! paths git then expands (a folder, `.`), and shows (a diff), records (a commit), or sends
//! (a push) file contents Guard never saw. So before Guard decides, Plenipo's own git looks at
//! the repository:
//!
//! - `git add`: the files it would stage join the files Guard checks, so a blocked file is
//!   refused whether it was named or found under a folder.
//! - `git diff`: blocked files are left out of the diff, and the worker hears how many.
//! - `git commit`: a blocked file among the staged files stops the commit; the worker unstages
//!   it (Plenipo never changes the index for it).
//! - `git push` (and the push of a pull request): the approval card names the blocked files
//!   the commits change, or says the check could not run. The owner decides.
//!
//! `git status` and `git log` show names and commit lines, never contents, and stay as they are.

use std::path::Path;

use plenipo_guard::paths::blocked_by;
use plenipo_guard::{Layer, Resolved, Workspace};

use super::{Broker, Prepared, Refused, Work};

/// What a git tool asks Plenipo's own git to look at before Guard decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GitLook {
    /// `git add` of these paths (as the folder sees them): the files it would stage.
    Add { paths: Vec<String> },
    /// `git diff`, staged or not, of one path or everything: the blocked files it would show.
    Diff { staged: bool, path: Option<String> },
    /// `git commit`: the blocked files staged.
    Commit,
    /// `git push` of a branch (`None`: the current one) to a remote: the blocked files in the
    /// commits it would send.
    Push {
        remote: String,
        branch: Option<String>,
    },
}

/// What the look found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Found {
    /// `git add`: the files it would stage, as the folder sees them (blocked or not).
    Staging(Vec<String>),
    /// `git diff`: the blocked files it would show, as paths from the repository's top (the
    /// form a pathspec that leaves them out takes).
    Showing(Vec<String>),
    /// `git commit`: the blocked files staged, as the folder sees them.
    Staged(Vec<String>),
    /// `git push`: the blocked files the commits change, as the folder sees them; `None` when
    /// the commits to be sent could not be worked out.
    Sending(Option<Vec<String>>),
}

/// Why a git tool call stops before Guard.
pub(super) enum Stopped {
    /// The blocked-files rule refuses it (a blocked file is staged).
    Refused(Refused),
    /// The look itself could not run; the call is not done, and the worker may try again.
    Unchecked(String),
}

/// Runs git in the project folder for Plenipo itself: its output, or a one-line reason.
pub(super) type Run<'a> = &'a dyn Fn(&[&str]) -> Result<String, String>;

/// The folder's place in its repository (`sub/`, or empty at the top). Git names files from the
/// repository's top; the blocked list is matched as the folder sees them.
fn prefix(run: Run<'_>) -> Result<String, String> {
    run(&["rev-parse", "--show-prefix"]).map(|s| s.trim().to_owned())
}

/// A path from the repository's top as the folder sees it.
fn in_folder(prefix: &str, from_top: &str) -> String {
    from_top.strip_prefix(prefix).unwrap_or(from_top).to_owned()
}

/// Paths git printed with `-z` (NUL between them, never quoted).
fn paths_z(out: &str) -> Vec<String> {
    out.split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The files `git add` would stage for `paths`, as the folder sees them: files not yet tracked
/// (ignored ones left out, as `git add` leaves them), changed, or deleted.
pub(super) fn would_stage(run: Run<'_>, paths: &[String]) -> Result<Vec<String>, String> {
    let mut args = vec![
        "ls-files",
        "-z",
        "--others",
        "--modified",
        "--deleted",
        "--exclude-standard",
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    let mut files = paths_z(&run(&args)?);
    files.sort();
    files.dedup();
    Ok(files)
}

/// The blocked files among `from_top` (paths from the repository's top), in that form.
fn blocked_among<'a>(blocked: &[String], prefix: &str, from_top: &'a [String]) -> Vec<&'a str> {
    from_top
        .iter()
        .filter(|p| blocked_by(blocked, &in_folder(prefix, p)).is_some())
        .map(String::as_str)
        .collect()
}

/// Where the commits a push of `branch` (or the current branch) to `remote` would send begin:
/// its upstream, else the remote's copy of the branch, else (a branch never pushed) the
/// remote's copy of `base` (the objective's base branch) or `base` itself, else the remote's
/// default branch. The files those commits change are `from...branch`.
fn push_from(
    run: Run<'_>,
    remote: &str,
    branch: Option<&str>,
    base: Option<&str>,
) -> Result<String, String> {
    let head = branch.unwrap_or("HEAD");
    let exists = |rev: &str| run(&["rev-parse", "--verify", "--quiet", rev]).is_ok();
    let upstream = format!("{head}@{{u}}");
    if exists(&upstream) {
        return Ok(upstream);
    }
    let name = match branch {
        Some(b) => Some(b.to_owned()),
        None => run(&["rev-parse", "--abbrev-ref", "HEAD"])
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| s != "HEAD"),
    };
    let remote_copy = |b: &str| format!("refs/remotes/{remote}/{b}");
    if let Some(name) = &name {
        if exists(&remote_copy(name)) {
            return Ok(remote_copy(name));
        }
    }
    let of_base = base.and_then(|b| {
        [remote_copy(b), b.to_owned()]
            .into_iter()
            .find(|r| exists(r))
    });
    of_base
        .or_else(|| {
            run(&["symbolic-ref", "-q", &remote_copy("HEAD")])
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            ["main", "master"]
                .iter()
                .map(|b| remote_copy(b))
                .find(|r| exists(r))
        })
        .ok_or_else(|| format!("no upstream and no default branch of {remote} to compare with"))
}

/// The blocked files the commits a push would send change, as the folder sees them.
fn sending(
    run: Run<'_>,
    remote: &str,
    branch: Option<&str>,
    base: Option<&str>,
    blocked: &[String],
) -> Result<Vec<String>, String> {
    let prefix = prefix(run)?;
    let from = push_from(run, remote, branch, base)?;
    let range = format!("{from}...{}", branch.unwrap_or("HEAD"));
    let files = paths_z(&run(&[
        "diff",
        "--no-renames",
        "--name-only",
        "-z",
        &range,
    ])?);
    Ok(blocked_among(blocked, &prefix, &files)
        .iter()
        .map(|p| in_folder(&prefix, p))
        .collect())
}

/// Look at the repository for one git tool call.
pub(super) fn look_at(
    run: Run<'_>,
    look: &GitLook,
    blocked: &[String],
    base: Option<&str>,
) -> Result<Found, String> {
    match look {
        GitLook::Add { paths } => would_stage(run, paths).map(Found::Staging),
        GitLook::Diff { staged, path } => {
            let prefix = prefix(run)?;
            let mut args = vec!["diff", "--no-renames", "--name-only", "-z"];
            if *staged {
                args.push("--cached");
            }
            if let Some(p) = path {
                args.extend(["--", p.as_str()]);
            }
            let files = paths_z(&run(&args)?);
            Ok(Found::Showing(
                blocked_among(blocked, &prefix, &files)
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            ))
        }
        GitLook::Commit => {
            let prefix = prefix(run)?;
            let files = paths_z(&run(&[
                "diff",
                "--cached",
                "--no-renames",
                "--name-only",
                "-z",
            ])?);
            Ok(Found::Staged(
                blocked_among(blocked, &prefix, &files)
                    .iter()
                    .map(|p| in_folder(&prefix, p))
                    .collect(),
            ))
        }
        GitLook::Push { remote, branch } => Ok(Found::Sending(
            sending(run, remote, branch.as_deref(), base, blocked).ok(),
        )),
    }
}

/// A pathspec that leaves a file (named from the repository's top) out of a diff: that exact
/// path, not a pattern (`literal`), so a look-alike name is not left out with it.
fn leave_out(from_top: &str) -> String {
    format!(":(top,literal,exclude){from_top}")
}

/// One line for the worker: how many files a diff left out.
fn not_shown(n: usize) -> String {
    format!("{n} file(s) on the blocked list are not shown.")
}

/// A push's approval card: the push line, then what the look found, then the rest (for a pull
/// request, gh's line and the worker's own text, which cannot then pass for Plenipo's words).
fn with_push_note(detail: &str, note: &str) -> String {
    match detail.split_once('\n') {
        Some((push, rest)) => format!("{push}\n{note}\n{rest}"),
        None => format!("{detail}\n{note}"),
    }
}

/// The note under a push line.
fn push_note(found: Option<&[String]>) -> String {
    match found {
        None => "Plenipo could not check for blocked files in these commits.".into(),
        Some([]) => "No files on your blocked list are in these commits.".into(),
        Some(files) => format!(
            "These commits change files on your blocked list: {}.",
            files.join(", ")
        ),
    }
}

/// Plain words for a refused commit (shown after "Blocked:").
fn staged_reason(files: &[String]) -> String {
    if files.len() == 1 {
        format!("A blocked file is staged: {}. Unstage it first.", files[0])
    } else {
        format!(
            "Blocked files are staged: {}. Unstage them first.",
            files.join(", ")
        )
    }
}

/// Bring what the look found into the prepared call: the files Guard checks, the diff's
/// pathspec and note, the refusal, or the card's line.
pub(super) fn apply(prepared: &mut Prepared, found: Found, root: &Path) -> Result<(), Stopped> {
    match found {
        Found::Staging(files) => {
            prepared.files.extend(files.into_iter().map(|rel| Resolved {
                abs: root.join(&rel),
                rel,
                exists: true,
                root: root.to_owned(),
            }));
        }
        Found::Showing(from_top) => {
            if from_top.is_empty() {
                return Ok(());
            }
            let excludes: Vec<String> = from_top.iter().map(|p| leave_out(p)).collect();
            if let Work::Program { args, .. } = &mut prepared.work {
                if !args.iter().any(|a| a == "--") {
                    args.push("--".into());
                }
                args.extend(excludes.iter().cloned());
            }
            if !prepared.detail.contains(" -- ") {
                prepared.detail.push_str(" --");
            }
            prepared
                .detail
                .push_str(&format!(" {}", excludes.join(" ")));
            prepared.note = Some(not_shown(from_top.len()));
        }
        Found::Staged(files) => {
            if !files.is_empty() {
                return Err(Stopped::Refused(Refused {
                    layer: Layer::Rule,
                    reason: staged_reason(&files),
                    summary: prepared.summary.clone(),
                }));
            }
        }
        Found::Sending(found) => {
            prepared.detail = with_push_note(&prepared.detail, &push_note(found.as_deref()));
        }
    }
    Ok(())
}

impl Broker {
    /// Look at the repository for a git tool call and bring what is found into it, before
    /// Guard decides. Plenipo's own git does the looking (hooks off, no prompts, a time limit).
    pub(super) async fn look_at_git(
        &self,
        prepared: &mut Prepared,
        look: GitLook,
        ws: Option<&Workspace>,
        base: Option<&str>,
        blocked: &[String],
    ) -> Result<(), Stopped> {
        let Some(ws) = ws else {
            return Ok(());
        };
        let is_push = matches!(look, GitLook::Push { .. });
        let unchecked =
            |why: String| format!("Plenipo could not check the blocked-files list for this: {why}");
        let Some(git) = self.inner.git.clone() else {
            // Git is not installed: the call itself says so.
            if matches!(prepared.work, Work::Missing(_)) {
                return Ok(());
            }
            return if is_push {
                apply(prepared, Found::Sending(None), ws.root())
            } else {
                Err(Stopped::Unchecked(unchecked(
                    "git could not be found".into(),
                )))
            };
        };
        let root = ws.root().to_path_buf();
        let blocked = blocked.to_vec();
        let base = base.map(str::to_owned);
        let looked = tokio::task::spawn_blocking(move || {
            let run = |args: &[&str]| git.run(&root, args);
            look_at(&run, &look, &blocked, base.as_deref())
        })
        .await
        .unwrap_or_else(|e| Err(format!("the look at the repository stopped: {e}")));
        match looked {
            Ok(found) => apply(prepared, found, ws.root()),
            Err(_) if is_push => apply(prepared, Found::Sending(None), ws.root()),
            Err(why) => Err(Stopped::Unchecked(unchecked(why))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worktrees::Git;
    use plenipo_guard::defaults::default_blocked_files;

    /// A repository on `main` with one commit (`a.txt`, `sub/b.txt`), Plenipo's git, and a
    /// runner in `folder` (the top, or `sub`).
    struct Repo {
        dir: tempfile::TempDir,
        git: Git,
    }

    impl Repo {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let top = dir.path().join("repo");
            std::fs::create_dir_all(top.join("sub/config")).unwrap();
            std::fs::write(top.join("a.txt"), "a\n").unwrap();
            std::fs::write(top.join("sub/b.txt"), "b\n").unwrap();
            let git = Git::find(dir.path().join(".no-hooks")).expect("git on PATH");
            let g = |args: &[&str]| git.run(&top, args).unwrap();
            g(&["init", "-q", "-b", "main"]);
            g(&["config", "user.name", "Plenipo Test"]);
            g(&["config", "user.email", "test@example.com"]);
            g(&["add", "-A"]);
            g(&["commit", "-q", "-m", "Start"]);
            Self { dir, git }
        }

        fn top(&self) -> std::path::PathBuf {
            self.dir.path().join("repo")
        }

        fn write(&self, rel: &str, text: &str) {
            std::fs::write(self.top().join(rel), text).unwrap();
        }

        fn git(&self, args: &[&str]) -> String {
            self.git.run(&self.top(), args).unwrap()
        }

        /// A runner in the repository's top.
        fn at_top(&self) -> impl Fn(&[&str]) -> Result<String, String> + '_ {
            move |args: &[&str]| self.git.run(&self.top(), args)
        }

        /// A runner in `sub`.
        fn in_sub(&self) -> impl Fn(&[&str]) -> Result<String, String> + '_ {
            move |args: &[&str]| self.git.run(&self.top().join("sub"), args)
        }
    }

    fn blocked() -> Vec<String> {
        default_blocked_files()
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn git_add_would_stage_new_changed_and_deleted_files_but_not_ignored_ones() {
        let r = Repo::new();
        r.write(".env.local", "SECRET=1\n");
        r.write("notes.txt", "n\n");
        r.write("ignored.txt", "i\n");
        r.write(".gitignore", "ignored.txt\n");
        r.write("a.txt", "changed\n");
        std::fs::remove_file(r.top().join("sub/b.txt")).unwrap();
        let run = r.at_top();
        let all = would_stage(&run, &strings(&["."])).unwrap();
        assert_eq!(
            all,
            strings(&[
                ".env.local",
                ".gitignore",
                "a.txt",
                "notes.txt",
                "sub/b.txt"
            ])
        );
        assert_eq!(
            would_stage(&run, &strings(&["notes.txt"])).unwrap(),
            strings(&["notes.txt"])
        );
        // Nothing matching: nothing listed (git add itself then says so).
        assert!(would_stage(&run, &strings(&["nope"])).unwrap().is_empty());
        // Already staged and unchanged since: nothing to stage.
        r.git(&["add", "notes.txt"]);
        assert!(would_stage(&run, &strings(&["notes.txt"]))
            .unwrap()
            .is_empty());
        // The default blocked list catches the secret among them.
        let found = look_at(
            &run,
            &GitLook::Add {
                paths: strings(&["."]),
            },
            &blocked(),
            None,
        );
        let Ok(Found::Staging(files)) = found else {
            panic!("{found:?}");
        };
        let hit: Vec<&String> = files
            .iter()
            .filter(|f| blocked_by(&blocked(), f).is_some())
            .collect();
        assert_eq!(hit, [".env.local"]);
    }

    #[test]
    fn a_staged_blocked_file_stops_a_commit_and_a_clean_index_does_not() {
        let r = Repo::new();
        r.write("a.txt", "changed\n");
        r.git(&["add", "a.txt"]);
        let run = r.at_top();
        assert_eq!(
            look_at(&run, &GitLook::Commit, &blocked(), None).unwrap(),
            Found::Staged(vec![])
        );
        r.write(".env.local", "SECRET=1\n");
        r.write("sub/config/server.key", "KEY\n");
        r.git(&["add", ".env.local", "sub/config/server.key"]);
        assert_eq!(
            look_at(&run, &GitLook::Commit, &blocked(), None).unwrap(),
            Found::Staged(strings(&[".env.local", "sub/config/server.key"]))
        );
        assert_eq!(
            staged_reason(&strings(&[".env.local"])),
            "A blocked file is staged: .env.local. Unstage it first."
        );
        assert_eq!(
            staged_reason(&strings(&[".env.local", "sub/config/server.key"])),
            "Blocked files are staged: .env.local, sub/config/server.key. Unstage them first."
        );
    }

    #[test]
    fn a_diff_leaves_blocked_files_out_and_counts_them() {
        let r = Repo::new();
        r.write("a.txt", "changed\n");
        r.write(".env.local", "SECRET=1\n");
        r.git(&["add", "a.txt", ".env.local"]);
        r.write(".env.local", "SECRET=1\nMORE=2\n");
        r.write("sub/b.txt", "changed too\n");
        let run = r.at_top();
        // Staged: the secret is among the files; the pathspec leaves it out.
        let staged = GitLook::Diff {
            staged: true,
            path: None,
        };
        let Found::Showing(out) = look_at(&run, &staged, &blocked(), None).unwrap() else {
            panic!("not a diff");
        };
        assert_eq!(out, strings(&[".env.local"]));
        let shown = r.git(&["diff", "--cached", "--", &leave_out(&out[0])]);
        assert!(
            shown.contains("a.txt") && shown.contains("+changed"),
            "{shown}"
        );
        assert!(!shown.contains("SECRET"), "{shown}");
        // Not staged: the same.
        let unstaged = GitLook::Diff {
            staged: false,
            path: None,
        };
        assert_eq!(
            look_at(&run, &unstaged, &blocked(), None).unwrap(),
            Found::Showing(strings(&[".env.local"]))
        );
        let shown = r.git(&["diff", "--", &leave_out(".env.local")]);
        assert!(shown.contains("sub/b.txt"), "{shown}");
        assert!(!shown.contains("MORE"), "{shown}");
        // Limited to a folder without blocked files: nothing to leave out.
        let sub = GitLook::Diff {
            staged: false,
            path: Some("sub".into()),
        };
        assert_eq!(
            look_at(&run, &sub, &blocked(), None).unwrap(),
            Found::Showing(vec![])
        );
        assert_eq!(not_shown(1), "1 file(s) on the blocked list are not shown.");
    }

    /// A blocked file renamed (which git would list under its new name only) is seen under both
    /// names: a diff leaves both out, and a commit names both.
    #[test]
    fn a_renamed_blocked_file_is_seen_under_both_names() {
        let r = Repo::new();
        r.write(".env.local", "SECRET=1\n");
        r.git(&["add", ".env.local"]);
        r.git(&["commit", "-q", "-m", "Add the secret"]);
        r.git(&["mv", ".env.local", ".env.production"]);
        r.write("a.txt", "changed\n");
        r.git(&["add", "a.txt"]);
        let run = r.at_top();
        let staged = GitLook::Diff {
            staged: true,
            path: None,
        };
        let Found::Showing(out) = look_at(&run, &staged, &blocked(), None).unwrap() else {
            panic!("not a diff");
        };
        assert_eq!(out, strings(&[".env.local", ".env.production"]));
        let excludes: Vec<String> = out.iter().map(|p| leave_out(p)).collect();
        let mut args = vec!["diff", "--cached", "--"];
        args.extend(excludes.iter().map(String::as_str));
        let shown = r.git(&args);
        assert!(
            shown.contains("a.txt") && shown.contains("+changed"),
            "{shown}"
        );
        assert!(
            !shown.contains("SECRET") && !shown.contains(".env"),
            "{shown}"
        );
        assert_eq!(
            look_at(&run, &GitLook::Commit, &blocked(), None).unwrap(),
            Found::Staged(strings(&[".env.local", ".env.production"]))
        );
        // The exact path is left out, not a look-alike: `[` in a name is no pattern.
        r.write("a.txt", "a\n");
        r.git(&["add", "a.txt"]);
        r.write("config.[k]ey", "K\n");
        r.write("config.key", "K2\n");
        r.git(&["add", "config.[k]ey", "config.key"]);
        let shown = r.git(&["diff", "--cached", "--", &leave_out("config.[k]ey")]);
        assert!(shown.contains("+K2"), "{shown}");
        assert!(!shown.contains("+K\n"), "{shown}");
    }

    /// From a folder inside the repository, blocked files are matched as that folder sees them
    /// and left out by their path from the top.
    #[test]
    fn a_folder_inside_the_repository_sees_its_own_paths() {
        let r = Repo::new();
        r.write("sub/config/.env.local", "SECRET=1\n");
        r.write("sub/b.txt", "changed\n");
        let run = r.in_sub();
        assert_eq!(
            would_stage(&run, &strings(&["."])).unwrap(),
            strings(&["b.txt", "config/.env.local"])
        );
        r.git(&["add", "sub"]);
        assert_eq!(
            look_at(&run, &GitLook::Commit, &blocked(), None).unwrap(),
            Found::Staged(strings(&["config/.env.local"]))
        );
        let staged = GitLook::Diff {
            staged: true,
            path: Some("config".into()),
        };
        let Found::Showing(out) = look_at(&run, &staged, &blocked(), None).unwrap() else {
            panic!("not a diff");
        };
        assert_eq!(out, strings(&["sub/config/.env.local"]));
        let shown = r
            .git
            .run(
                &r.top().join("sub"),
                &["diff", "--cached", "--", "config", &leave_out(&out[0])],
            )
            .unwrap();
        assert!(shown.trim().is_empty(), "{shown}");
    }

    #[test]
    fn a_push_names_blocked_files_in_the_commits_it_would_send() {
        let r = Repo::new();
        let run = r.at_top();
        let push = GitLook::Push {
            remote: "origin".into(),
            branch: None,
        };
        // No remote at all: it cannot be checked, and says so.
        assert_eq!(
            look_at(&run, &push, &blocked(), None).unwrap(),
            Found::Sending(None)
        );
        let origin = r.dir.path().join("origin.git");
        r.git
            .run(r.dir.path(), &["init", "-q", "--bare", "origin.git"])
            .unwrap();
        r.git(&["remote", "add", "origin", &origin.display().to_string()]);
        r.git(&["push", "-q", "-u", "origin", "main"]);
        // Nothing new: nothing to name.
        assert_eq!(
            look_at(&run, &push, &blocked(), None).unwrap(),
            Found::Sending(Some(vec![]))
        );
        r.write(".env.local", "SECRET=1\n");
        r.write("notes.txt", "n\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "Add notes and the secret"]);
        assert_eq!(
            look_at(&run, &push, &blocked(), None).unwrap(),
            Found::Sending(Some(strings(&[".env.local"])))
        );
        // A branch never pushed, with no upstream: compared with the remote's copy of its base.
        r.git(&["push", "-q", "origin", "main"]);
        r.git(&["switch", "-q", "-c", "plenipo/feature"]);
        r.write("id_rsa", "KEY\n");
        r.git(&["add", "id_rsa"]);
        r.git(&["commit", "-q", "-m", "Add a key"]);
        let own = GitLook::Push {
            remote: "origin".into(),
            branch: Some("plenipo/feature".into()),
        };
        assert_eq!(
            look_at(&run, &own, &blocked(), Some("main")).unwrap(),
            Found::Sending(Some(strings(&["id_rsa"])))
        );
        // Without a base named, the remote's default branch (or its main) is used.
        assert_eq!(
            look_at(&run, &own, &blocked(), None).unwrap(),
            Found::Sending(Some(strings(&["id_rsa"])))
        );
        // A base the remote has no copy of: compared with the base itself, so the commits the
        // base carries are not counted twice.
        r.git(&["switch", "-q", "-c", "develop", "main"]);
        r.write("notes.txt", "more\n");
        r.git(&["commit", "-q", "-am", "More notes"]);
        r.git(&["switch", "-q", "-c", "plenipo/other"]);
        r.write("secrets.pem", "PEM\n");
        r.git(&["add", "secrets.pem"]);
        r.git(&["commit", "-q", "-m", "Add a certificate"]);
        let other = GitLook::Push {
            remote: "origin".into(),
            branch: Some("plenipo/other".into()),
        };
        assert_eq!(
            look_at(&run, &other, &blocked(), Some("develop")).unwrap(),
            Found::Sending(Some(strings(&["secrets.pem"])))
        );
        // The remote's default branch: only when no upstream, remote copy, or base fits.
        assert_eq!(
            look_at(&run, &other, &blocked(), None).unwrap(),
            Found::Sending(Some(strings(&["secrets.pem"])))
        );
        assert_eq!(
            push_note(None),
            "Plenipo could not check for blocked files in these commits."
        );
        assert_eq!(
            push_note(Some(&[])),
            "No files on your blocked list are in these commits."
        );
        assert_eq!(
            push_note(Some(&strings(&["id_rsa"]))),
            "These commits change files on your blocked list: id_rsa."
        );
        assert_eq!(
            with_push_note("git push origin main", "note"),
            "git push origin main\nnote"
        );
        assert_eq!(
            with_push_note("git push -u origin b\ngh pr create\n\nbody", "note"),
            "git push -u origin b\nnote\ngh pr create\n\nbody"
        );
    }
}
