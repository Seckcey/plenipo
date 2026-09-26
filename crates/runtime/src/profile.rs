//! Launch profiles: the only things the UI can ask the runtime to start.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::diagnostic::{self, Scenario};
use crate::dto::LaunchProfileInfo;
use crate::policy::{validate_env_name, ExecutablePolicy};

pub const MAX_RUNTIME_LIMIT: Duration = Duration::from_secs(24 * 60 * 60);
/// Upper bound for [`LaunchSpec::max_line_bytes`].
pub const MAX_OBSERVED_LINE_BYTES: usize = 16 * 1024 * 1024;

/// A fully specified, pre-approved launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchProfile {
    pub id: String,
    pub label: String,
    pub description: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
    /// The only variables added to the child's baseline environment.
    pub env: Vec<(String, String)>,
    pub working_dir: PathBuf,
    /// Hard limit; the process tree is terminated when exceeded.
    pub max_runtime: Duration,
}

/// One fully specified launch, built by Core: from a registered [`LaunchProfile`], or by an
/// agent runtime adapter from validated inputs. Never constructed from UI input directly.
/// The executable must still pass the supervisor's allowlist at spawn.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub profile_id: String,
    pub label: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
    /// The only variables added to the child's baseline environment.
    pub env: Vec<(String, String)>,
    pub working_dir: PathBuf,
    pub max_runtime: Duration,
    /// Written to the child's stdin, which is then closed. `None`: stdin is empty.
    pub stdin: Option<Vec<u8>>,
    /// Longest line delivered to `observer` (lines shown in the UI keep the configured cap).
    /// `None`: the supervisor's configured per-line limit.
    pub max_line_bytes: Option<usize>,
    /// Receives every output line, in order, with its full text; closes after the process's
    /// output ends. Unbounded on purpose: a slow observer must never stall reading the
    /// child's pipes (which would trip the drain timeout and lose the final lines).
    pub observer: Option<tokio::sync::mpsc::UnboundedSender<crate::dto::OutputLine>>,
    pub agent: Option<Box<crate::dto::AgentAttribution>>,
}

impl LaunchProfile {
    /// The launch this profile describes (no stdin, no observer).
    pub fn to_spec(&self) -> LaunchSpec {
        LaunchSpec {
            profile_id: self.id.clone(),
            label: self.label.clone(),
            executable: self.executable.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            working_dir: self.working_dir.clone(),
            max_runtime: self.max_runtime,
            stdin: None,
            max_line_bytes: None,
            observer: None,
            agent: None,
        }
    }

    pub fn info(&self) -> LaunchProfileInfo {
        LaunchProfileInfo {
            id: self.id.clone(),
            label: self.label.clone(),
            description: self.description.clone(),
            max_runtime_secs: self.max_runtime.as_secs(),
        }
    }

    fn validate(&self, policy: &ExecutablePolicy) -> Result<(), String> {
        validate_profile_id(&self.id)?;
        if self.label.trim().is_empty() {
            return Err("label must not be empty".into());
        }
        policy.check(&self.executable).map_err(|e| e.to_string())?;
        if !self.working_dir.is_absolute() {
            return Err("working directory must be absolute".into());
        }
        for (name, _) in &self.env {
            validate_env_name(name).map_err(|e| e.to_string())?;
        }
        if self.max_runtime.is_zero() || self.max_runtime > MAX_RUNTIME_LIMIT {
            return Err("max runtime must be between 1 second and 24 hours".into());
        }
        Ok(())
    }
}

/// `[a-z0-9][a-z0-9.-]{0,63}`
pub fn validate_profile_id(id: &str) -> Result<(), String> {
    let mut chars = id.chars();
    let ok = (1..=64).contains(&id.len())
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(format!("invalid profile id: {id:?}"))
    }
}

/// Validated set of launch profiles.
#[derive(Debug, Clone, Default)]
pub struct ProfileRegistry {
    profiles: Vec<LaunchProfile>,
}

impl ProfileRegistry {
    /// Register profiles, rejecting any that are invalid or duplicated.
    /// Returns the registry plus one message per rejected profile.
    pub fn new(
        candidates: impl IntoIterator<Item = LaunchProfile>,
        policy: &ExecutablePolicy,
    ) -> (Self, Vec<String>) {
        let mut profiles: Vec<LaunchProfile> = Vec::new();
        let mut rejected = Vec::new();
        for profile in candidates {
            if profiles.iter().any(|p| p.id == profile.id) {
                rejected.push(format!("Launch profile {:?} is duplicated", profile.id));
            } else if let Err(reason) = profile.validate(policy) {
                rejected.push(format!(
                    "Launch profile {:?} rejected: {reason}",
                    profile.id
                ));
            } else {
                profiles.push(profile);
            }
        }
        (Self { profiles }, rejected)
    }

    pub fn get(&self, id: &str) -> Option<&LaunchProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn infos(&self) -> Vec<LaunchProfileInfo> {
        self.profiles.iter().map(LaunchProfile::info).collect()
    }
}

/// Built-in diagnostic profiles that run `executable` (Plenipo itself) in diagnostic mode.
pub fn diagnostic_profiles(executable: &Path, working_dir: &Path) -> Vec<LaunchProfile> {
    let make =
        |id: &str, label: &str, description: &str, scenario: Scenario, secs: u64| LaunchProfile {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            executable: executable.to_path_buf(),
            args: vec![diagnostic::arg(scenario)],
            env: vec![],
            working_dir: working_dir.to_path_buf(),
            max_runtime: Duration::from_secs(secs),
        };
    let mut echo = make(
        "diagnostic.echo",
        "Echo test",
        "Prints stdout and stderr lines over about two seconds, then exits 0.",
        Scenario::Echo,
        60,
    );
    echo.env = vec![(diagnostic::GREETING_VAR.into(), "hello from Plenipo".into())];
    vec![
        echo,
        make(
            "diagnostic.failure",
            "Failing process",
            "Writes an error to stderr and exits with code 3.",
            Scenario::Failure,
            60,
        ),
        make(
            "diagnostic.long-running",
            "Long-running process",
            "Emits a heartbeat every half second until cancelled (10 minute limit).",
            Scenario::LongRunning,
            600,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf, ExecutablePolicy) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("plenipo.exe");
        std::fs::write(&exe, b"x").unwrap();
        let policy = ExecutablePolicy::new([&exe]).unwrap();
        (dir, exe, policy)
    }

    #[test]
    fn diagnostic_profiles_register() {
        let (dir, exe, policy) = fixture();
        let (registry, rejected) =
            ProfileRegistry::new(diagnostic_profiles(&exe, dir.path()), &policy);
        assert!(rejected.is_empty(), "{rejected:?}");
        let ids: Vec<_> = registry.infos().into_iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            [
                "diagnostic.echo",
                "diagnostic.failure",
                "diagnostic.long-running"
            ]
        );
    }

    #[test]
    fn rejects_invalid_profiles() {
        let (dir, exe, policy) = fixture();
        let base = diagnostic_profiles(&exe, dir.path()).remove(0);
        let other = dir.path().join("other.exe");
        std::fs::write(&other, b"x").unwrap();

        let bad_id = LaunchProfile {
            id: "Bad ID".into(),
            ..base.clone()
        };
        let bad_exe = LaunchProfile {
            id: "bad.exe".into(),
            executable: other,
            ..base.clone()
        };
        let bad_env = LaunchProfile {
            id: "bad.env".into(),
            env: vec![("A=B".into(), "x".into())],
            ..base.clone()
        };
        let bad_dir = LaunchProfile {
            id: "bad.dir".into(),
            working_dir: "relative".into(),
            ..base.clone()
        };
        let bad_runtime = LaunchProfile {
            id: "bad.runtime".into(),
            max_runtime: Duration::ZERO,
            ..base.clone()
        };
        let (registry, rejected) = ProfileRegistry::new(
            [
                base.clone(),
                base.clone(),
                bad_id,
                bad_exe,
                bad_env,
                bad_dir,
                bad_runtime,
            ],
            &policy,
        );
        assert_eq!(registry.infos().len(), 1);
        assert_eq!(rejected.len(), 6, "{rejected:?}");
    }

    #[test]
    fn profile_ids() {
        for good in ["a", "diagnostic.echo", "x-1.y"] {
            assert!(validate_profile_id(good).is_ok());
        }
        for bad in ["", ".a", "A", "a b", "a/b", "../x", &"a".repeat(65)] {
            assert!(validate_profile_id(bad).is_err(), "{bad}");
        }
    }
}
