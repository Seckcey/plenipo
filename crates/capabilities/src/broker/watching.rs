//! Watch in the broker (Phase 18, ADR-055): who made each change, the check a change being
//! written passes before anything of it is shown, and what the Watch tab reads. A preview is
//! shown only for a file inside the worker's working copy that its permissions let it change or
//! ask about, never a blocked file or git's own files; secrets are hidden over the whole text.
//! Nothing here writes to disk.

use plenipo_guard::paths::blocked_by;
use plenipo_guard::{Capability, Level};
use plenipo_runtime::agent::WritePreview;
use serde_json::Value;

use crate::watch::{ChangeKind, WatchChange, WatchFileView, WatchHub, WatchState, WatchView, Who};

use super::Broker;

/// The longest path shown for a change Plenipo will not show.
const MAX_SHOWN_PATH: usize = 120;

fn shown_path(path: &str) -> String {
    let path = path.trim();
    if path.chars().count() <= MAX_SHOWN_PATH {
        return path.to_owned();
    }
    let mut out: String = path.chars().take(MAX_SHOWN_PATH).collect();
    out.push('…');
    out
}

impl Broker {
    /// The changes Watch shows while Plenipo runs.
    pub fn watch(&self) -> &WatchHub {
        &self.inner.watch
    }

    /// Who a grant's changes belong to: its worker, task, conversation, and objective.
    pub(super) fn who(&self, grant_id: &str) -> Option<Who> {
        let (task_id, session_id, position_id, worker) = {
            let s = self.state();
            let g = s.grants.get(grant_id)?;
            (
                g.task_id.clone(),
                g.session_id.clone(),
                g.position_id.clone(),
                g.worker.clone(),
            )
        };
        let objective_task_id = self
            .ledger()
            .objective_of(&task_id)
            .unwrap_or_else(|_| task_id.clone());
        Some(Who {
            task_id,
            session_id,
            position_id,
            worker,
            objective_task_id,
        })
    }

    /// A change the AI tool of `grant_id` is still writing: shown only once its file is known
    /// and would be allowed (or asked about) for this worker; its text, secrets hidden.
    pub(super) fn preview(&self, grant_id: &str, preview: WritePreview) {
        let Some(path) = preview.path.as_deref() else {
            // The file comes first; text that comes before it waits.
            return;
        };
        let gate = {
            let s = self.state();
            let Some(g) = s.grants.get(grant_id) else {
                return;
            };
            let level = g
                .levels
                .get(&Capability::FilesystemWrite)
                .copied()
                .unwrap_or_default();
            if g.revoked || level == Level::Blocked {
                // It cannot change files at all: there is nothing to show.
                return;
            }
            g.workspace.as_ref().map(|ws| ws.resolve(path))
        };
        let Some(who) = self.who(grant_id) else {
            return;
        };
        let blocked = self
            .inner
            .guard
            .config()
            .map(|c| c.blocked_files)
            .unwrap_or_default();
        let hub = self.watch();
        match gate {
            Some(Ok(r)) if !r.in_git_dir() && blocked_by(&blocked, &r.rel).is_none() => {
                let text = self.redact(&preview.text);
                hub.writing(&who, &preview.call, &r.rel, &text);
            }
            Some(Ok(r)) => {
                hub.hidden(&who, &preview.call, &r.rel);
            }
            _ => {
                hub.hidden(&who, &preview.call, &self.redact(&shown_path(path)));
            }
        }
    }

    /// What `position_id`'s worker changed in its latest objective: from memory while Plenipo
    /// runs; after a restart, from the Ledger's record of each saved change (without its lines).
    pub fn watch_view(&self, position_id: &str) -> WatchView {
        if let Some(view) = self.watch().view(position_id) {
            return view;
        }
        self.watch_from_the_record(position_id)
            .unwrap_or(WatchView {
                position_id: position_id.to_owned(),
                ..WatchView::default()
            })
    }

    fn watch_from_the_record(&self, position_id: &str) -> Option<WatchView> {
        let ledger = self.ledger();
        let task = ledger
            .position_tasks(position_id, 1)
            .ok()?
            .into_iter()
            .next()?;
        let objective = ledger.objective_of(&task.id).ok()?;
        let mut tasks = vec![objective.clone()];
        tasks.extend(
            ledger
                .descendant_tasks(&objective)
                .ok()?
                .into_iter()
                .map(|(t, _)| t.id),
        );
        let mut changes: Vec<WatchChange> = Vec::new();
        for task_id in tasks {
            let Ok(events) = ledger.events_for_task(&task_id) else {
                continue;
            };
            for e in events
                .into_iter()
                .filter(|e| e.event_type == "capability.used")
            {
                let c = &e.payload["change"];
                let Some(path) = c["path"].as_str() else {
                    continue;
                };
                let n = |k: &str| {
                    c[k].as_u64()
                        .and_then(|v| u32::try_from(v).ok())
                        .unwrap_or(0)
                };
                changes.push(WatchChange {
                    id: e.id.clone(),
                    task_id: task_id.clone(),
                    position_id: Some(position_id.to_owned()),
                    worker: e.payload["worker"].as_str().unwrap_or("").to_owned(),
                    objective_task_id: objective.clone(),
                    path: path.to_owned(),
                    state: WatchState::Saved,
                    kind: Some(if c["kind"] == "created" {
                        ChangeKind::Created
                    } else {
                        ChangeKind::Changed
                    }),
                    added: n("added"),
                    removed: n("removed"),
                    reason: None,
                    summary: Some(
                        "The lines are shown only while Plenipo runs; the file is in the \
                         working copy."
                            .into(),
                    ),
                    at: e.created_at,
                });
            }
        }
        if changes.is_empty() {
            return None;
        }
        changes.sort_by_key(|c| std::cmp::Reverse(c.at));
        let mut seen = std::collections::HashSet::new();
        changes.retain(|c| seen.insert(c.path.clone()));
        Some(WatchView {
            position_id: position_id.to_owned(),
            objective_task_id: Some(objective),
            changes,
            from_the_record: true,
        })
    }

    /// One change's file, its lines marked (or a summary).
    pub fn watch_change(&self, change_id: &str) -> Option<WatchFileView> {
        self.watch().file(change_id)
    }
}

impl Broker {
    /// A change to `path` was refused (by Guard, or not approved): its name and why, no text.
    pub(super) fn watch_refused(&self, grant_id: &str, path: &str, why: &str) {
        if let Some(who) = self.who(grant_id) {
            self.watch()
                .refused(&who, &self.redact(&shown_path(path)), &self.redact(why));
        }
    }

    /// A change to `path` waits for the owner's approval.
    pub(super) fn watch_waiting(&self, grant_id: &str, path: &str) {
        if let Some(who) = self.who(grant_id) {
            self.watch().waiting(&who, path);
        }
    }

    /// A change to `path` could not be made.
    pub(super) fn watch_not_saved(&self, grant_id: &str, path: &str, why: &str) {
        if let Some(who) = self.who(grant_id) {
            self.watch().not_saved(&who, path, &self.redact(why));
        }
    }

    /// Plenipo saved a change to `path`: Watch shows it, with secrets hidden in the file before
    /// and after; returns what the Ledger's record of the call keeps (the file, whether it was
    /// made, and the lines added and removed — never its contents).
    pub(super) fn watch_saved(
        &self,
        grant_id: &str,
        path: &str,
        written: crate::watch::Written,
    ) -> Option<Value> {
        use crate::watch::{Before, Written};
        let who = self.who(grant_id)?;
        let before = match written.before {
            Before::Text(t) => Before::Text(self.redact(&t)),
            other => other,
        };
        let created = before == Before::Missing;
        let written = Written {
            before,
            after: self.redact(&written.after),
        };
        let (_, counts) = self.watch().saved(&who, path, &written);
        Some(serde_json::json!({
            "path": path,
            "kind": if created { "created" } else { "changed" },
            "added": counts.added,
            "removed": counts.removed,
            "bytes": written.after.len(),
        }))
    }
}
