//! Watch (Phase 18, ADR-055): the file changes Plenipo carries out for workers, shown as they
//! happen. The broker hands each change to the [`WatchHub`] as it applies it (or refuses it), and
//! each preview of a change an AI tool is still writing, once Guard's checks passed; the hub keeps
//! the lines of each change in memory while Plenipo runs (64 MB in all, the oldest first out),
//! marks new and changed lines, and tells the main window. Nothing here is written to disk: the
//! Ledger keeps only the record of each saved change (`capability.used`).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A file larger than this, or with more lines, is shown as a summary.
pub const MAX_SHOWN_BYTES: usize = 256 * 1024;
pub const MAX_SHOWN_LINES: usize = 5_000;
/// The most a preview shows while it is being written.
pub const MAX_PREVIEW_BYTES: usize = 256 * 1024;
/// The lines of saved changes kept at once.
pub const MAX_KEPT_BYTES: usize = 64 * 1024 * 1024;
/// Changes remembered at once (their records; their lines go first).
const MAX_CHANGES: usize = 5_000;
/// What a change Plenipo will not show says.
pub const HIDDEN: &str = "A change Plenipo will not show";

/// Where a change stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WatchState {
    /// The AI tool is writing it now: not saved yet.
    Writing,
    /// It waits for the owner's approval.
    Waiting,
    Saved,
    /// Guard, or the owner, said no.
    Refused,
    /// The AI tool stopped before sending it, or it could not be made.
    NotSaved,
}

/// Whether a saved change made the file or changed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChangeKind {
    Created,
    Changed,
}

/// How a line is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LineMark {
    New,
    Changed,
}

/// One change to one file by one worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchChange {
    pub id: String,
    pub task_id: String,
    /// The worker's conversation (Stop stops its task, as on the Workers page).
    pub session_id: String,
    #[ts(optional)]
    pub position_id: Option<String>,
    pub worker: String,
    /// The objective it belongs to (the root task).
    pub objective_task_id: String,
    /// Inside the working copy, with `/`.
    pub path: String,
    pub state: WatchState,
    #[ts(optional)]
    pub kind: Option<ChangeKind>,
    pub added: u32,
    pub removed: u32,
    /// Why it was refused or not saved.
    #[ts(optional)]
    pub reason: Option<String>,
    /// Shown instead of the lines: a large or non-text file, or lines no longer kept.
    #[ts(optional)]
    pub summary: Option<String>,
    #[ts(type = "number")]
    pub at: u64,
}

/// One line of a file, as Watch shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchLine {
    pub text: String,
    #[ts(optional)]
    pub mark: Option<LineMark>,
    /// Lines removed just before this one.
    pub removed_before: u32,
}

/// A change's file: the file after the change, its lines marked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchFileView {
    pub change: WatchChange,
    pub lines: Vec<WatchLine>,
    /// Lines removed at the end of the file.
    pub removed_at_end: u32,
    /// While it is being written: the text so far (secrets hidden), not saved.
    #[ts(optional)]
    pub writing: Option<String>,
}

/// What a worker changed in its objective: each file's latest change, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchView {
    pub position_id: String,
    #[ts(optional)]
    pub objective_task_id: Option<String>,
    pub changes: Vec<WatchChange>,
    /// Some changes are from before Plenipo started again: their lines are not kept.
    pub from_the_record: bool,
}

/// What the main window hears as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchUpdate {
    pub change: WatchChange,
    /// While it is being written: the text so far (secrets hidden).
    #[ts(optional)]
    pub writing: Option<String>,
}

pub type WatchListener = Arc<dyn Fn(&WatchUpdate) + Send + Sync>;

/// Who made a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Who {
    pub task_id: String,
    pub session_id: String,
    pub position_id: Option<String>,
    pub worker: String,
    pub objective_task_id: String,
}

/// What a file was before a change Plenipo made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Before {
    /// It did not exist.
    Missing,
    Text(String),
    /// Too large to read for Watch, or not text.
    Unshown {
        bytes: u64,
        binary: bool,
    },
}

/// A change Plenipo made: the file before and after (secrets hidden in both).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub before: Before,
    pub after: String,
}

/// The counts of a change: lines added and removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub added: u32,
    pub removed: u32,
}

struct Marked {
    lines: Vec<WatchLine>,
    removed_at_end: u32,
    bytes: usize,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Mark `after`'s lines against `before`: new, changed, and where lines went.
fn mark(before: &str, after: &str) -> (Counts, Marked) {
    let diff = similar::TextDiff::from_lines(before, after);
    let new_lines: Vec<&str> = diff
        .new_slices()
        .iter()
        .map(|l| l.strip_suffix('\n').unwrap_or(l))
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let mut marks: Vec<Option<LineMark>> = vec![None; new_lines.len()];
    let mut removed_before = vec![0u32; new_lines.len() + 1];
    let mut counts = Counts::default();
    for op in diff.ops() {
        let (tag, old, new) = op.as_tag_tuple();
        match tag {
            similar::DiffTag::Equal => {}
            similar::DiffTag::Insert => {
                counts.added += count(new.len());
                for m in &mut marks[new.clone()] {
                    *m = Some(LineMark::New);
                }
            }
            similar::DiffTag::Delete => {
                counts.removed += count(old.len());
                removed_before[new.start] += count(old.len());
            }
            similar::DiffTag::Replace => {
                counts.added += count(new.len());
                counts.removed += count(old.len());
                for m in &mut marks[new.clone()] {
                    *m = Some(LineMark::Changed);
                }
                if old.len() > new.len() {
                    removed_before[new.end] += count(old.len() - new.len());
                }
            }
        }
    }
    let lines: Vec<WatchLine> = new_lines
        .iter()
        .zip(marks)
        .enumerate()
        .map(|(i, (text, mark))| WatchLine {
            text: (*text).to_owned(),
            mark,
            removed_before: removed_before[i],
        })
        .collect();
    let bytes = after.len();
    (
        counts,
        Marked {
            lines,
            removed_at_end: removed_before[new_lines.len()],
            bytes,
        },
    )
}

fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", bytes.div_ceil(1024))
    } else {
        format!("{bytes} bytes")
    }
}

fn plural(n: u32, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

/// Why lines are not shown, when they are not.
fn summary_of(written: &Written, counts: Option<Counts>) -> Option<String> {
    if let Before::Unshown {
        bytes,
        binary: true,
    } = written.before
    {
        return Some(format!("Not a text file ({})", size(bytes)));
    }
    let large = written.after.len() > MAX_SHOWN_BYTES
        || written.after.lines().count() > MAX_SHOWN_LINES
        || matches!(written.before, Before::Unshown { binary: false, .. });
    if !large {
        return None;
    }
    let mut s = format!("Large file: {}", size(written.after.len() as u64));
    if let Some(c) = counts {
        s.push_str(&format!(
            " · {} added, {} removed",
            plural(c.added, "line"),
            c.removed
        ));
    }
    Some(s)
}

struct Kept {
    change: WatchChange,
    marked: Option<Marked>,
    /// While being written: the text so far.
    writing: Option<String>,
    session_id: String,
}

#[derive(Default)]
struct State {
    changes: HashMap<String, Kept>,
    /// Oldest first.
    order: VecDeque<String>,
    kept_bytes: usize,
    /// (conversation, path) → the change being written or waiting there.
    open: HashMap<(String, String), String>,
    /// A tool call's preview → its change.
    previews: HashMap<(String, String), String>,
}

/// The changes Watch shows, while Plenipo runs.
#[derive(Default)]
pub struct WatchHub {
    state: Mutex<State>,
    listener: RwLock<Option<WatchListener>>,
}

impl WatchHub {
    pub fn set_listener(&self, listener: WatchListener) {
        *self.listener.write().unwrap_or_else(|p| p.into_inner()) = Some(listener);
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn tell(&self, update: &WatchUpdate) {
        let listener = self
            .listener
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(listener) = listener {
            listener(update);
        }
    }

    fn new_change(who: &Who, path: &str, state: WatchState) -> WatchChange {
        WatchChange {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: who.task_id.clone(),
            session_id: who.session_id.clone(),
            position_id: who.position_id.clone(),
            worker: who.worker.clone(),
            objective_task_id: who.objective_task_id.clone(),
            path: path.to_owned(),
            state,
            kind: None,
            added: 0,
            removed: 0,
            reason: None,
            summary: None,
            at: plenipo_ledger::now_ms(),
        }
    }

    /// Drop the oldest lines (then the oldest records) until what is kept fits.
    fn trim(state: &mut State) {
        let mut i = 0;
        while state.kept_bytes > MAX_KEPT_BYTES && i < state.order.len() {
            let id = state.order[i].clone();
            if let Some(k) = state.changes.get_mut(&id) {
                if let Some(m) = k.marked.take() {
                    state.kept_bytes = state.kept_bytes.saturating_sub(m.bytes);
                    k.change.summary = Some(format!(
                        "{} added, {} removed. The lines are no longer kept; the file is in \
                         the working copy.",
                        plural(k.change.added, "line"),
                        k.change.removed
                    ));
                }
            }
            i += 1;
        }
        while state.order.len() > MAX_CHANGES {
            if let Some(id) = state.order.pop_front() {
                if let Some(k) = state.changes.remove(&id) {
                    if let Some(m) = k.marked {
                        state.kept_bytes = state.kept_bytes.saturating_sub(m.bytes);
                    }
                }
            }
        }
    }

    /// The open change in `who`'s conversation for `path` (being written or waiting), or a new
    /// one.
    fn take_open(state: &mut State, who: &Who, path: &str) -> Option<String> {
        state
            .open
            .remove(&(who.session_id.clone(), path.to_owned()))
            .filter(|id| state.changes.contains_key(id))
    }

    fn settle(&self, who: &Who, path: &str, apply: impl FnOnce(&mut Kept)) -> WatchChange {
        let change = {
            let mut state = self.state();
            let id = Self::take_open(&mut state, who, path);
            let id = match id {
                Some(id) => id,
                None => {
                    let change = Self::new_change(who, path, WatchState::Writing);
                    let id = change.id.clone();
                    state.order.push_back(id.clone());
                    state.changes.insert(
                        id.clone(),
                        Kept {
                            change,
                            marked: None,
                            writing: None,
                            session_id: who.session_id.clone(),
                        },
                    );
                    id
                }
            };
            state.previews.retain(|_, v| *v != id);
            let kept = state.changes.get_mut(&id).expect("just found or made");
            let before = kept.marked.as_ref().map_or(0, |m| m.bytes);
            kept.writing = None;
            kept.change.at = plenipo_ledger::now_ms();
            apply(kept);
            let after = kept.marked.as_ref().map_or(0, |m| m.bytes);
            let change = kept.change.clone();
            state.kept_bytes = state.kept_bytes.saturating_sub(before) + after;
            Self::trim(&mut state);
            change
        };
        self.tell(&WatchUpdate {
            change: change.clone(),
            writing: None,
        });
        change
    }

    /// Plenipo saved a change to `path` (secrets already hidden in `written`).
    pub fn saved(&self, who: &Who, path: &str, written: &Written) -> (WatchChange, Counts) {
        let shown_before = match &written.before {
            Before::Missing => Some(""),
            Before::Text(t) => Some(t.as_str()),
            Before::Unshown { .. } => None,
        };
        let marked = shown_before.map(|b| mark(b, &written.after));
        let counts = marked.as_ref().map(|(c, _)| *c).unwrap_or_default();
        let summary = summary_of(written, marked.as_ref().map(|(c, _)| *c));
        let change = self.settle(who, path, |k| {
            k.change.state = WatchState::Saved;
            k.change.kind = Some(if written.before == Before::Missing {
                ChangeKind::Created
            } else {
                ChangeKind::Changed
            });
            k.change.added = counts.added;
            k.change.removed = counts.removed;
            k.change.reason = None;
            k.change.summary = summary.clone();
            k.marked = match (summary.is_none(), marked) {
                (true, Some((_, m))) => Some(m),
                _ => None,
            };
        });
        (change, counts)
    }

    /// Guard or the owner refused a change to `path`: only its name and why are kept.
    pub fn refused(&self, who: &Who, path: &str, why: &str) -> WatchChange {
        self.ended(who, path, WatchState::Refused, why)
    }

    /// A change to `path` could not be made ("the text to replace was not found").
    pub fn not_saved(&self, who: &Who, path: &str, why: &str) -> WatchChange {
        self.ended(who, path, WatchState::NotSaved, why)
    }

    fn ended(&self, who: &Who, path: &str, state: WatchState, why: &str) -> WatchChange {
        self.settle(who, path, |k| {
            k.change.state = state;
            k.change.reason = Some(why.to_owned());
            k.change.kind = None;
            k.change.added = 0;
            k.change.removed = 0;
            k.change.summary = None;
            k.marked = None;
        })
    }

    /// A change to `path` waits for the owner's approval.
    pub fn waiting(&self, who: &Who, path: &str) -> WatchChange {
        let change = {
            let mut state = self.state();
            let key = (who.session_id.clone(), path.to_owned());
            let id = match state.open.get(&key).cloned() {
                Some(id) if state.changes.contains_key(&id) => id,
                _ => {
                    let change = Self::new_change(who, path, WatchState::Waiting);
                    let id = change.id.clone();
                    state.order.push_back(id.clone());
                    state.changes.insert(
                        id.clone(),
                        Kept {
                            change,
                            marked: None,
                            writing: None,
                            session_id: who.session_id.clone(),
                        },
                    );
                    state.open.insert(key, id.clone());
                    id
                }
            };
            let kept = state.changes.get_mut(&id).expect("just found or made");
            kept.change.state = WatchState::Waiting;
            kept.change.at = plenipo_ledger::now_ms();
            (kept.change.clone(), kept.writing.clone())
        };
        self.tell(&WatchUpdate {
            change: change.0.clone(),
            writing: change.1,
        });
        change.0
    }

    /// The text so far of a change an AI tool is writing to `path` (tool call `call`), checked
    /// and with secrets hidden by the broker. At most [`MAX_PREVIEW_BYTES`] are shown.
    pub fn writing(&self, who: &Who, call: &str, path: &str, text: &str) -> WatchChange {
        let mut text = text.to_owned();
        let mut summary = None;
        if text.len() > MAX_PREVIEW_BYTES {
            let mut cut = MAX_PREVIEW_BYTES;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
            summary = Some("Large file: the rest shows when it is saved".to_owned());
        }
        let change = {
            let mut state = self.state();
            let call_key = (who.session_id.clone(), call.to_owned());
            let id = match state.previews.get(&call_key).cloned() {
                Some(id) if state.changes.contains_key(&id) => id,
                _ => {
                    let change = Self::new_change(who, path, WatchState::Writing);
                    let id = change.id.clone();
                    state.order.push_back(id.clone());
                    state.changes.insert(
                        id.clone(),
                        Kept {
                            change,
                            marked: None,
                            writing: None,
                            session_id: who.session_id.clone(),
                        },
                    );
                    state.previews.insert(call_key, id.clone());
                    state
                        .open
                        .insert((who.session_id.clone(), path.to_owned()), id.clone());
                    id
                }
            };
            let kept = state.changes.get_mut(&id).expect("just found or made");
            kept.writing = Some(text.clone());
            kept.change.summary = summary;
            kept.change.at = plenipo_ledger::now_ms();
            kept.change.clone()
        };
        self.tell(&WatchUpdate {
            change: change.clone(),
            writing: Some(text),
        });
        change
    }

    /// A change an AI tool is writing to a file Plenipo will not show (outside its working copy,
    /// a blocked file, or git's own files): its name only, never its text.
    pub fn hidden(&self, who: &Who, call: &str, path: &str) -> WatchChange {
        let change = {
            let mut state = self.state();
            let call_key = (who.session_id.clone(), call.to_owned());
            let id = match state.previews.get(&call_key).cloned() {
                Some(id) if state.changes.contains_key(&id) => id,
                _ => {
                    let change = Self::new_change(who, path, WatchState::Writing);
                    let id = change.id.clone();
                    state.order.push_back(id.clone());
                    state.changes.insert(
                        id.clone(),
                        Kept {
                            change,
                            marked: None,
                            writing: None,
                            session_id: who.session_id.clone(),
                        },
                    );
                    state.previews.insert(call_key, id.clone());
                    state
                        .open
                        .insert((who.session_id.clone(), path.to_owned()), id.clone());
                    id
                }
            };
            let kept = state.changes.get_mut(&id).expect("just found or made");
            kept.writing = None;
            kept.change.summary = Some(HIDDEN.to_owned());
            kept.change.at = plenipo_ledger::now_ms();
            kept.change.clone()
        };
        self.tell(&WatchUpdate {
            change: change.clone(),
            writing: None,
        });
        change
    }

    /// A step ended: changes still being written in its conversation were never sent.
    pub fn step_ended(&self, session_id: &str) {
        let ended: Vec<WatchChange> = {
            let mut state = self.state();
            let open: Vec<(String, String)> = state
                .open
                .iter()
                .filter(|((s, _), _)| s == session_id)
                .map(|(k, _)| k.clone())
                .collect();
            let mut out = Vec::new();
            for key in open {
                if let Some(id) = state.open.remove(&key) {
                    if let Some(k) = state.changes.get_mut(&id) {
                        if k.change.state == WatchState::Writing {
                            k.change.state = WatchState::NotSaved;
                            k.change.reason =
                                Some("the AI tool stopped before sending it".to_owned());
                            k.writing = None;
                            out.push(k.change.clone());
                        }
                    }
                }
            }
            state.previews.retain(|(s, _), _| s != session_id);
            out
        };
        for change in ended {
            self.tell(&WatchUpdate {
                change,
                writing: None,
            });
        }
    }

    /// Every change of `position_id`'s latest objective (each file's latest change), newest
    /// first; `None` when there is none in memory.
    pub fn view(&self, position_id: &str) -> Option<WatchView> {
        let state = self.state();
        let latest = state
            .order
            .iter()
            .rev()
            .filter_map(|id| state.changes.get(id))
            .find(|k| k.change.position_id.as_deref() == Some(position_id))?;
        let objective = latest.change.objective_task_id.clone();
        let mut seen = std::collections::HashSet::new();
        let changes = state
            .order
            .iter()
            .rev()
            .filter_map(|id| state.changes.get(id))
            .filter(|k| {
                k.change.position_id.as_deref() == Some(position_id)
                    && k.change.objective_task_id == objective
            })
            .filter(|k| seen.insert(k.change.path.clone()))
            .map(|k| k.change.clone())
            .collect();
        Some(WatchView {
            position_id: position_id.to_owned(),
            objective_task_id: Some(objective),
            changes,
            from_the_record: false,
        })
    }

    /// One change's file, its lines marked.
    pub fn file(&self, change_id: &str) -> Option<WatchFileView> {
        let state = self.state();
        let kept = state.changes.get(change_id)?;
        let (lines, removed_at_end) = kept
            .marked
            .as_ref()
            .map_or((Vec::new(), 0), |m| (m.lines.clone(), m.removed_at_end));
        Some(WatchFileView {
            change: kept.change.clone(),
            lines,
            removed_at_end,
            writing: kept.writing.clone(),
        })
    }

    /// The conversation a change was made in (tests).
    pub fn session_of(&self, change_id: &str) -> Option<String> {
        self.state()
            .changes
            .get(change_id)
            .map(|k| k.session_id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who() -> Who {
        Who {
            task_id: "t1".into(),
            session_id: "s1".into(),
            position_id: Some("p1".into()),
            worker: "Senior Developer".into(),
            objective_task_id: "root".into(),
        }
    }

    fn text(before: &str, after: &str) -> Written {
        Written {
            before: Before::Text(before.into()),
            after: after.into(),
        }
    }

    #[test]
    fn new_changed_and_removed_lines_are_marked() {
        let hub = WatchHub::default();
        let (change, counts) = hub.saved(
            &who(),
            "src/app.rs",
            &text(
                "one\ntwo\nthree\nfour\nfive\n",
                "one\nTWO\nthree\nfive\nsix\nseven\n",
            ),
        );
        assert_eq!(change.state, WatchState::Saved);
        assert_eq!(change.kind, Some(ChangeKind::Changed));
        assert_eq!((counts.added, counts.removed), (3, 2));
        let file = hub.file(&change.id).unwrap();
        let marks: Vec<(&str, Option<LineMark>, u32)> = file
            .lines
            .iter()
            .map(|l| (l.text.as_str(), l.mark, l.removed_before))
            .collect();
        assert_eq!(
            marks,
            vec![
                ("one", None, 0),
                ("TWO", Some(LineMark::Changed), 0),
                ("three", None, 0),
                ("five", None, 1),
                ("six", Some(LineMark::New), 0),
                ("seven", Some(LineMark::New), 0),
            ]
        );
        assert_eq!(file.removed_at_end, 0);
        // A new file: every line is new.
        let (created, _) = hub.saved(
            &who(),
            "src/new.rs",
            &Written {
                before: Before::Missing,
                after: "a\nb\n".into(),
            },
        );
        assert_eq!(created.kind, Some(ChangeKind::Created));
        assert!(hub
            .file(&created.id)
            .unwrap()
            .lines
            .iter()
            .all(|l| l.mark == Some(LineMark::New)));
        // Lines removed at the end.
        let (cut, _) = hub.saved(&who(), "src/app.rs", &text("a\nb\nc\n", "a\n"));
        assert_eq!(hub.file(&cut.id).unwrap().removed_at_end, 2);
    }

    #[test]
    fn a_large_or_non_text_file_shows_a_summary_not_its_contents() {
        let hub = WatchHub::default();
        let big = "x\n".repeat(MAX_SHOWN_LINES + 1);
        let (change, _) = hub.saved(&who(), "data.csv", &text("", &big));
        let summary = change.summary.clone().unwrap();
        assert!(summary.starts_with("Large file: "), "{summary}");
        assert!(summary.contains("lines added"), "{summary}");
        assert!(hub.file(&change.id).unwrap().lines.is_empty());
        let (binary, _) = hub.saved(
            &who(),
            "logo.png",
            &Written {
                before: Before::Unshown {
                    bytes: 34 * 1024,
                    binary: true,
                },
                after: "now text".into(),
            },
        );
        assert_eq!(binary.summary.as_deref(), Some("Not a text file (34 KB)"));
        assert!(hub.file(&binary.id).unwrap().lines.is_empty());
    }

    #[test]
    fn a_change_being_written_turns_saved_or_refused_and_never_both() {
        let hub = WatchHub::default();
        let seen: Arc<Mutex<Vec<WatchUpdate>>> = Arc::default();
        let log = Arc::clone(&seen);
        hub.set_listener(Arc::new(move |u| log.lock().unwrap().push(u.clone())));
        let w = who();
        let first = hub.writing(&w, "call-1", "src/a.rs", "fn main");
        let again = hub.writing(&w, "call-1", "src/a.rs", "fn main() {}\n");
        assert_eq!(first.id, again.id, "the same change, growing");
        assert_eq!(again.state, WatchState::Writing);
        assert_eq!(
            hub.file(&first.id).unwrap().writing.as_deref(),
            Some("fn main() {}\n")
        );
        let (saved, _) = hub.saved(&w, "src/a.rs", &text("", "fn main() {}\n"));
        assert_eq!(saved.id, first.id, "the preview became the saved change");
        assert_eq!(hub.file(&saved.id).unwrap().writing, None);

        let refused = hub.writing(&w, "call-2", "src/b.rs", "secret plans");
        let refusal = hub.refused(&w, "src/b.rs", "Blocked: src/b.rs is a blocked file");
        assert_eq!(refusal.id, refused.id);
        assert_eq!(refusal.state, WatchState::Refused);
        let file = hub.file(&refusal.id).unwrap();
        assert_eq!(file.writing, None, "a refused change's text is dropped");
        assert!(file.lines.is_empty());
        let states: Vec<(String, WatchState)> = seen
            .lock()
            .unwrap()
            .iter()
            .filter(|u| u.change.id == refused.id)
            .map(|u| (u.change.id.clone(), u.change.state))
            .collect();
        assert!(
            states.iter().all(|(_, s)| *s != WatchState::Saved),
            "{states:?}"
        );

        // A preview that is never sent turns "not saved" when its step ends.
        let lost = hub.writing(&w, "call-3", "src/c.rs", "half");
        hub.step_ended("s1");
        assert_eq!(
            hub.file(&lost.id).unwrap().change.state,
            WatchState::NotSaved
        );
    }

    #[test]
    fn the_view_lists_each_file_of_the_latest_objective_once() {
        let hub = WatchHub::default();
        let w = who();
        hub.saved(&w, "a.txt", &text("", "1\n"));
        hub.saved(&w, "b.txt", &text("", "1\n"));
        let (latest_a, _) = hub.saved(&w, "a.txt", &text("1\n", "2\n"));
        let view = hub.view("p1").unwrap();
        assert_eq!(view.objective_task_id.as_deref(), Some("root"));
        let paths: Vec<&str> = view.changes.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, vec!["a.txt", "b.txt"]);
        assert_eq!(view.changes[0].id, latest_a.id);
        // A newer objective replaces the list.
        let next = Who {
            objective_task_id: "root2".into(),
            ..who()
        };
        hub.saved(&next, "c.txt", &text("", "x\n"));
        let view = hub.view("p1").unwrap();
        assert_eq!(view.changes.len(), 1);
        assert!(hub.view("nobody").is_none());
    }
}
