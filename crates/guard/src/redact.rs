//! Secret redaction. Hides the values of the owner's stored secrets and text that is
//! recognizably a secret (private keys, common API-key and token formats, passwords in URLs,
//! and `PASSWORD=`-style settings) before text reaches a worker, the Ledger, or the screen.
//! Recognition is best effort: a secret in an unrecognizable form can only be hidden once it
//! is stored in the Vault.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

/// Every hidden value is replaced by this, followed by what was hidden and `]`.
pub const MARKER: &str = "[hidden by Plenipo: ";
/// Stored secrets shorter than this are not searched for (they would hide ordinary words).
pub const MIN_KNOWN_LEN: usize = 6;

struct Pattern {
    regex: Regex,
    kind: &'static str,
    /// Capture group holding the secret itself (0: the whole match).
    group: usize,
}

fn patterns() -> &'static [Pattern] {
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let p = |re: &str, kind, group| Pattern {
            regex: Regex::new(re).expect("valid redaction pattern"),
            kind,
            group,
        };
        vec![
            p(
                r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|\z)",
                "private key",
                0,
            ),
            p(r"\bsk-(?:ant-|proj-)?[A-Za-z0-9_\-]{20,}", "API key", 0),
            p(r"\b[sr]k_(?:live|test)_[A-Za-z0-9]{16,}", "API key", 0),
            p(r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{30,}", "GitHub token", 0),
            p(r"\bgithub_pat_[A-Za-z0-9_]{40,}", "GitHub token", 0),
            p(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b", "cloud access key", 0),
            p(r"\bAIza[0-9A-Za-z_\-]{35}", "API key", 0),
            p(r"\bxox[abposr]-[A-Za-z0-9-]{10,}", "chat token", 0),
            p(
                r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
                "token",
                0,
            ),
            p(r"(?i)\bbearer\s+([A-Za-z0-9._~+/=-]{20,})", "token", 1),
            p(
                r"(?i)\b[a-z][a-z0-9+.-]*://[^/\s:@]+:([^@\s/]{3,})@",
                "password",
                1,
            ),
            p(
                r#"(?m)^\s*(?:export\s+|set\s+|\$env:)?[A-Z0-9_]*(?:PASSWORD|PASSWD|SECRET|TOKEN|API_?KEY|PRIVATE_?KEY|ACCESS_?KEY)[A-Z0-9_]*\s*[=:]\s*["']?([^\s"'#]{4,})"#,
                "secret setting",
                1,
            ),
            p(
                r#"(?i)"[a-z0-9_]*(?:password|passwd|secret|token|api_?key|private_?key|access_?key)[a-z0-9_]*"\s*:\s*"([^"]{4,})""#,
                "secret setting",
                1,
            ),
        ]
    })
}

/// Hides secrets in text. Cheap to clone; build a new one when the stored secrets change.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    /// (value, name), longest first so a secret containing another is hidden whole.
    known: Vec<(String, String)>,
}

impl Redactor {
    /// A redactor for these stored secrets (value, name) plus the recognizable formats.
    pub fn new(known: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut known: Vec<(String, String)> = known
            .into_iter()
            .filter(|(v, _)| v.chars().count() >= MIN_KNOWN_LEN)
            .collect();
        known.sort_by_key(|(v, _)| std::cmp::Reverse(v.len()));
        known.dedup_by(|a, b| a.0 == b.0);
        Self { known }
    }

    /// `text` with every secret hidden.
    pub fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let mut out: Cow<'a, str> = Cow::Borrowed(text);
        for (value, name) in &self.known {
            if out.contains(value.as_str()) {
                out = Cow::Owned(out.replace(value.as_str(), &format!("{MARKER}{name}]")));
            }
        }
        for p in patterns() {
            if !p.regex.is_match(&out) {
                continue;
            }
            let replaced = p
                .regex
                .replace_all(&out, |caps: &regex::Captures<'_>| {
                    let whole = caps.get(0).map_or("", |m| m.as_str());
                    let hidden = format!("{MARKER}{}]", p.kind);
                    match caps.get(p.group) {
                        Some(secret) if p.group != 0 => {
                            let start = secret.start() - caps.get(0).map_or(0, |m| m.start());
                            let end = start + secret.as_str().len();
                            format!("{}{hidden}{}", &whole[..start], &whole[end..])
                        }
                        _ => hidden,
                    }
                })
                .into_owned();
            out = Cow::Owned(replaced);
        }
        out
    }

    /// Hide secrets in every string inside `value`.
    pub fn redact_json(&self, value: &mut Value) {
        match value {
            Value::String(s) => {
                if let Cow::Owned(r) = self.redact(s) {
                    *s = r;
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|v| self.redact_json(v)),
            Value::Object(map) => map.values_mut().for_each(|v| self.redact_json(v)),
            _ => {}
        }
    }

    /// Stored secrets it hides.
    pub fn known_count(&self) -> usize {
        self.known.len()
    }
}

/// `text` holds a hidden-secret marker (so it must not be written back to a file).
pub fn has_marker(text: &str) -> bool {
    text.contains(MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_secrets_are_hidden() {
        let r = Redactor::new([
            ("s3cr3t-value-123".to_owned(), "Deploy key".to_owned()),
            ("abc".to_owned(), "too short".to_owned()),
        ]);
        assert_eq!(r.known_count(), 1);
        assert_eq!(
            r.redact("token=s3cr3t-value-123 and abc"),
            "token=[hidden by Plenipo: Deploy key] and abc"
        );
        assert!(matches!(r.redact("nothing here"), Cow::Borrowed(_)));
    }

    #[test]
    fn recognizable_secrets_are_hidden() {
        let r = Redactor::default();
        for (input, hidden) in [
            (
                "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaA\n-----END OPENSSH PRIVATE KEY-----",
                "private key",
            ),
            ("key sk-ant-api03-abcdefghijklmnopqrstuvwx", "API key"),
            ("OPENAI sk-proj-ABCDEFGHIJKLMNOPQRSTUVWXYZ012345", "API key"),
            ("stripe sk_live_ABCDEFGHIJKLMNOPQRST", "API key"),
            ("gh ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789", "GitHub token"),
            ("AKIAIOSFODNN7EXAMPLE", "cloud access key"),
            ("xoxb-1234567890-abcdefghij", "chat token"),
            (
                "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U",
                "token",
            ),
        ] {
            let out = r.redact(input);
            assert!(
                out.contains(&format!("{MARKER}{hidden}]")),
                "{input} → {out}"
            );
        }
    }

    #[test]
    fn settings_keep_their_names() {
        let r = Redactor::default();
        assert_eq!(
            r.redact("DB_PASSWORD=hunter22\nPORT=8080"),
            "DB_PASSWORD=[hidden by Plenipo: secret setting]\nPORT=8080"
        );
        assert_eq!(
            r.redact("export GITHUB_TOKEN=\"abcd1234\""),
            "export GITHUB_TOKEN=\"[hidden by Plenipo: secret setting]\""
        );
        assert_eq!(
            r.redact(r#"{"apiKey": "zzzz9999", "name": "x"}"#),
            r#"{"apiKey": "[hidden by Plenipo: secret setting]", "name": "x"}"#
        );
        assert_eq!(
            r.redact("Authorization: Bearer abcdefghijklmnopqrstuvwxyz"),
            "Authorization: Bearer [hidden by Plenipo: token]"
        );
        assert_eq!(
            r.redact("postgres://app:pa55word@db:5432/x"),
            "postgres://app:[hidden by Plenipo: password]@db:5432/x"
        );
    }

    #[test]
    fn ordinary_text_is_left_alone() {
        let r = Redactor::default();
        for text in [
            "commit 3f786850e387550fdab836ed7e6dc881de23001b",
            "TOKEN_COUNT is 5",
            "fn main() { let password_len = 8; }",
            "https://github.com/Seckcey/plenipo",
            "Bearer tokens expire",
            "    token: String,",
            "password = self.password",
        ] {
            assert_eq!(r.redact(text), text, "{text}");
        }
    }

    #[test]
    fn json_values_and_markers() {
        let r = Redactor::new([("topsecret99".to_owned(), "Key".to_owned())]);
        let mut v = serde_json::json!({ "a": ["x topsecret99"], "b": { "c": "fine" }, "n": 1 });
        r.redact_json(&mut v);
        assert_eq!(v["a"][0], "x [hidden by Plenipo: Key]");
        assert_eq!(v["b"]["c"], "fine");
        assert!(has_marker(v["a"][0].as_str().unwrap()));
        assert!(!has_marker("plain"));
    }
}
