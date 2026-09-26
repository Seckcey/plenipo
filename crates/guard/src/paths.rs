//! Folder confinement: every path a worker names is resolved inside its project folder, or
//! refused. Refused: `..` that climbs out, absolute paths elsewhere, symbolic links that lead
//! out, Windows device names, drive-relative and alternate-stream names (`C:x`, `file:stream`),
//! and names Windows would silently change (trailing dots or spaces). Blocked-file patterns are
//! matched against the real (canonical) path, so a short 8.3 name or a link cannot dodge them.

use std::path::{Component, Path, PathBuf};

/// Longest path accepted from a worker.
pub const MAX_PATH_CHARS: usize = 1000;

/// A project folder, resolved once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// Canonical absolute path.
    root: PathBuf,
}

/// A path inside the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// Absolute path (canonical up to the deepest part that exists).
    pub abs: PathBuf,
    /// Relative to the workspace, with `/` separators; empty for the folder itself.
    pub rel: String,
    pub exists: bool,
}

impl Resolved {
    /// How to show it: the relative path, or `.` for the folder itself.
    pub fn shown(&self) -> &str {
        if self.rel.is_empty() {
            "."
        } else {
            &self.rel
        }
    }

    /// Inside a `.git` folder (git's own files, changed only through the git tools).
    pub fn in_git_dir(&self) -> bool {
        self.rel.split('/').any(|c| c.eq_ignore_ascii_case(".git"))
    }
}

/// Why a path was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathRefusal {
    /// Not a usable path at all.
    Invalid(String),
    /// It would leave the workspace.
    Outside(String),
}

impl PathRefusal {
    pub fn message(&self) -> &str {
        match self {
            Self::Invalid(m) | Self::Outside(m) => m,
        }
    }
}

const DEVICE_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9", "conin$",
    "conout$",
];

/// True when `input` names an absolute location (either platform's style).
fn looks_absolute(input: &str) -> bool {
    let b = input.as_bytes();
    input.starts_with('/')
        || input.starts_with('\\')
        || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}

/// Check one name the worker wrote (a path component).
fn check_name(name: &str) -> Result<(), PathRefusal> {
    if name == "." || name == ".." {
        return Ok(());
    }
    if name.contains(':') {
        return Err(PathRefusal::Invalid(format!(
            "\"{name}\" is not allowed: a colon in a name selects a drive or a hidden stream"
        )));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(PathRefusal::Invalid(format!(
            "\"{name}\" is not allowed: Windows ignores dots and spaces at the end of a name"
        )));
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    if DEVICE_NAMES.contains(&stem.trim_end()) {
        return Err(PathRefusal::Invalid(format!(
            "\"{name}\" is a reserved device name on Windows"
        )));
    }
    if name
        .chars()
        .any(|c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Err(PathRefusal::Invalid(format!(
            "\"{name}\" contains a character that cannot be used in file names"
        )));
    }
    Ok(())
}

/// `path` with `.` and `..` resolved without touching the disk. `None` when `..` climbs above
/// the path's root.
fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for c in path.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => out.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return None;
                }
                out.pop();
                depth -= 1;
            }
            Component::Normal(n) => {
                out.push(n);
                depth += 1;
            }
        }
    }
    Some(out)
}

fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    dunce::canonicalize(path)
}

impl Workspace {
    /// The project folder at `path`, which must be an existing folder.
    pub fn open(path: &str) -> Result<Self, String> {
        let p = Path::new(path.trim());
        if !p.is_absolute() {
            return Err(format!("the project folder {path} is not an absolute path"));
        }
        let root = canonical(p)
            .map_err(|_| format!("the project folder {path} does not exist or cannot be opened"))?;
        if !root.is_dir() {
            return Err(format!("the project folder {path} is not a folder"));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a path the worker wrote (relative to the folder, or absolute inside it).
    pub fn resolve(&self, input: &str) -> Result<Resolved, PathRefusal> {
        let input = input.trim();
        let input = if input.is_empty() { "." } else { input };
        if input.chars().count() > MAX_PATH_CHARS {
            return Err(PathRefusal::Invalid(format!(
                "paths are limited to {MAX_PATH_CHARS} characters"
            )));
        }
        if input.chars().any(char::is_control) {
            return Err(PathRefusal::Invalid(
                "paths cannot contain control characters".into(),
            ));
        }
        if input.starts_with("\\\\?\\")
            || input.starts_with("\\\\.\\")
            || input.starts_with("//?/")
            || input.starts_with("//./")
        {
            return Err(PathRefusal::Invalid(
                "device paths (\\\\?\\ and \\\\.\\) are not allowed".into(),
            ));
        }
        // Backslashes separate folders on Windows; elsewhere they would become part of a name.
        let input_owned;
        let input = if cfg!(windows) {
            input
        } else {
            input_owned = input.replace('\\', "/");
            input_owned.as_str()
        };
        let candidate = if looks_absolute(input) {
            let p = PathBuf::from(input);
            if !p.is_absolute() {
                // e.g. "C:foo" or "\foo" on Windows, or "C:\x" on Unix.
                return Err(PathRefusal::Outside(format!(
                    "{input} is outside the project folder; use a path inside it, such as \
                     src/main.rs"
                )));
            }
            p
        } else {
            for name in input.split(['/', '\\']) {
                check_name(name)?;
            }
            self.root.join(input)
        };
        let normalized = normalize(&candidate).ok_or_else(|| outside(input))?;
        // Follow links: the deepest part that exists must really be inside the folder (this
        // also settles differences in letter case on Windows).
        let mut existing = normalized.clone();
        let mut missing: Vec<std::ffi::OsString> = Vec::new();
        loop {
            match std::fs::symlink_metadata(&existing) {
                Ok(_) => break,
                Err(_) => {
                    let Some(name) = existing.file_name().map(|n| n.to_os_string()) else {
                        return Err(outside(input));
                    };
                    missing.push(name);
                    if !existing.pop() {
                        return Err(outside(input));
                    }
                }
            }
        }
        let real = canonical(&existing).map_err(|_| {
            PathRefusal::Invalid(format!("{input} goes through a link that leads nowhere"))
        })?;
        if !real.starts_with(&self.root) {
            return Err(if normalized.starts_with(&self.root) {
                PathRefusal::Outside(format!(
                    "{input} leads outside the project folder through a link"
                ))
            } else {
                outside(input)
            });
        }
        // Names still to be created must be ordinary names.
        for name in &missing {
            check_name(&name.to_string_lossy())?;
        }
        let exists = missing.is_empty();
        let mut abs = real;
        for name in missing.into_iter().rev() {
            abs.push(name);
        }
        let rel = abs
            .strip_prefix(&self.root)
            .map_err(|_| outside(input))?
            .components()
            .filter_map(|c| match c {
                Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/");
        Ok(Resolved { abs, rel, exists })
    }
}

fn outside(input: &str) -> PathRefusal {
    PathRefusal::Outside(format!(
        "{input} is outside the project folder; workers can use files only inside it"
    ))
}

// ---- Patterns ----------------------------------------------------------------------------

/// `text` matches the glob `pattern`: `*` any characters except `/`, `**` any characters,
/// `?` one character. ASCII case-insensitive.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    glob_from(&p, &t)
}

fn glob_from(p: &[char], t: &[char]) -> bool {
    match p.first() {
        None => t.is_empty(),
        Some('*') if p.get(1) == Some(&'*') => {
            let rest = p[2..].strip_prefix(&['/']).unwrap_or(&p[2..]);
            (0..=t.len()).any(|i| glob_from(rest, &t[i..]))
                || (0..=t.len()).any(|i| glob_from(&p[2..], &t[i..]))
        }
        Some('*') => {
            let rest = &p[1..];
            let mut i = 0;
            loop {
                if glob_from(rest, &t[i..]) {
                    return true;
                }
                if i == t.len() || t[i] == '/' {
                    return false;
                }
                i += 1;
            }
        }
        Some('?') => !t.is_empty() && t[0] != '/' && glob_from(&p[1..], &t[1..]),
        Some(c) => !t.is_empty() && t[0] == *c && glob_from(&p[1..], &t[1..]),
    }
}

/// One blocked-file pattern matches `rel`: a pattern without `/` matches any name along the
/// path (a matching folder blocks everything in it); one with `/` matches from the folder's
/// top, and blocks everything under a matching folder too.
fn pattern_matches(pattern: &str, rel: &str) -> bool {
    let pattern = pattern.trim().trim_end_matches('/');
    if pattern.is_empty() || rel.is_empty() {
        return false;
    }
    let parts: Vec<&str> = rel.split('/').collect();
    if !pattern.contains('/') {
        return parts.iter().any(|name| glob_match(pattern, name));
    }
    let pattern = pattern.trim_start_matches('/');
    (1..=parts.len()).any(|n| glob_match(pattern, &parts[..n].join("/")))
}

/// Whether `rel` is a blocked file under `patterns`, gitignore-style: a later match wins, and
/// `!pattern` makes an exception. Returns the pattern that blocks it.
pub fn blocked_by<'a>(patterns: &'a [String], rel: &str) -> Option<&'a str> {
    let mut hit: Option<&str> = None;
    for p in patterns {
        let p = p.trim();
        if let Some(exception) = p.strip_prefix('!') {
            if pattern_matches(exception, rel) {
                hit = None;
            }
        } else if pattern_matches(p, rel) {
            hit = Some(p);
        }
    }
    hit
}

/// A blocked-file pattern as the owner writes it: 1–200 characters, one line, not only `!`.
pub fn valid_pattern(p: &str) -> Result<String, String> {
    let p = p.trim();
    let body = p.strip_prefix('!').unwrap_or(p);
    if body.trim().is_empty() || p.chars().count() > 200 || p.chars().any(char::is_control) {
        return Err(format!("{p:?} is not a usable file pattern"));
    }
    Ok(p.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("proj/src")).unwrap();
        std::fs::write(dir.path().join("proj/src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "outside").unwrap();
        let w = Workspace::open(&dir.path().join("proj").display().to_string()).unwrap();
        (dir, w)
    }

    #[test]
    fn paths_inside_resolve() {
        let (_d, w) = ws();
        let r = w.resolve("src/main.rs").unwrap();
        assert_eq!(r.rel, "src/main.rs");
        assert!(r.exists);
        assert_eq!(w.resolve("").unwrap().rel, "");
        assert_eq!(w.resolve(".").unwrap().shown(), ".");
        assert_eq!(
            w.resolve("./src/../src/main.rs").unwrap().rel,
            "src/main.rs"
        );
        let new = w.resolve("src/new/file.txt").unwrap();
        assert!(!new.exists);
        assert_eq!(new.rel, "src/new/file.txt");
        // An absolute path inside the folder is fine.
        let abs = w.root().join("src").join("main.rs");
        assert_eq!(
            w.resolve(&abs.display().to_string()).unwrap().rel,
            "src/main.rs"
        );
        assert!(w.resolve(".git/config").unwrap().in_git_dir());
        assert!(!w.resolve("src/git/x").unwrap().in_git_dir());
    }

    #[test]
    fn traversal_is_refused() {
        let (dir, w) = ws();
        for bad in [
            "../secret.txt",
            "src/../../secret.txt",
            "..",
            "src/../../proj/../secret.txt",
        ] {
            assert!(
                matches!(w.resolve(bad), Err(PathRefusal::Outside(_))),
                "{bad}"
            );
        }
        let elsewhere = dir.path().join("secret.txt").display().to_string();
        assert!(matches!(
            w.resolve(&elsewhere),
            Err(PathRefusal::Outside(_))
        ));
        assert!(matches!(
            w.resolve("/etc/passwd"),
            Err(PathRefusal::Outside(_))
        ));
        for bad in [
            "src/con",
            "src/NUL.txt",
            "a:b",
            "file.txt:stream",
            ".env.",
            "dir /file",
            "\\\\?\\C:\\x",
            "a\u{0}b",
            "src/*.rs",
        ] {
            assert!(w.resolve(bad).is_err(), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn links_that_leave_are_refused() {
        let (dir, w) = ws();
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), w.root().join("link")).unwrap();
        std::os::unix::fs::symlink(dir.path(), w.root().join("up")).unwrap();
        std::os::unix::fs::symlink(w.root().join("src"), w.root().join("inner")).unwrap();
        assert!(matches!(w.resolve("link"), Err(PathRefusal::Outside(_))));
        assert!(matches!(
            w.resolve("up/secret.txt"),
            Err(PathRefusal::Outside(_))
        ));
        assert!(matches!(
            w.resolve("up/new.txt"),
            Err(PathRefusal::Outside(_))
        ));
        // A link that stays inside resolves to where it really is.
        assert_eq!(w.resolve("inner/main.rs").unwrap().rel, "src/main.rs");
        std::os::unix::fs::symlink(w.root().join("nowhere"), w.root().join("dangling")).unwrap();
        assert!(w.resolve("dangling").is_err());
    }

    #[test]
    fn workspace_must_exist() {
        assert!(Workspace::open("relative/path").is_err());
        let dir = tempfile::tempdir().unwrap();
        assert!(Workspace::open(&dir.path().join("missing").display().to_string()).is_err());
        std::fs::write(dir.path().join("f"), "").unwrap();
        assert!(Workspace::open(&dir.path().join("f").display().to_string()).is_err());
    }

    #[test]
    fn globs() {
        assert!(glob_match("*.pem", "server.PEM"));
        assert!(!glob_match("*.pem", "keys/server.pem"));
        assert!(glob_match("**/*.pem", "keys/server.pem"));
        assert!(glob_match("**/*.pem", "server.pem"));
        assert!(glob_match("id_rsa*", "id_rsa.pub"));
        assert!(glob_match("?.txt", "a.txt"));
        assert!(!glob_match("?.txt", "ab.txt"));
        assert!(glob_match("config/**", "config/a/b"));
    }

    #[test]
    fn blocked_files_follow_gitignore_rules() {
        let p: Vec<String> = [
            ".env",
            ".env.*",
            "!.env.example",
            "*.pem",
            "secrets/",
            "/local.key",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(blocked_by(&p, ".env"), Some(".env"));
        assert_eq!(blocked_by(&p, "app/.env.local"), Some(".env.*"));
        assert_eq!(blocked_by(&p, ".env.example"), None);
        assert_eq!(blocked_by(&p, "certs/site.PEM"), Some("*.pem"));
        assert_eq!(blocked_by(&p, "secrets/db.txt"), Some("secrets/"));
        assert_eq!(blocked_by(&p, "local.key"), Some("/local.key"));
        assert_eq!(blocked_by(&p, "sub/local.key"), None);
        assert_eq!(blocked_by(&p, "src/main.rs"), None);
        assert_eq!(blocked_by(&p, ""), None);
        assert!(valid_pattern("!").is_err());
        assert!(valid_pattern(" *.pem ").is_ok());
    }
}
