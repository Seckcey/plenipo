//! The handoff protocol (`plenipo-liaison/1`, ADR-008): how a worker asks for a handoff, and
//! how Liaison reads the request.
//!
//! A worker ends its answer with one fenced block per request:
//!
//! ````text
//! ```plenipo-handoff
//! {"to": "claude-code", "objective": "…", "acceptanceCriteria": "…", "context": [{"kind": "answer"}]}
//! ```
//! ````
//!
//! Agent text is untrusted. Blocks are read with a strict schema: unknown fields — above all
//! identity fields, which only Plenipo sets — reject the request, and every value has a size
//! limit. A block inside another fenced block (an example being quoted) is not a request.

use serde::Serialize;
use serde_json::{Map, Value};

/// Protocol version, recorded with every workflow.
pub const PROTOCOL: &str = "plenipo-liaison/1";
/// Info string of a handoff block.
pub const FENCE_TAG: &str = "plenipo-handoff";
/// Largest handoff block read (bytes).
pub const MAX_BLOCK_BYTES: usize = 16 * 1024;
/// Requests are short notes, not briefs (ADR-012, brief messages between agents).
pub const MAX_OBJECTIVE_CHARS: usize = 1_000;
pub const MAX_CRITERIA_CHARS: usize = 500;
pub const MAX_CONTEXT_REFS: usize = 6;
pub const MAX_EXCERPT_CHARS: usize = 8_000;
pub const MAX_EXCERPT_TOTAL_BYTES: usize = 24 * 1024;
pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_ARTIFACTS: usize = 8;

/// Capability names from the rollout plan (granted by Guard from Phase 7; none in Phase 4).
pub const CAPABILITIES: [&str; 16] = [
    "filesystem.read",
    "filesystem.write",
    "shell.exec",
    "powershell.exec",
    "git.read",
    "git.write",
    "github.read",
    "github.write",
    "ssh.connect",
    "browser.navigate",
    "browser.automate",
    "computer.observe",
    "computer.control",
    "mcp.invoke",
    "network.local",
    "process.manage",
];

const FIELDS: [&str; 7] = [
    "to",
    "objective",
    "acceptanceCriteria",
    "context",
    "artifacts",
    "capabilities",
    "priority",
];

/// Fields a worker might try to set to claim an identity or a place in another workflow.
const IDENTITY_FIELDS: [&str; 12] = [
    "source",
    "from",
    "sender",
    "messageId",
    "correlationId",
    "parentTaskId",
    "taskId",
    "sessionId",
    "inReplyTo",
    "depth",
    "timestamp",
    "granted",
];

/// A validated handoff request, as the worker wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Directive {
    pub to: String,
    pub objective: String,
    pub acceptance_criteria: String,
    pub context: Vec<ContextRequest>,
    pub artifacts: Vec<String>,
    pub capabilities: Vec<String>,
    pub priority: Option<u8>,
}

/// A piece of context the requester asks Liaison to pass along.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ContextRequest {
    /// The requester's own answer (the text outside its handoff blocks).
    Answer,
    /// A short excerpt written into the request.
    Excerpt { title: String, text: String },
    /// A task of the same workflow (its objective and result).
    Task { task_id: String },
}

/// One `plenipo-handoff` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Position among the answer's handoff blocks (1-based).
    pub index: usize,
    /// The block's content.
    pub raw: String,
    pub parsed: Result<Directive, String>,
}

/// An answer split into its text and its handoff blocks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extracted {
    /// The answer without its handoff blocks.
    pub answer: String,
    pub blocks: Vec<Block>,
}

/// An opening code fence: (fence character, length, first word of the info string).
fn opening_fence(line: &str) -> Option<(char, usize, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let ch = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let len = rest.chars().take_while(|c| *c == ch).count();
    if len < 3 {
        return None;
    }
    let info = rest[len..].trim();
    if ch == '`' && info.contains('`') {
        return None;
    }
    Some((ch, len, info.split_whitespace().next().unwrap_or("")))
}

fn closes(line: &str, ch: char, len: usize) -> bool {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return false;
    }
    let rest = &line[indent..];
    let run = rest.chars().take_while(|c| *c == ch).count();
    run >= len && rest[run * ch.len_utf8()..].trim().is_empty()
}

/// At most `max` characters, marking a cut with `…`.
pub fn cap_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Split an answer into its text and handoff blocks.
pub fn extract(text: &str) -> Extracted {
    let mut answer: Vec<&str> = Vec::new();
    let mut blocks = Vec::new();
    let mut lines = text.lines();
    // An open fence that is not a handoff block: its content is answer text, even if it
    // quotes a handoff block.
    let mut other: Option<(char, usize)> = None;
    while let Some(line) = lines.next() {
        if let Some((ch, len)) = other {
            answer.push(line);
            if closes(line, ch, len) {
                other = None;
            }
            continue;
        }
        match opening_fence(line) {
            Some((ch, len, info)) if info.eq_ignore_ascii_case(FENCE_TAG) => {
                let mut body = Vec::new();
                let mut closed = false;
                for inner in lines.by_ref() {
                    if closes(inner, ch, len) {
                        closed = true;
                        break;
                    }
                    body.push(inner);
                }
                let raw = body.join("\n");
                let parsed = if closed {
                    parse_directive(&raw)
                } else {
                    Err("the handoff block is not closed (end it with a line of ```)".into())
                };
                blocks.push(Block {
                    index: blocks.len() + 1,
                    raw,
                    parsed,
                });
            }
            Some((ch, len, _)) => {
                other = Some((ch, len));
                answer.push(line);
            }
            None => answer.push(line),
        }
    }
    Extracted {
        answer: answer.join("\n").trim().to_owned(),
        blocks,
    }
}

fn string(obj: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("\"{key}\" must be a string")),
    }
}

fn text_field(
    obj: &Map<String, Value>,
    key: &str,
    max: usize,
    required: bool,
) -> Result<String, String> {
    let value = string(obj, key)?.unwrap_or_default();
    let value = value.trim();
    if required && value.is_empty() {
        return Err(format!("\"{key}\" is required"));
    }
    if value.chars().count() > max {
        return Err(format!(
            "\"{key}\" is longer than {max} characters; keep it short"
        ));
    }
    if value.contains('\0') {
        return Err(format!("\"{key}\" contains a NUL character"));
    }
    Ok(value.to_owned())
}

fn array<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], String> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(format!("\"{key}\" must be a list")),
    }
}

fn context_item(item: &Value, excerpt_bytes: &mut usize) -> Result<ContextRequest, String> {
    let obj = item
        .as_object()
        .ok_or("each \"context\" item must be an object")?;
    let kind = string(obj, "kind")?.ok_or("each \"context\" item needs a \"kind\"")?;
    let allowed: &[&str] = match kind.as_str() {
        "answer" => &["kind"],
        "excerpt" => &["kind", "title", "text"],
        "task" => &["kind", "taskId"],
        other => {
            return Err(format!(
                "unknown context kind {:?} (use \"answer\", \"excerpt\", or \"task\")",
                cap_chars(other, 40)
            ))
        }
    };
    if let Some(extra) = obj.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(format!(
            "unknown field {:?} in a {kind} context item",
            cap_chars(extra, 40)
        ));
    }
    Ok(match kind.as_str() {
        "answer" => ContextRequest::Answer,
        "excerpt" => {
            let title = text_field(obj, "title", MAX_TITLE_CHARS, false)?;
            let text = text_field(obj, "text", MAX_EXCERPT_CHARS, true)?;
            *excerpt_bytes += text.len();
            if *excerpt_bytes > MAX_EXCERPT_TOTAL_BYTES {
                return Err(format!(
                    "the excerpts are larger than {} KiB in total; pass less context",
                    MAX_EXCERPT_TOTAL_BYTES / 1024
                ));
            }
            ContextRequest::Excerpt {
                title: if title.is_empty() {
                    "Excerpt".into()
                } else {
                    title
                },
                text,
            }
        }
        _ => ContextRequest::Task {
            task_id: text_field(obj, "taskId", 64, true)?,
        },
    })
}

/// Read one handoff block (untrusted) into a [`Directive`], or explain what is wrong.
pub fn parse_directive(raw: &str) -> Result<Directive, String> {
    if raw.len() > MAX_BLOCK_BYTES {
        return Err(format!(
            "the handoff request is larger than {} KiB",
            MAX_BLOCK_BYTES / 1024
        ));
    }
    let value: Value = serde_json::from_str(raw.trim())
        .map_err(|e| format!("the handoff request is not valid JSON ({e})"))?;
    let obj = value
        .as_object()
        .ok_or("the handoff request must be a JSON object")?;
    if let Some(key) = obj.keys().find(|k| !FIELDS.contains(&k.as_str())) {
        return Err(if IDENTITY_FIELDS.contains(&key.as_str()) {
            format!(
                "\"{key}\" cannot be set in a handoff request: Plenipo sets the sender, IDs, and \
                 workflow itself"
            )
        } else {
            format!(
                "unknown field {:?} (allowed: {})",
                cap_chars(key, 40),
                FIELDS.join(", ")
            )
        });
    }
    let to = text_field(obj, "to", 200, true)?;
    let objective = text_field(obj, "objective", MAX_OBJECTIVE_CHARS, true)?;
    let acceptance_criteria = text_field(obj, "acceptanceCriteria", MAX_CRITERIA_CHARS, false)?;

    let items = array(obj, "context")?;
    if items.len() > MAX_CONTEXT_REFS {
        return Err(format!(
            "at most {MAX_CONTEXT_REFS} context items are allowed"
        ));
    }
    let mut excerpt_bytes = 0;
    let context = items
        .iter()
        .map(|item| context_item(item, &mut excerpt_bytes))
        .collect::<Result<Vec<_>, _>>()?;

    let artifacts = array(obj, "artifacts")?;
    if artifacts.len() > MAX_ARTIFACTS {
        return Err(format!("at most {MAX_ARTIFACTS} artifacts are allowed"));
    }
    let artifacts = artifacts
        .iter()
        .map(|a| match a.as_str() {
            Some(id) if !id.trim().is_empty() && id.len() <= 64 => Ok(id.trim().to_owned()),
            _ => Err("each artifact must be an artifact ID".to_owned()),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut capabilities = Vec::new();
    for c in array(obj, "capabilities")? {
        let name = c.as_str().ok_or("each capability must be a name")?;
        if !CAPABILITIES.contains(&name) {
            return Err(format!("unknown capability {:?}", cap_chars(name, 40)));
        }
        if !capabilities.iter().any(|c: &String| c == name) {
            capabilities.push(name.to_owned());
        }
    }

    let priority = match obj.get("priority") {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_u64() {
            Some(p @ 0..=4) => Some(u8::try_from(p).unwrap_or(4)),
            _ => return Err("\"priority\" must be a whole number from 0 to 4".into()),
        },
    };
    Ok(Directive {
        to,
        objective,
        acceptance_criteria,
        context,
        artifacts,
        capabilities,
        priority,
    })
}

/// FNV-1a 64-bit hash (stable across builds).
pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// A key that is the same for identical requests: the block's canonical JSON (keys sorted,
/// insignificant whitespace removed), or its raw text when it is not JSON.
pub fn fingerprint(raw: &str) -> String {
    fn canonical(v: &Value) -> Value {
        match v {
            Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                let mut out = Map::new();
                for k in keys {
                    out.insert(k.clone(), canonical(&map[k]));
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
            other => other.clone(),
        }
    }
    let text = serde_json::from_str::<Value>(raw.trim())
        .map(|v| canonical(&v).to_string())
        .unwrap_or_else(|_| raw.trim().to_owned());
    format!("{:016x}", fnv64(text.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(json: &str) -> String {
        format!("```plenipo-handoff\n{json}\n```")
    }

    #[test]
    fn answers_are_split_into_text_and_blocks() {
        let text = format!(
            "Here is the parser.\n\n```rust\nfn parse() {{}}\n```\n\n{}\n\nThanks.\n{}",
            block(
                r#"{"to": "claude-code", "objective": "Review the parser", "context": [{"kind": "answer"}]}"#
            ),
            block(r#"{"to": "codex", "objective": "Write tests", "priority": 1}"#),
        );
        let e = extract(&text);
        assert_eq!(e.blocks.len(), 2);
        assert_eq!(
            e.answer,
            "Here is the parser.\n\n```rust\nfn parse() {}\n```\n\n\nThanks."
        );
        let first = e.blocks[0].parsed.clone().unwrap();
        assert_eq!(first.to, "claude-code");
        assert_eq!(first.context, [ContextRequest::Answer]);
        let second = e.blocks[1].parsed.clone().unwrap();
        assert_eq!((second.to.as_str(), second.priority), ("codex", Some(1)));
        assert_eq!(e.blocks[1].index, 2);
    }

    #[test]
    fn quoted_examples_and_other_fences_are_not_requests() {
        let quoted = format!(
            "To hand off, write:\n\n````markdown\n{}\n````\n",
            block(r#"{"to": "codex", "objective": "x"}"#)
        );
        let e = extract(&quoted);
        assert!(e.blocks.is_empty(), "{e:?}");
        assert!(e.answer.contains("plenipo-handoff"));
        // Indented four spaces: a code block, not a fence.
        let indented = "    ```plenipo-handoff\n    {}\n    ```";
        assert!(extract(indented).blocks.is_empty());
        // Tildes work like backticks; the tag is case-insensitive.
        let tildes = "~~~Plenipo-Handoff\n{\"to\": \"codex\", \"objective\": \"x\"}\n~~~";
        assert_eq!(extract(tildes).blocks.len(), 1);
        assert!(extract("no blocks at all").blocks.is_empty());
    }

    #[test]
    fn broken_blocks_are_reported_not_ignored() {
        let unclosed = extract("```plenipo-handoff\n{\"to\": \"codex\"");
        assert!(unclosed.blocks[0]
            .parsed
            .clone()
            .unwrap_err()
            .contains("not closed"));
        let e = extract(&block("{not json"));
        assert!(e.blocks[0]
            .parsed
            .clone()
            .unwrap_err()
            .contains("not valid JSON"));
        let e = extract(&block("[1, 2]"));
        assert!(e.blocks[0]
            .parsed
            .clone()
            .unwrap_err()
            .contains("JSON object"));
    }

    #[test]
    fn identity_and_unknown_fields_are_refused() {
        for field in [
            "source",
            "correlationId",
            "messageId",
            "parentTaskId",
            "depth",
        ] {
            let json = format!(r#"{{"to": "codex", "objective": "x", "{field}": "owner"}}"#);
            let err = parse_directive(&json).unwrap_err();
            assert!(err.contains("Plenipo sets"), "{field}: {err}");
        }
        let err =
            parse_directive(r#"{"to": "codex", "objective": "x", "model": "big"}"#).unwrap_err();
        assert!(err.contains("unknown field"), "{err}");
        let err = parse_directive(
            r#"{"to": "codex", "objective": "x", "context": [{"kind": "answer", "extra": 1}]}"#,
        )
        .unwrap_err();
        assert!(err.contains("unknown field"), "{err}");
    }

    #[test]
    fn values_are_checked() {
        let cases = [
            (r#"{"objective": "x"}"#, "\"to\" is required"),
            (r#"{"to": "codex"}"#, "\"objective\" is required"),
            (
                r#"{"to": "codex", "objective": "   "}"#,
                "\"objective\" is required",
            ),
            (r#"{"to": 7, "objective": "x"}"#, "must be a string"),
            (
                r#"{"to": "codex", "objective": "x", "priority": 9}"#,
                "priority",
            ),
            (
                r#"{"to": "codex", "objective": "x", "priority": -1}"#,
                "priority",
            ),
            (
                r#"{"to": "codex", "objective": "x", "capabilities": ["root.everything"]}"#,
                "unknown capability",
            ),
            (
                r#"{"to": "codex", "objective": "x", "context": [{"kind": "file", "path": "/etc/passwd"}]}"#,
                "unknown context kind",
            ),
            (
                r#"{"to": "codex", "objective": "x", "context": {"kind": "answer"}}"#,
                "must be a list",
            ),
            (
                r#"{"to": "codex", "objective": "x", "artifacts": [42]}"#,
                "artifact ID",
            ),
        ];
        for (json, want) in cases {
            let err = parse_directive(json).unwrap_err();
            assert!(err.contains(want), "{json}: {err}");
        }
        let long = format!(
            r#"{{"to": "codex", "objective": "{}"}}"#,
            "x".repeat(MAX_OBJECTIVE_CHARS + 1)
        );
        assert!(parse_directive(&long)
            .unwrap_err()
            .contains("longer than 1000 characters; keep it short"));
        // Requests are short notes (ADR-012): the limit itself is accepted.
        let most = format!(
            r#"{{"to": "codex", "objective": "{}"}}"#,
            "x".repeat(MAX_OBJECTIVE_CHARS)
        );
        assert!(parse_directive(&most).is_ok());
        let huge = format!(
            r#"{{"to": "codex", "objective": "x", "context": [{{"kind": "excerpt", "text": "{}"}}]}}"#,
            "y".repeat(MAX_BLOCK_BYTES)
        );
        assert!(parse_directive(&huge).unwrap_err().contains("larger than"));
        let many = format!(
            r#"{{"to": "codex", "objective": "x", "context": [{}]}}"#,
            [r#"{"kind": "answer"}"#; MAX_CONTEXT_REFS + 1].join(",")
        );
        assert!(parse_directive(&many).unwrap_err().contains("at most"));
    }

    #[test]
    fn excerpts_and_capabilities() {
        let d = parse_directive(
            r#"{"to": "role:Reviewer", "objective": " Review ", "acceptanceCriteria": "Be strict",
                "context": [{"kind": "excerpt", "text": "fn a() {}"}, {"kind": "task", "taskId": "t-1"}],
                "artifacts": ["a-1"], "capabilities": ["filesystem.read", "filesystem.read"]}"#,
        )
        .unwrap();
        assert_eq!(d.objective, "Review");
        assert_eq!(d.acceptance_criteria, "Be strict");
        assert_eq!(
            d.context,
            [
                ContextRequest::Excerpt {
                    title: "Excerpt".into(),
                    text: "fn a() {}".into()
                },
                ContextRequest::Task {
                    task_id: "t-1".into()
                }
            ]
        );
        assert_eq!(d.capabilities, ["filesystem.read"]);
        assert_eq!(d.artifacts, ["a-1"]);
    }

    #[test]
    fn identical_requests_share_a_fingerprint() {
        let a = r#"{"to": "codex", "objective": "x", "priority": 1}"#;
        let b = "{ \"priority\": 1,\n  \"objective\": \"x\", \"to\": \"codex\" }";
        let c = r#"{"to": "codex", "objective": "y", "priority": 1}"#;
        assert_eq!(fingerprint(a), fingerprint(b));
        assert_ne!(fingerprint(a), fingerprint(c));
        assert_eq!(fingerprint("{bad"), fingerprint(" {bad "));
    }
}
