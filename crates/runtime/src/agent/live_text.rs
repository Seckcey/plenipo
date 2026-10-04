//! Live text, redacted as whole words (ADR-216). The runtime hides secrets in an agent's
//! streamed words and thinking before they reach the screens, but a redaction pattern needs a
//! whole token (an `sk-…` key, a `-----BEGIN … PRIVATE KEY-----` block), and a stream hands the
//! text over in small pieces: a key written across two pieces passed each check on its own and
//! showed on screen, joined by the screen. So the word being written is held back. A piece goes
//! on only up to its newest whitespace, and the redactor sees the held word with the text already
//! shown in front of it (so `bearer ` and `PASSWORD=` prefixes still match), hiding only from
//! where the new text starts. A private key is held from its `-----BEGIN` to its `-----END`. The
//! beginning of a stored secret that has whitespace in it is held until the rest comes. What is
//! held goes on when an event of another kind comes, and when the turn ends.

use crate::agent::dto::{AgentEvent, StatusPhase};
use crate::agent::tools::TextFilter;

/// Whether `event` ends the blocks of live text, so what is held goes on before it (and the
/// screen keeps the order). Waiting for the AI company (a retry can land mid-word), token counts
/// and plans do not; everything else does: a message, a tool call or its result, a notice, the
/// "Thinking" and "Starting" signs (#200 hides the thinking sign when reasoning follows it, so
/// held words must not come between them), a session bound, memory shortened.
pub fn ends_blocks(event: &AgentEvent) -> bool {
    !matches!(
        event,
        AgentEvent::TextDelta { .. }
            | AgentEvent::Reasoning { .. }
            | AgentEvent::Status {
                phase: StatusPhase::Waiting,
                ..
            }
            | AgentEvent::Usage { .. }
            | AgentEvent::Plan { .. }
    )
}

/// How much already-shown text the redactor sees in front of the held word: the current line,
/// up to this many characters, enough for a setting's name (`export MY_DATABASE_PASSWORD=`).
const CONTEXT_CHARS: usize = 200;
/// Held text longer than this goes on anyway (a private key as its marker; other text redacted
/// as far as it can be), so what is held stays bounded.
const MAX_HELD_BYTES: usize = 64 * 1024;
const KEY_BEGIN: &str = "-----BEGIN";
const KEY_END: &str = "-----END";
const PRIVATE_KEY: &str = "private key";

/// The two kinds of live text, held back separately: an agent's words and its thinking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveKind {
    Words,
    Thinking,
}

impl LiveKind {
    /// The live event that carries `text` of this kind.
    pub fn event(self, text: String) -> AgentEvent {
        match self {
            Self::Words => AgentEvent::TextDelta { text },
            Self::Thinking => AgentEvent::Reasoning { text },
        }
    }
}

/// The live text of one turn, held back by kind.
#[derive(Debug, Default)]
pub struct LiveText {
    words: Holdback,
    thinking: Holdback,
}

impl LiveText {
    /// Take one streamed piece; what may go on now, redacted (often nothing: the word is still
    /// being written). With no filter nothing is held, and the piece goes on as it is.
    pub fn push(&mut self, kind: LiveKind, piece: &str, filter: Option<&TextFilter>) -> String {
        self.of(kind).push(piece, filter)
    }

    /// Everything of `kind` still held, redacted; empty when nothing is.
    pub fn flush(&mut self, kind: LiveKind, filter: Option<&TextFilter>) -> String {
        self.of(kind).flush(filter)
    }

    fn of(&mut self, kind: LiveKind) -> &mut Holdback {
        match kind {
            LiveKind::Words => &mut self.words,
            LiveKind::Thinking => &mut self.thinking,
        }
    }
}

/// One kind's text: what is held, and what the redactor sees in front of it.
#[derive(Debug, Default)]
struct Holdback {
    /// Received and not yet sent on, as the agent wrote it.
    held: String,
    /// The end of what was sent on, as the agent wrote it (the current line, up to
    /// [`CONTEXT_CHARS`]): the redactor's context, never shown again.
    shown: String,
    /// Inside a private key: from its `-----BEGIN` until its `-----END` (or the block's end),
    /// nothing goes on but the key's marker.
    in_key: bool,
}

impl Holdback {
    fn push(&mut self, piece: &str, filter: Option<&TextFilter>) -> String {
        let Some(f) = filter else {
            return piece.to_owned();
        };
        self.held.push_str(piece);
        // A key's BEGIN, wherever it is in what is held (a word may carry it: `key="-----BEGIN`),
        // before anything of that word goes on.
        if !self.in_key && self.held.contains(KEY_BEGIN) {
            self.in_key = true;
        }
        if self.in_key {
            return self.push_in_key(f);
        }
        let mut ready = after_last_whitespace(&self.held);
        if self.held.len() > MAX_HELD_BYTES {
            ready = self.held.len();
        } else if let Some(at) = f.stored_secret_start(&format!("{}{}", self.shown, self.held)) {
            // A stored secret with whitespace in it may have begun there: what comes before
            // it goes on, the rest waits until the secret is whole (or turns out not to be one).
            ready = ready.min(at.saturating_sub(self.shown.len()));
        }
        if ready == 0 {
            return String::new();
        }
        let out: String = self.held.drain(..ready).collect();
        self.send(&out, f)
    }

    /// Inside a private key: nothing goes on until its END (then the text before the key, the
    /// key's marker, and what follows up to its newest whitespace), or the cap is passed (then
    /// the text before the key and the marker, and the key stays open: each further cap sends
    /// another marker, as the stored copy hides the key to the end of its message).
    fn push_in_key(&mut self, f: &TextFilter) -> String {
        let begin = self.held.find(KEY_BEGIN);
        let before: String = match begin {
            Some(at) => self.held.drain(..at).collect(),
            None => String::new(),
        };
        let end = key_end(&self.held);
        if end.is_none() && self.held.len() <= MAX_HELD_BYTES {
            // Still inside the key: what came before it may go on when it is whole.
            let mut out = String::new();
            if !before.is_empty() {
                let ready = after_last_whitespace(&before);
                let (now, later) = before.split_at(ready);
                out = self.send(now, f);
                let rest = std::mem::take(&mut self.held);
                self.held = format!("{later}{rest}");
            }
            return out;
        }
        let mut out = self.send(&before, f);
        let marker = f.hidden(PRIVATE_KEY);
        out.push_str(&marker);
        self.shown.push_str(&marker);
        match end {
            Some(end) => {
                self.held.drain(..end);
                self.in_key = false;
                let ready = after_last_whitespace(&self.held);
                if ready > 0 {
                    let after: String = self.held.drain(..ready).collect();
                    out.push_str(&self.send(&after, f));
                }
            }
            None => self.held.clear(),
        }
        self.trim_shown();
        out
    }

    fn flush(&mut self, filter: Option<&TextFilter>) -> String {
        if self.held.is_empty() {
            return String::new();
        }
        let Some(f) = filter else {
            return std::mem::take(&mut self.held);
        };
        if self.in_key {
            // The block ends inside the key: its marker, and the key is over for the live view.
            let before: String = match self.held.find(KEY_BEGIN) {
                Some(at) => self.held.drain(..at).collect(),
                None => String::new(),
            };
            self.held.clear();
            self.in_key = false;
            let mut out = self.send(&before, f);
            out.push_str(&f.hidden(PRIVATE_KEY));
            return out;
        }
        let out = std::mem::take(&mut self.held);
        self.send(&out, f)
    }

    /// `out`, redacted with the shown text in front, hiding only from where `out` starts.
    fn send(&mut self, out: &str, f: &TextFilter) -> String {
        if out.is_empty() {
            return String::new();
        }
        let window = format!("{}{out}", self.shown);
        let redacted = f.redact_from(&window, self.shown.len());
        // The first bytes come back as they are; if a filter broke that promise, the new text is
        // redacted on its own rather than shown wrong.
        let text = match redacted.strip_prefix(self.shown.as_str()) {
            Some(rest) => rest.to_owned(),
            None => f.redact(out),
        };
        self.shown.push_str(out);
        self.trim_shown();
        text
    }

    /// Keep the current line only, and no more than [`CONTEXT_CHARS`] characters of it, cut at
    /// a character's start.
    fn trim_shown(&mut self) {
        if let Some(nl) = self.shown.rfind('\n') {
            self.shown.drain(..=nl);
        }
        if let Some((i, _)) = self.shown.char_indices().rev().nth(CONTEXT_CHARS) {
            self.shown.drain(..=i);
        }
    }
}

/// The byte index just after the last whitespace character in `s`, or 0 when it has none.
fn after_last_whitespace(s: &str) -> usize {
    s.char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8())
}

/// Where the private key in `held` ends: just after the `-----` that closes its `-----END …
/// KEY-----` line, or `None` while the END has not arrived.
fn key_end(held: &str) -> Option<usize> {
    let end = held.find(KEY_END)? + KEY_END.len();
    let dashes = held[end..].find("-----")?;
    Some(end + dashes + "-----".len())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use plenipo_guard::redact::{Redactor, MARKER};

    use super::*;
    use crate::agent::tools::TextRedaction;

    const STORED: &str = "hunter2-stored-value";
    const PHRASE: &str = "my secret phrase";

    /// Guard's redactor as the runtime's filter (the broker does the same in the app).
    struct Guarded(Redactor);

    impl TextRedaction for Guarded {
        fn redact(&self, text: &str) -> String {
            self.0.redact(text).into_owned()
        }
        fn redact_from(&self, text: &str, start: usize) -> String {
            self.0.redact_from(text, start).into_owned()
        }
        fn hidden(&self, what: &str) -> String {
            format!("{MARKER}{what}]")
        }
        fn stored_secret_start(&self, text: &str) -> Option<usize> {
            self.0.stored_secret_start(text)
        }
    }

    fn filter() -> TextFilter {
        Arc::new(Guarded(Redactor::new([
            (STORED.to_owned(), "Deploy key".to_owned()),
            (PHRASE.to_owned(), "Passphrase".to_owned()),
        ])))
    }

    fn whole(text: &str) -> String {
        filter().redact(text)
    }

    fn marker(what: &str) -> String {
        format!("{MARKER}{what}]")
    }

    /// `text` cut into `n` pieces of about the same size, at character boundaries.
    fn cut(text: &str, n: usize) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let size = chars.len().div_ceil(n).max(1);
        chars.chunks(size).map(|c| c.iter().collect()).collect()
    }

    /// The ways a stream might hand `text` over: in 2, 3, 5 and 7 pieces, and character by
    /// character.
    fn cuttings(text: &str) -> Vec<Vec<String>> {
        let mut ways: Vec<Vec<String>> = [2, 3, 5, 7].iter().map(|n| cut(text, *n)).collect();
        ways.push(text.chars().map(String::from).collect());
        ways
    }

    /// Stream `pieces` as words, then end the block: each piece's output, and the flush.
    fn stream(pieces: &[String]) -> (Vec<String>, String) {
        let f = filter();
        let mut live = LiveText::default();
        let outs: Vec<String> = pieces
            .iter()
            .map(|p| live.push(LiveKind::Words, p, Some(&f)))
            .collect();
        let flushed = live.flush(LiveKind::Words, Some(&f));
        (outs, flushed)
    }

    /// `piece` shows eight or more consecutive characters of `secret` (or all of a shorter one).
    fn leaks(piece: &str, secret: &str) -> bool {
        let s: Vec<char> = secret.chars().collect();
        s.windows(8.min(s.len())).any(|w| {
            let w: String = w.iter().collect();
            piece.contains(&w)
        })
    }

    /// Every secret format the redactor knows, with the part that must never show.
    fn secrets() -> Vec<(String, &'static str)> {
        vec![
            (
                "key sk-ant-api03-abcdefghijklmnopqrstuvwx done".into(),
                "sk-ant-api03-abcdefghijklmnopqrstuvwx",
            ),
            (
                "OPENAI sk-proj-ABCDEFGHIJKLMNOPQRSTUVWXYZ012345".into(),
                "sk-proj-ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
            ),
            (
                "stripe sk_live_ABCDEFGHIJKLMNOPQRST ok".into(),
                "sk_live_ABCDEFGHIJKLMNOPQRST",
            ),
            (
                "gh ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".into(),
                "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
            ),
            (
                "pat github_pat_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdef now".into(),
                "github_pat_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdef",
            ),
            ("aws AKIAIOSFODNN7EXAMPLE here".into(), "AKIAIOSFODNN7EXAMPLE"),
            (
                "google AIzaSyA-abcdefghijklmnopqrstuvwxyz01234 x".into(),
                "AIzaSyA-abcdefghijklmnopqrstuvwxyz01234",
            ),
            (
                "slack xoxb-1234567890-abcdefghij end".into(),
                "xoxb-1234567890-abcdefghij",
            ),
            (
                "grok xai-ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".into(),
                "xai-ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
            ),
            (
                "npm ERR! npm_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".into(),
                "npm_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
            ),
            (
                "jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U".into(),
                "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U",
            ),
            (
                "Authorization: Bearer abcdefghijklmnopqrstuvwxyz\n".into(),
                "abcdefghijklmnopqrstuvwxyz",
            ),
            (
                "db postgres://app:pa55word@db:5432/x ok".into(),
                "pa55word",
            ),
            ("DB_PASSWORD=hunter22\nPORT=8080\n".into(), "hunter22"),
            (
                "export GITHUB_TOKEN=\"abcd1234\"\n".into(),
                "abcd1234",
            ),
            (
                r#"{"apiKey": "zzzz9999", "name": "x"}"#.into(),
                "zzzz9999",
            ),
            (
                "the key:\n-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaA\nQUJDREVGR0g\n-----END OPENSSH PRIVATE KEY-----\nthat was it".into(),
                "b3BlbnNzaA",
            ),
            (format!("token={STORED} and more"), STORED),
            (format!("please use {PHRASE} now"), PHRASE),
        ]
    }

    /// ADR-216: however a stream cuts the text, no piece shows a secret, and the pieces joined
    /// read exactly as the whole text redacted.
    #[test]
    fn a_secret_split_across_pieces_never_shows() {
        for (text, secret) in secrets() {
            assert!(
                whole(&text).contains(MARKER),
                "the redactor knows this one: {text}"
            );
            for pieces in cuttings(&text) {
                let (outs, flushed) = stream(&pieces);
                for out in &outs {
                    assert!(!leaks(out, secret), "{pieces:?} → {outs:?}");
                }
                assert!(!leaks(&flushed, secret), "{pieces:?} → flush {flushed:?}");
                let joined = format!("{}{flushed}", outs.join(""));
                assert_eq!(joined, whole(&text), "{pieces:?}");
            }
        }
    }

    /// A private key streamed line by line shows nothing from its BEGIN until its END; one the
    /// stream never finishes is hidden when the block ends.
    #[test]
    fn a_private_key_is_held_from_begin_to_end() {
        let f = filter();
        let mut live = LiveText::default();
        let lines = [
            "Here it is:\n",
            "-----BEGIN RSA PRIVATE KEY-----\n",
            "MIIEowIBAAKCAQEA\n",
            "7Yx2z9k3Lm0pQrSt\n",
            "-----END RSA PRIVATE KEY-----\n",
            "Done.",
        ];
        let outs: Vec<String> = lines
            .iter()
            .map(|l| live.push(LiveKind::Words, l, Some(&f)))
            .collect();
        assert_eq!(
            outs,
            [
                "Here it is:\n",
                "",
                "",
                "",
                &format!("{}\n", marker(PRIVATE_KEY)),
                ""
            ]
        );
        assert_eq!(live.flush(LiveKind::Words, Some(&f)), "Done.");
        let mut live = LiveText::default();
        for l in &lines[..3] {
            live.push(LiveKind::Words, l, Some(&f));
        }
        let flushed = live.flush(LiveKind::Words, Some(&f));
        assert_eq!(flushed, marker(PRIVATE_KEY));
        // After the block ends, the key is over: new text streams again.
        assert_eq!(live.push(LiveKind::Words, "next ", Some(&f)), "next ");
    }

    /// The BEGIN may arrive cut across pieces, or glued to a word (`key="-----BEGIN…`): it is
    /// found before anything of that word goes on.
    #[test]
    fn a_split_or_glued_begin_is_still_held() {
        let f = filter();
        let mut live = LiveText::default();
        let outs: Vec<String> = [
            "see -----BE",
            "GIN RSA PRIVATE KEY-----\nMIIE\n",
            "-----END RSA PRIVATE KEY-----\n",
        ]
        .iter()
        .map(|p| live.push(LiveKind::Words, p, Some(&f)))
        .collect();
        assert_eq!(outs, ["see ", "", &format!("{}\n", marker(PRIVATE_KEY))]);
        let mut live = LiveText::default();
        let outs: Vec<String> = [
            "key=\"-----BEGIN RSA PRIVATE KEY-----\nMIIE\n",
            "-----END RSA PRIVATE KEY-----\" ok\n",
        ]
        .iter()
        .map(|p| live.push(LiveKind::Words, p, Some(&f)))
        .collect();
        assert_eq!(outs, ["", &format!("key=\"{}\" ok\n", marker(PRIVATE_KEY))]);
        assert_eq!(
            whole(
                "key=\"-----BEGIN RSA PRIVATE KEY-----\nMIIE\n-----END RSA PRIVATE KEY-----\" ok\n"
            ),
            format!("key=\"{}\" ok\n", marker(PRIVATE_KEY)),
            "the live and the stored copy read the same"
        );
    }

    /// A key longer than the cap sends a marker per cap and stays a key until its END; nothing
    /// of it shows.
    #[test]
    fn a_very_long_key_is_marked_per_cap_and_never_shown() {
        let f = filter();
        let mut live = LiveText::default();
        assert_eq!(
            live.push(
                LiveKind::Words,
                "-----BEGIN RSA PRIVATE KEY-----\n",
                Some(&f)
            ),
            ""
        );
        let body = format!("{}\n", "Q".repeat(1000));
        let mut outs = Vec::new();
        for _ in 0..200 {
            outs.push(live.push(LiveKind::Words, &body, Some(&f)));
        }
        let markers = outs.iter().filter(|o| !o.is_empty()).count();
        assert!(markers >= 2, "{markers} markers for 200 KB of key");
        assert!(
            outs.iter()
                .all(|o| o.is_empty() || *o == marker(PRIVATE_KEY)),
            "{outs:?}"
        );
        assert_eq!(
            live.push(
                LiveKind::Words,
                "-----END RSA PRIVATE KEY-----\nafter ",
                Some(&f)
            ),
            format!("{}\nafter ", marker(PRIVATE_KEY))
        );
        assert_eq!(live.flush(LiveKind::Words, Some(&f)), "");
    }

    /// Ordinary text streams with at most the word being written held back; a block that ends
    /// mid-word sends the rest on; without a filter nothing is held.
    #[test]
    fn ordinary_text_waits_only_for_the_word_being_written() {
        let f = filter();
        let mut live = LiveText::default();
        assert_eq!(live.push(LiveKind::Words, "Hel", Some(&f)), "");
        assert_eq!(live.push(LiveKind::Words, "lo wor", Some(&f)), "Hello ");
        assert_eq!(live.push(LiveKind::Words, "ld", Some(&f)), "");
        assert_eq!(live.flush(LiveKind::Words, Some(&f)), "world");
        assert_eq!(live.flush(LiveKind::Words, Some(&f)), "", "nothing twice");
        let mut live = LiveText::default();
        for word in ["The ", "quick\u{a0}", "brown\t", "fox\n"] {
            assert_eq!(live.push(LiveKind::Words, word, Some(&f)), word);
        }
        let mut live = LiveText::default();
        assert_eq!(
            live.push(LiveKind::Words, "sk-ant-api03-abc", None),
            "sk-ant-api03-abc"
        );
        assert_eq!(live.flush(LiveKind::Words, None), "");
    }

    /// A turn that ends in the middle of a key sends it on hidden.
    #[test]
    fn a_turn_that_ends_mid_word_flushes_it_redacted() {
        let f = filter();
        let mut live = LiveText::default();
        assert_eq!(
            live.push(
                LiveKind::Words,
                "key sk-ant-api03-abcdefghijklmnop",
                Some(&f)
            ),
            "key "
        );
        assert_eq!(live.push(LiveKind::Words, "qrstuvwx", Some(&f)), "");
        assert_eq!(live.flush(LiveKind::Words, Some(&f)), marker("API key"));
    }

    /// The redactor sees what was already shown, so a prefix that went on earlier (`export `,
    /// `Bearer `) still makes the held word a secret; what was shown is never shown again, and a
    /// marker already sent on is never matched again.
    #[test]
    fn the_context_keeps_prefix_patterns_working_and_nothing_shows_twice() {
        let f = filter();
        let mut live = LiveText::default();
        let outs: Vec<String> = ["export GITHUB_", "TOKEN=\"abcd", "1234\"\n", "PORT=8080\n"]
            .iter()
            .map(|p| live.push(LiveKind::Words, p, Some(&f)))
            .collect();
        assert_eq!(
            outs,
            [
                "export ",
                "",
                &format!("GITHUB_TOKEN=\"{}\"\n", marker("secret setting")),
                "PORT=8080\n"
            ]
        );
        let mut live = LiveText::default();
        let outs: Vec<String> = ["Authorization: Bearer ", "abcdefghijklm", "nopqrstuvwxyz\n"]
            .iter()
            .map(|p| live.push(LiveKind::Words, p, Some(&f)))
            .collect();
        assert_eq!(
            outs,
            [
                "Authorization: Bearer ",
                "",
                &format!("{}\n", marker("token"))
            ]
        );
        // A stored secret hidden by name, then a setting line right after: one marker each.
        let mut live = LiveText::default();
        let outs: Vec<String> = [
            format!("PASSWORD={STORED}\n"),
            "NEXT=value1234\n".to_owned(),
        ]
        .iter()
        .map(|p| live.push(LiveKind::Words, p, Some(&f)))
        .collect();
        assert_eq!(
            outs,
            [
                format!("PASSWORD={}\n", marker("Deploy key")),
                "NEXT=value1234\n".to_owned()
            ]
        );
    }

    /// The context is the current line, at most 200 characters, cut at a character's start: a
    /// multi-byte character there never panics, and a line's own start is kept for `^`.
    #[test]
    fn the_context_is_the_current_line_and_cut_at_a_character() {
        let f = filter();
        let mut live = LiveText::default();
        let word = "héllo ";
        for _ in 0..60 {
            live.push(LiveKind::Words, word, Some(&f));
        }
        assert!(live.words.shown.chars().count() <= CONTEXT_CHARS);
        assert!(live.words.shown.is_char_boundary(0));
        assert!(live.words.shown.ends_with(word));
        // A setting on a new line is still seen from its line start.
        assert_eq!(
            live.push(LiveKind::Words, "\nDB_PASSWORD=hunter22\n", Some(&f)),
            format!("\nDB_PASSWORD={}\n", marker("secret setting"))
        );
        assert!(!live.words.shown.contains('\n'));
        // Pieces that cut inside a multi-byte character still come out whole.
        let mut live = LiveText::default();
        let text = "naïve café ünïcödé sk-ant-api03-abcdefghijklmnopqrstuvwx done";
        let mut joined = String::new();
        for chunk in text.as_bytes().chunks(5) {
            // A stream hands over whole characters; feed them as such.
            let piece = String::from_utf8_lossy(chunk).into_owned();
            joined.push_str(&live.push(LiveKind::Words, &piece, Some(&f)));
        }
        joined.push_str(&live.flush(LiveKind::Words, Some(&f)));
        assert!(joined.contains(&marker("API key")), "{joined}");
        assert!(!joined.contains("sk-ant-api03"), "{joined}");
    }

    /// A stored secret with whitespace in it is held from its first word until it is whole.
    #[test]
    fn a_stored_secret_with_spaces_is_held_whole() {
        let f = filter();
        let mut live = LiveText::default();
        let outs: Vec<String> = ["please use my ", "secret ", "phrase now ", "ok "]
            .iter()
            .map(|p| live.push(LiveKind::Words, p, Some(&f)))
            .collect();
        assert_eq!(
            outs,
            [
                "please use ",
                "",
                &format!("{} now ", marker("Passphrase")),
                "ok "
            ]
        );
        // Words that only look like a beginning go on once they turn out not to be.
        let mut live = LiveText::default();
        assert_eq!(live.push(LiveKind::Words, "use my ", Some(&f)), "use ");
        assert_eq!(
            live.push(LiveKind::Words, "friend ", Some(&f)),
            "my friend "
        );
        // The stated residual: a JSON setting value with spaces (a pattern, not a stored
        // secret) shows its first word; the rest is hidden once the value is whole.
        let mut live = LiveText::default();
        let outs: Vec<String> = [r#"{"apiKey": "two "#, r#"words", "n": 1}"#]
            .iter()
            .map(|p| live.push(LiveKind::Words, p, Some(&f)))
            .collect();
        assert_eq!(outs[0], r#"{"apiKey": "two "#);
        assert_eq!(outs[1], format!("{}\", \"n\": ", marker("secret setting")));
        assert_eq!(live.flush(LiveKind::Words, Some(&f)), "1}");
    }

    /// Words and thinking are held apart.
    #[test]
    fn words_and_thinking_are_held_apart() {
        let f = filter();
        let mut live = LiveText::default();
        assert_eq!(
            live.push(
                LiveKind::Words,
                "I will use sk-ant-api03-abcdefghijklmnop",
                Some(&f)
            ),
            "I will use "
        );
        assert_eq!(
            live.push(LiveKind::Thinking, "the key is sk-ant-api03-abcd", Some(&f)),
            "the key is "
        );
        assert_eq!(
            live.push(LiveKind::Thinking, "efghijklmnopqrstuvwxyz ok", Some(&f)),
            format!("{} ", marker("API key"))
        );
        // Each kind keeps its own held word: the words' key is still whole and hidden.
        assert_eq!(live.flush(LiveKind::Words, Some(&f)), marker("API key"));
        assert_eq!(live.flush(LiveKind::Thinking, Some(&f)), "ok");
        assert_eq!(
            LiveKind::Words.event("a".into()),
            AgentEvent::TextDelta { text: "a".into() }
        );
        assert_eq!(
            LiveKind::Thinking.event("b".into()),
            AgentEvent::Reasoning { text: "b".into() }
        );
    }

    /// Which events end a block of live text: held words go on before a "Thinking" sign, so
    /// words, the sign, then thinking keep that order on screen (#200); a wait for the AI company
    /// does not interrupt a word.
    #[test]
    fn the_thinking_sign_ends_a_block_and_a_wait_does_not() {
        use crate::agent::dto::NoticeLevel;
        use crate::dto::TokenUsage;
        let status = |phase| AgentEvent::Status {
            phase,
            text: String::new(),
        };
        assert!(ends_blocks(&status(StatusPhase::Thinking)));
        assert!(ends_blocks(&status(StatusPhase::Starting)));
        assert!(!ends_blocks(&status(StatusPhase::Waiting)));
        assert!(!ends_blocks(&AgentEvent::Usage {
            usage: TokenUsage::default()
        }));
        assert!(!ends_blocks(&AgentEvent::TextDelta { text: "a".into() }));
        assert!(!ends_blocks(&AgentEvent::Reasoning { text: "a".into() }));
        for ending in [
            AgentEvent::Message { text: "m".into() },
            AgentEvent::ToolUse {
                tool: "t".into(),
                summary: String::new(),
                id: None,
            },
            AgentEvent::ToolResult {
                tool: None,
                is_error: false,
                summary: String::new(),
                id: None,
            },
            AgentEvent::Notice {
                level: NoticeLevel::Info,
                text: String::new(),
            },
            AgentEvent::MemoryShortened {
                detail: String::new(),
            },
            AgentEvent::SessionStarted {
                provider_session_id: None,
                model: None,
            },
        ] {
            assert!(ends_blocks(&ending), "{ending:?}");
        }
        // The sequence the runtime makes of it: held words, the sign, then the thinking.
        let f = filter();
        let mut live = LiveText::default();
        let mut screen: Vec<String> = Vec::new();
        screen.push(live.push(LiveKind::Words, "Let me check the fil", Some(&f)));
        let sign = status(StatusPhase::Thinking);
        if ends_blocks(&sign) {
            screen.push(live.flush(LiveKind::Words, Some(&f)));
        }
        screen.push("[Thinking]".into());
        screen.push(live.push(LiveKind::Thinking, "Reading the ", Some(&f)));
        assert_eq!(
            screen,
            ["Let me check the ", "fil", "[Thinking]", "Reading the "]
        );
    }

    /// A run with no whitespace longer than the cap goes on anyway, hidden as far as it can be.
    #[test]
    fn a_very_long_run_is_not_held_for_good() {
        let f = filter();
        let mut live = LiveText::default();
        let run = "a".repeat(MAX_HELD_BYTES / 2);
        assert_eq!(live.push(LiveKind::Words, &run, Some(&f)), "");
        let out = live.push(LiveKind::Words, &format!("{run}bb"), Some(&f));
        assert_eq!(out.len(), MAX_HELD_BYTES + 2);
        assert_eq!(live.flush(LiveKind::Words, Some(&f)), "");
    }
}
