//! Prompts sized to the job (ADR-044): what a step sends, and how big it is.
//!
//! Plenipo measures every step's prompt — the whole of it, and its own text (everything except
//! what it only passes along: the objective from the owner or a lead, context from another
//! worker, replies) — and records the sizes with the step. Sizes only: the text is never kept.

use crate::agent::tools::{NOTE_END, NOTE_START};
use crate::dto::{BriefKind, BriefWhy, NoteKind, PromptSize};

/// An objective that, with the context handed with it, is this long or longer is a large job:
/// it always goes out with the full instructions (ADR-044 §2.7).
pub const LARGE_JOB_CHARS: usize = 4_000;

/// Liaison's message for the first step of a turn (ADR-044): the full instructions, and — for a
/// conversation that already has them — the same message with a short reminder instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefInput {
    /// The message with the full instructions (who the worker is, its job, its team, how to hand
    /// work on, its lessons).
    pub full: String,
    /// The same message with a short reminder in place of the instructions; `None`: the full
    /// message always goes out.
    pub reminder: Option<String>,
    /// Bytes of `full` Plenipo only passes along: the objective from the owner or a lead,
    /// context from another worker.
    pub passed_bytes: usize,
    /// The same for `reminder` (less when it refers to a saved record instead of pasting it).
    pub reminder_passed_bytes: usize,
    /// Identifies the instructions (not the objective or its context): when it changes, the
    /// full message goes out again.
    pub hash: u64,
    /// A large job ([`LARGE_JOB_CHARS`]): the full message goes out.
    pub large: bool,
}

/// A stable hash of a text (64-bit FNV-1a): the same text gives the same number in every run.
pub fn text_hash(text: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    text.bytes()
        .fold(OFFSET, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME))
}

/// Bytes the tools note adds in front of a message ([`crate::agent::tools::with_note`]).
pub fn note_overhead(note: Option<&str>) -> usize {
    note.map_or(0, |n| {
        NOTE_START.len() + 1 + n.trim().len() + 1 + NOTE_END.len() + 2
    })
}

/// What a step sends before the tools note, and how it counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outgoing {
    pub text: String,
    /// Bytes of `text` Plenipo only passes along.
    pub passed: usize,
    pub kind: BriefKind,
    pub why: Option<BriefWhy>,
    /// The full message's length and passed bytes, for what Plenipo's own text would have been
    /// with it.
    pub full_len: usize,
    pub full_passed: usize,
}

impl Outgoing {
    /// A message sent as it is: a continuation's replies, or a plain objective.
    pub fn plain(text: String, passed: usize, kind: BriefKind) -> Self {
        let full_len = text.len();
        Self {
            text,
            passed,
            kind,
            why: None,
            full_len,
            full_passed: passed,
        }
    }

    /// The step's size once the note is chosen: `note` is the note sent (if any) and `full_note`
    /// the whole note it stands for.
    pub fn size(&self, note: Option<&str>, kind: NoteKind, full_note: Option<&str>) -> PromptSize {
        let bytes = note_overhead(note) + self.text.len();
        let full = note_overhead(full_note) + self.full_len;
        PromptSize {
            bytes: clamp(bytes),
            own_bytes: clamp(bytes.saturating_sub(self.passed)),
            brief: self.kind,
            why: self.why,
            full_own_bytes: clamp(full.saturating_sub(self.full_passed)),
            note: kind,
        }
    }
}

fn clamp(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::with_note;

    #[test]
    fn the_hash_is_stable_and_tells_texts_apart() {
        assert_eq!(text_hash(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(text_hash("a"), 0xaf63_dc4c_8601_ec8c);
        assert_ne!(text_hash("team: A, B"), text_hash("team: A, C"));
    }

    #[test]
    fn sizes_count_the_note_and_leave_out_what_is_passed_along() {
        let note = "  You may read files.\n";
        let message = "[Plenipo Liaison — instructions]\n…\n\nWrite a parser";
        let sent = with_note(note, message);
        assert_eq!(note_overhead(Some(note)) + message.len(), sent.len());
        assert_eq!(note_overhead(None), 0);

        let out = Outgoing::plain(message.into(), "Write a parser".len(), BriefKind::Plain);
        let size = out.size(Some(note), NoteKind::Full, Some(note));
        assert_eq!(size.bytes as usize, sent.len());
        assert_eq!(size.own_bytes as usize, sent.len() - "Write a parser".len());
        assert_eq!(size.full_own_bytes, size.own_bytes);
        assert_eq!(
            (size.brief, size.why, size.note),
            (BriefKind::Plain, None, NoteKind::Full)
        );
        // Nothing passed along: all of it is Plenipo's own text.
        let own =
            Outgoing::plain("x".into(), 0, BriefKind::Replies).size(None, NoteKind::None, None);
        assert_eq!((own.bytes, own.own_bytes, own.full_own_bytes), (1, 1, 1));
    }
}
