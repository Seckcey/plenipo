//! Serializable data-transfer objects shared by the Rust backend and the
//! React frontend. Field names are camelCase on the wire.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Compile-time build profile of the running binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    /// Profile of the binary this code was compiled into.
    pub const fn current() -> Self {
        if cfg!(debug_assertions) {
            Self::Debug
        } else {
            Self::Release
        }
    }
}

/// Basic identity of the running application, shown in the shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub build_profile: BuildProfile,
    /// Operating system family, e.g. `windows`, `linux`, `macos`.
    pub os: String,
    /// CPU architecture, e.g. `x86_64`, `aarch64`.
    pub arch: String,
}

impl AppInfo {
    /// Build [`AppInfo`] for the current process using the given version string.
    pub fn current(version: impl Into<String>) -> Self {
        Self {
            name: crate::PRODUCT_NAME.to_owned(),
            version: version.into(),
            build_profile: BuildProfile::current(),
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
        }
    }
}

/// Category of a command failure. Stable, machine-readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CommandErrorKind {
    InvalidInput,
    Internal,
}

/// Error returned from every Tauri command. Never contains secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{kind:?}: {message}")]
#[ts(export)]
pub struct CommandError {
    pub kind: CommandErrorKind,
    pub message: String,
}

impl CommandError {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            kind: CommandErrorKind::InvalidInput,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: CommandErrorKind::Internal,
            message: message.into(),
        }
    }
}

/// Diagnostic actions on a synthetic task (Diagnostics → Ledger). Real tasks are driven
/// by coordinators and workers in later phases, never directly by these actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SyntheticTaskAction {
    Start,
    Block,
    AwaitApproval,
    Resume,
    Complete,
    Fail,
    Cancel,
    AddChild,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn app_info_serializes_camel_case() {
        let info = AppInfo {
            name: "Plenipo".into(),
            version: "0.1.0".into(),
            build_profile: BuildProfile::Release,
            os: "windows".into(),
            arch: "x86_64".into(),
        };
        let value = serde_json::to_value(&info).unwrap();
        assert_eq!(
            value,
            json!({
                "name": "Plenipo",
                "version": "0.1.0",
                "buildProfile": "release",
                "os": "windows",
                "arch": "x86_64"
            })
        );
    }

    #[test]
    fn app_info_round_trips() {
        let info = AppInfo::current("1.2.3");
        let text = serde_json::to_string(&info).unwrap();
        let back: AppInfo = serde_json::from_str(&text).unwrap();
        assert_eq!(info, back);
        assert_eq!(back.name, crate::PRODUCT_NAME);
        assert_eq!(back.os, std::env::consts::OS);
    }

    #[test]
    fn build_profile_matches_compilation() {
        let expected = if cfg!(debug_assertions) {
            BuildProfile::Debug
        } else {
            BuildProfile::Release
        };
        assert_eq!(BuildProfile::current(), expected);
    }

    #[test]
    fn command_error_serializes_kind_and_message() {
        let err = CommandError::invalid_input("bad value");
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            json!({ "kind": "invalidInput", "message": "bad value" })
        );
        assert_eq!(
            serde_json::to_value(CommandError::internal("x")).unwrap()["kind"],
            "internal"
        );
    }

    #[test]
    fn synthetic_actions_are_camel_case() {
        assert_eq!(
            serde_json::to_value(SyntheticTaskAction::AwaitApproval).unwrap(),
            json!("awaitApproval")
        );
        assert!(serde_json::from_value::<SyntheticTaskAction>(json!("deleteEverything")).is_err());
    }

    #[test]
    fn unknown_fields_in_error_kind_are_rejected() {
        let bad = serde_json::from_value::<CommandErrorKind>(json!("shellExec"));
        assert!(bad.is_err());
    }
}
