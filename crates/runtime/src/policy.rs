//! Executable allowlist, working-directory validation, and child environment isolation.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("path must be absolute: {0}")]
    NotAbsolute(String),
    #[error("path does not exist or is inaccessible: {0}")]
    NotFound(String),
    #[error("not a file: {0}")]
    NotAFile(String),
    #[error("not a directory: {0}")]
    NotADirectory(String),
    #[error("executable is not on the allowlist: {0}")]
    NotAllowed(String),
    #[error("invalid environment variable name: {0:?}")]
    InvalidEnvName(String),
}

/// Allowlist of executables, stored as canonical absolute paths.
///
/// A candidate is accepted only if it is absolute, exists, is a file, and its canonical
/// form (symlinks and `..` resolved) equals an allowlisted canonical path.
#[derive(Debug, Clone, Default)]
pub struct ExecutablePolicy {
    allowed: Vec<PathBuf>,
}

impl ExecutablePolicy {
    pub fn new<I, P>(paths: I) -> Result<Self, PolicyError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let allowed = paths
            .into_iter()
            .map(|p| canonical_file(p.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { allowed })
    }

    /// Validate `path` and return its canonical form.
    pub fn check(&self, path: &Path) -> Result<PathBuf, PolicyError> {
        let canonical = canonical_file(path)?;
        if self.allowed.iter().any(|a| a == &canonical) {
            Ok(canonical)
        } else {
            Err(PolicyError::NotAllowed(path.display().to_string()))
        }
    }

    pub fn allowed(&self) -> &[PathBuf] {
        &self.allowed
    }
}

fn canonical_file(path: &Path) -> Result<PathBuf, PolicyError> {
    let shown = || path.display().to_string();
    if !path.is_absolute() {
        return Err(PolicyError::NotAbsolute(shown()));
    }
    let canonical = dunce::canonicalize(path).map_err(|_| PolicyError::NotFound(shown()))?;
    if !canonical.is_file() {
        return Err(PolicyError::NotAFile(shown()));
    }
    Ok(canonical)
}

/// Validate a working directory and return its canonical form.
pub fn check_working_dir(path: &Path) -> Result<PathBuf, PolicyError> {
    let shown = || path.display().to_string();
    if !path.is_absolute() {
        return Err(PolicyError::NotAbsolute(shown()));
    }
    let canonical = dunce::canonicalize(path).map_err(|_| PolicyError::NotFound(shown()))?;
    if !canonical.is_dir() {
        return Err(PolicyError::NotADirectory(shown()));
    }
    Ok(canonical)
}

/// Variables passed through from Plenipo's own environment so ordinary programs can run.
/// Everything else — including any credentials in Plenipo's environment — is withheld.
#[cfg(windows)]
pub const BASELINE_ENV: &[&str] = &[
    "SystemRoot",
    "SystemDrive",
    "windir",
    "ComSpec",
    "PATH",
    "PATHEXT",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramData",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramW6432",
    "CommonProgramFiles",
    "HOMEDRIVE",
    "HOMEPATH",
    "USERNAME",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "OS",
];

#[cfg(not(windows))]
pub const BASELINE_ENV: &[&str] = &["PATH", "HOME", "USER", "LANG", "LC_ALL", "TMPDIR", "TZ"];

/// `[A-Za-z_][A-Za-z0-9_]*`, at most 128 bytes.
pub fn validate_env_name(name: &str) -> Result<(), PolicyError> {
    let mut chars = name.chars();
    let ok = name.len() <= 128
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(PolicyError::InvalidEnvName(name.to_owned()))
    }
}

/// Build the complete child environment: baseline pass-through plus declared variables.
/// Declared variables win over baseline ones with the same name.
pub fn build_child_env(declared: &[(String, String)]) -> Vec<(OsString, OsString)> {
    build_child_env_from(declared, |name| std::env::var_os(name))
}

fn build_child_env_from(
    declared: &[(String, String)],
    lookup: impl Fn(&str) -> Option<OsString>,
) -> Vec<(OsString, OsString)> {
    let is_declared = |name: &str| {
        declared
            .iter()
            .any(|(d, _)| d.eq_ignore_ascii_case(name) && (cfg!(windows) || d == name))
    };
    let mut env: Vec<(OsString, OsString)> = BASELINE_ENV
        .iter()
        .filter(|name| !is_declared(name))
        .filter_map(|name| lookup(name).map(|v| (OsString::from(name), v)))
        .collect();
    env.extend(
        declared
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v))),
    );
    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let allowed = dir.path().join("allowed.exe");
        let other = dir.path().join("other.exe");
        fs::write(&allowed, b"x").unwrap();
        fs::write(&other, b"x").unwrap();
        (dir, allowed, other)
    }

    #[test]
    fn accepts_allowlisted_executable() {
        let (_d, allowed, _) = setup();
        let policy = ExecutablePolicy::new([&allowed]).unwrap();
        assert_eq!(
            policy.check(&allowed).unwrap(),
            dunce::canonicalize(&allowed).unwrap()
        );
    }

    #[test]
    fn rejects_executable_not_on_allowlist() {
        let (_d, allowed, other) = setup();
        let policy = ExecutablePolicy::new([&allowed]).unwrap();
        assert!(matches!(
            policy.check(&other),
            Err(PolicyError::NotAllowed(_))
        ));
    }

    #[test]
    fn rejects_relative_paths() {
        let (_d, allowed, _) = setup();
        let policy = ExecutablePolicy::new([&allowed]).unwrap();
        assert!(matches!(
            policy.check(Path::new("allowed.exe")),
            Err(PolicyError::NotAbsolute(_))
        ));
        assert!(ExecutablePolicy::new(["relative.exe"]).is_err());
    }

    #[test]
    fn traversal_resolves_before_comparison() {
        let (d, allowed, _) = setup();
        let policy = ExecutablePolicy::new([&allowed]).unwrap();
        let sub = d.path().join("sub");
        fs::create_dir(&sub).unwrap();
        // `sub/../other.exe` must be judged as `other.exe`, not smuggled in.
        assert!(matches!(
            policy.check(&sub.join("..").join("other.exe")),
            Err(PolicyError::NotAllowed(_))
        ));
        // `sub/../allowed.exe` is the allowlisted file and is accepted.
        assert!(policy.check(&sub.join("..").join("allowed.exe")).is_ok());
    }

    #[test]
    fn rejects_missing_and_directories() {
        let (d, allowed, _) = setup();
        let policy = ExecutablePolicy::new([&allowed]).unwrap();
        assert!(matches!(
            policy.check(&d.path().join("missing.exe")),
            Err(PolicyError::NotFound(_))
        ));
        assert!(matches!(
            policy.check(d.path()),
            Err(PolicyError::NotAFile(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_disallowed_file_is_rejected() {
        let (d, allowed, other) = setup();
        let policy = ExecutablePolicy::new([&allowed]).unwrap();
        let link = d.path().join("looks-innocent.exe");
        std::os::unix::fs::symlink(&other, &link).unwrap();
        assert!(matches!(
            policy.check(&link),
            Err(PolicyError::NotAllowed(_))
        ));
    }

    #[test]
    fn working_dir_validation() {
        let (d, allowed, _) = setup();
        assert!(check_working_dir(d.path()).is_ok());
        assert!(matches!(
            check_working_dir(&allowed),
            Err(PolicyError::NotADirectory(_))
        ));
        assert!(matches!(
            check_working_dir(Path::new("relative")),
            Err(PolicyError::NotAbsolute(_))
        ));
        assert!(matches!(
            check_working_dir(&d.path().join("missing")),
            Err(PolicyError::NotFound(_))
        ));
    }

    #[test]
    fn env_names() {
        for good in ["PATH", "_X", "PLENIPO_DIAGNOSTIC_GREETING", "a1"] {
            assert!(validate_env_name(good).is_ok(), "{good}");
        }
        for bad in ["", "1A", "A-B", "A=B", "A B", "Ä"] {
            assert!(validate_env_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn child_env_is_baseline_plus_declared_only() {
        let parent = |name: &str| match name {
            "PATH" => Some(OsString::from("/bin")),
            "OPENAI_API_KEY" | "ANTHROPIC_API_KEY" => Some(OsString::from("secret")),
            _ => None,
        };
        let declared = vec![("GREETING".to_owned(), "hello".to_owned())];
        let env = build_child_env_from(&declared, parent);
        let names: Vec<_> = env.iter().map(|(k, _)| k.to_string_lossy()).collect();
        assert!(names.contains(&"PATH".into()));
        assert!(names.contains(&"GREETING".into()));
        assert!(!names.iter().any(|n| n.contains("API_KEY")));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn declared_overrides_baseline() {
        let parent = |name: &str| (name == "PATH").then(|| OsString::from("/bin"));
        let declared = vec![("PATH".to_owned(), "/custom".to_owned())];
        let env = build_child_env_from(&declared, parent);
        assert_eq!(
            env,
            vec![(OsString::from("PATH"), OsString::from("/custom"))]
        );
    }
}
