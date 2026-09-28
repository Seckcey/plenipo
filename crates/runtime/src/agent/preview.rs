//! A change an AI tool is still writing (Phase 18, ADR-055): some AI tools stream a tool call's
//! arguments while the model writes them — Claude Code's `input_json_delta`, ACP's
//! `tool_call_update` notes marked `in_progress` (Kimi). The parsers read the file's path and the
//! text so far from the unfinished JSON and hand them, a few times a second, to the tool
//! provider, which checks them against the step's permissions before anything is shown. Nothing
//! here is stored.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How often a growing preview is handed on.
pub const PREVIEW_EVERY: Duration = Duration::from_millis(100);
/// The most of one tool call's arguments read for a preview (larger ones show when saved).
pub const MAX_PREVIEW_JSON: usize = 512 * 1024;

/// Which of Plenipo's file changes a tool call is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteTool {
    /// Create or replace a whole file (`write_file`, ACP's write).
    Write,
    /// Replace part of a file (`edit_file`).
    Edit,
}

/// A change being written: the AI tool's path (as it gave it) and the text so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritePreview {
    /// The tool call it belongs to, unique within the conversation's step.
    pub call: String,
    pub tool: WriteTool,
    pub path: Option<String>,
    /// The file's new content so far (a write), or the replacing text so far (an edit).
    pub text: String,
    /// The AI tool finished writing the call (it is about to be sent).
    pub done: bool,
}

/// The string fields of an unfinished JSON object: each key's value so far, and whether that
/// value is complete. Only top-level string values are read; anything else is skipped.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PartialFields {
    pub fields: Vec<(String, String, bool)>,
}

impl PartialFields {
    /// The first of `keys` present, with whether its value is complete.
    pub fn get(&self, keys: &[&str]) -> Option<(&str, bool)> {
        keys.iter().find_map(|k| {
            self.fields
                .iter()
                .find(|(key, _, _)| key == k)
                .map(|(_, v, done)| (v.as_str(), *done))
        })
    }
}

/// Read a JSON string starting after its opening quote; returns the text and where it ended
/// (after the closing quote), or `None` for the end when the string is unfinished. An escape cut
/// off at the end is left out.
fn string_at(chars: &[char], mut i: usize) -> (String, Option<usize>) {
    let mut out = String::new();
    while i < chars.len() {
        match chars[i] {
            '"' => return (out, Some(i + 1)),
            '\\' => {
                let Some(&e) = chars.get(i + 1) else {
                    return (out, None);
                };
                match e {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'u' => {
                        let hex: String = chars.iter().skip(i + 2).take(4).collect();
                        if hex.len() < 4 {
                            return (out, None);
                        }
                        let Ok(code) = u32::from_str_radix(&hex, 16) else {
                            return (out, None);
                        };
                        // A surrogate pair: the second half follows.
                        if (0xD800..0xDC00).contains(&code) {
                            let low: String = chars.iter().skip(i + 8).take(4).collect();
                            if chars.get(i + 6) != Some(&'\\')
                                || chars.get(i + 7) != Some(&'u')
                                || low.len() < 4
                            {
                                return (out, None);
                            }
                            let low = u32::from_str_radix(&low, 16).unwrap_or(0);
                            let joined = 0x10000
                                + ((code - 0xD800) << 10)
                                + (low.wrapping_sub(0xDC00) & 0x3FF);
                            out.push(char::from_u32(joined).unwrap_or('\u{fffd}'));
                            i += 12;
                            continue;
                        }
                        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        i += 6;
                        continue;
                    }
                    other => out.push(other),
                }
                i += 2;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    (out, None)
}

/// Skip a non-string value (number, `true`, an object or array) at `i`; returns where it ends.
fn skip_value(chars: &[char], mut i: usize) -> Option<usize> {
    let mut depth = 0i32;
    while i < chars.len() {
        match chars[i] {
            '"' => {
                let (_, end) = string_at(chars, i + 1);
                i = end?;
                continue;
            }
            '{' | '[' => depth += 1,
            '}' | ']' if depth > 0 => depth -= 1,
            ',' | '}' if depth == 0 => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// The top-level string fields of `json`, which may be cut off anywhere.
pub fn partial_fields(json: &str) -> PartialFields {
    let chars: Vec<char> = json.chars().collect();
    let mut out = PartialFields::default();
    let mut i = match chars.iter().position(|c| !c.is_whitespace()) {
        Some(p) if chars[p] == '{' => p + 1,
        _ => return out,
    };
    loop {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == ',') {
            i += 1;
        }
        if i >= chars.len() || chars[i] != '"' {
            return out;
        }
        let (key, end) = string_at(&chars, i + 1);
        let Some(mut j) = end else {
            return out;
        };
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        if j >= chars.len() || chars[j] != ':' {
            return out;
        }
        j += 1;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        if j >= chars.len() {
            return out;
        }
        if chars[j] == '"' {
            let (value, end) = string_at(&chars, j + 1);
            out.fields.push((key, value, end.is_some()));
            match end {
                Some(e) => i = e,
                None => return out,
            }
        } else {
            match skip_value(&chars, j) {
                Some(e) => i = e,
                None => return out,
            }
        }
    }
}

/// The preview a tool call's arguments so far give, if they name anything to show.
pub fn preview_of(call: &str, tool: WriteTool, json: &str, done: bool) -> Option<WritePreview> {
    let json = if json.len() > MAX_PREVIEW_JSON {
        let mut cut = MAX_PREVIEW_JSON;
        while !json.is_char_boundary(cut) {
            cut -= 1;
        }
        &json[..cut]
    } else {
        json
    };
    let fields = partial_fields(json);
    // A file named twice is not shown at all: the AI tool uses the last, and the text must
    // never show under another file's name.
    let named = fields
        .fields
        .iter()
        .filter(|(k, _, _)| matches!(k.as_str(), "path" | "file_path" | "filePath"))
        .count();
    let path = fields
        .get(&["path", "file_path", "filePath"])
        .filter(|(_, complete)| *complete && named == 1)
        .map(|(p, _)| p.to_owned());
    if named > 1 {
        return None;
    }
    let text = match tool {
        WriteTool::Write => fields.get(&["content", "text"]),
        WriteTool::Edit => fields.get(&["newText", "new_text", "new_string", "newString"]),
    }
    .map(|(t, _)| t.to_owned())
    .unwrap_or_default();
    (path.is_some() || !text.is_empty()).then(|| WritePreview {
        call: call.to_owned(),
        tool,
        path,
        text,
        done,
    })
}

/// Which previews are due: a growing call is handed on at most every [`PREVIEW_EVERY`], and
/// always when its path is first known or it is done.
#[derive(Debug, Default)]
pub struct PreviewPace {
    last: HashMap<String, (Instant, bool)>,
}

impl PreviewPace {
    /// Whether a preview of `call` could be due now (its path not known yet, or long enough
    /// since the last): before reading its arguments again.
    pub fn ready(&self, call: &str) -> bool {
        self.last
            .get(call)
            .is_none_or(|(at, had_path)| !had_path || at.elapsed() >= PREVIEW_EVERY)
    }

    pub fn due(&mut self, preview: &WritePreview) -> bool {
        let now = Instant::now();
        let has_path = preview.path.is_some();
        let due = match self.last.get(&preview.call) {
            None => true,
            Some((at, had_path)) => {
                preview.done || (has_path && !had_path) || now.duration_since(*at) >= PREVIEW_EVERY
            }
        };
        if preview.done {
            self.last.remove(&preview.call);
        } else if due {
            self.last.insert(preview.call.clone(), (now, has_path));
        }
        due
    }
}

/// Whether a tool name is Plenipo's own `write_file` or `edit_file` (as an AI tool names it:
/// `mcp__plenipo__write_file`, or plain).
pub fn plenipo_write_tool(name: &str) -> Option<WriteTool> {
    let name = name.strip_prefix("mcp__plenipo__").unwrap_or(name);
    match name {
        "write_file" => Some(WriteTool::Write),
        "edit_file" => Some(WriteTool::Edit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_so_far_is_read_from_unfinished_json() {
        let full =
            r#"{"path": "src/app.rs", "content": "fn main() {\n    println!(\"hi é\");\n}\n"}"#;
        let mut seen = Vec::new();
        for end in 0..=full.len() {
            if !full.is_char_boundary(end) {
                continue;
            }
            if let Some(p) = preview_of("c1", WriteTool::Write, &full[..end], false) {
                seen.push(p);
            }
        }
        // The path shows only once it is complete; the text grows, and never shows a cut escape.
        let first_with_path = seen.iter().position(|p| p.path.is_some()).unwrap();
        assert!(seen[..first_with_path].iter().all(|p| p.text.is_empty()));
        assert!(seen.iter().all(|p| !p.text.ends_with('\\')));
        let last = seen.last().unwrap();
        assert_eq!(last.path.as_deref(), Some("src/app.rs"));
        assert_eq!(last.text, "fn main() {\n    println!(\"hi é\");\n}\n");
        for pair in seen.windows(2) {
            assert!(pair[1].text.starts_with(&pair[0].text) || pair[0].text.is_empty());
        }
    }

    #[test]
    fn an_edit_shows_its_new_text_and_other_values_are_skipped() {
        let json = r#"{"replaceAll": false, "path": "a.txt", "oldText": "x", "newText": "hello wo"#;
        let p = preview_of("c2", WriteTool::Edit, json, false).unwrap();
        assert_eq!(p.path.as_deref(), Some("a.txt"));
        assert_eq!(p.text, "hello wo");
        let fields = partial_fields(r#"{"n": {"a": [1, "}"]}, "path": "b"}"#);
        assert_eq!(fields.get(&["path"]), Some(("b", true)));
        // A surrogate pair, and one cut in half.
        let emoji = partial_fields(r#"{"text": "ok 😀"}"#);
        assert_eq!(emoji.get(&["text"]), Some(("ok 😀", true)));
        let half = partial_fields(r#"{"text": "ok \ud83d"#);
        assert_eq!(half.get(&["text"]), Some(("ok ", false)));
        assert!(preview_of("c3", WriteTool::Write, "not json", false).is_none());
        // A file named twice shows nothing (the AI tool would write to the last one).
        let twice = r#"{"path": "src/ok.rs", "content": "text", "path": ".env"}"#;
        assert!(preview_of("c4", WriteTool::Write, twice, false).is_none());
    }

    #[test]
    fn previews_are_paced_but_the_path_and_the_end_always_go() {
        let mut pace = PreviewPace::default();
        let p = |path: Option<&str>, text: &str, done: bool| WritePreview {
            call: "c".into(),
            tool: WriteTool::Write,
            path: path.map(str::to_owned),
            text: text.into(),
            done,
        };
        assert!(pace.ready("c"), "nothing yet");
        assert!(pace.due(&p(None, "a", false)), "the first");
        assert!(pace.ready("c"), "its file is not known yet");
        assert!(!pace.due(&p(None, "ab", false)), "too soon");
        assert!(pace.due(&p(Some("x"), "ab", false)), "the path is new");
        assert!(!pace.ready("c"), "not read again this soon");
        assert!(!pace.due(&p(Some("x"), "abc", false)));
        assert!(pace.due(&p(Some("x"), "abcd", true)), "done");
        assert_eq!(
            plenipo_write_tool("mcp__plenipo__edit_file"),
            Some(WriteTool::Edit)
        );
        assert_eq!(plenipo_write_tool("write_file"), Some(WriteTool::Write));
        assert_eq!(plenipo_write_tool("mcp__other__write_file"), None);
    }
}
