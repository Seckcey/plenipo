//! Addresses of message senders and receivers.
//!
//! | Address          | Meaning                                                              |
//! | ---------------- | -------------------------------------------------------------------- |
//! | `owner`          | the person using Plenipo                                             |
//! | `liaison`        | Plenipo Liaison itself (for example, when it refuses a request)      |
//! | `session:<id>`   | one worker session; senders are always sessions Plenipo ran          |
//! | `runtime:<id>`   | a destination: a new, ephemeral worker on that runtime               |
//! | `role:<name>`    | a destination role, resolved by the Workforce engine (Phases 5–6)    |
//!
//! Agents may write a bare runtime ID (`claude-code`) for `runtime:claude-code`.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    Owner,
    Liaison,
    Session(String),
    Runtime(String),
    Role(String),
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner => f.write_str("owner"),
            Self::Liaison => f.write_str("liaison"),
            Self::Session(id) => write!(f, "session:{id}"),
            Self::Runtime(id) => write!(f, "runtime:{id}"),
            Self::Role(name) => write!(f, "role:{name}"),
        }
    }
}

/// `[a-z0-9][a-z0-9-]{0,31}`: a runtime ID.
fn runtime_id_ok(id: &str) -> bool {
    let mut chars = id.chars();
    (1..=32).contains(&id.len())
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Printable, single-line, at most 64 characters: a role name.
fn role_ok(name: &str) -> bool {
    (1..=64).contains(&name.chars().count())
        && name.chars().all(|c| !c.is_control())
        && !name.trim().is_empty()
}

impl Address {
    /// Parse an address as written by an agent in a handoff request.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        let describe = |raw: &str| {
            let shown: String = raw.chars().take(64).collect();
            format!("{shown:?} is not a destination")
        };
        match raw.split_once(':') {
            None if raw == "owner" => Ok(Self::Owner),
            None if raw == "liaison" => Ok(Self::Liaison),
            None if runtime_id_ok(raw) => Ok(Self::Runtime(raw.to_owned())),
            Some(("runtime", id)) if runtime_id_ok(id.trim()) => {
                Ok(Self::Runtime(id.trim().to_owned()))
            }
            Some(("role", name)) if role_ok(name) => Ok(Self::Role(name.trim().to_owned())),
            Some(("session", id)) if !id.trim().is_empty() && id.len() <= 64 => {
                Ok(Self::Session(id.trim().to_owned()))
            }
            _ => Err(describe(raw)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinations_parse_and_print() {
        for (raw, want) in [
            ("claude-code", Address::Runtime("claude-code".into())),
            (" codex ", Address::Runtime("codex".into())),
            ("runtime:codex", Address::Runtime("codex".into())),
            ("role:Code Reviewer", Address::Role("Code Reviewer".into())),
            ("owner", Address::Owner),
            ("session:abc", Address::Session("abc".into())),
        ] {
            let parsed = Address::parse(raw).unwrap();
            assert_eq!(parsed, want, "{raw}");
            assert_eq!(Address::parse(&parsed.to_string()).unwrap(), want);
        }
        for bad in [
            "",
            "Claude Code",
            "../codex",
            "runtime:",
            "runtime:Codex",
            "role:",
            "role:\u{7}bell",
            "mcp:server",
            "codex --yolo",
            &"a".repeat(33),
        ] {
            assert!(Address::parse(bad).is_err(), "{bad:?}");
        }
    }
}
