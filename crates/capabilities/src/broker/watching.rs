//! Watch in the broker (Phase 18, ADR-055): who made each change, the check a change being
//! written passes before anything of it is shown, and what the Watch tab reads. A preview is
//! shown only for a file inside the worker's working copy that its permissions let it change or
//! ask about, never a blocked file or git's own files; secrets are hidden over the whole text.
//! Nothing here writes to disk.

use plenipo_guard::paths::blocked_by;
use plenipo_guard::{level_for, Capability, Level};
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

    /// Who a grant's changes belong to: its worker, task, conversation, and objective, and
    /// where its files are (its working copy, or its project's folder).
    pub(super) fn who(&self, grant_id: &str) -> Option<Who> {
        let (task_id, session_id, position_id, worker, root) = {
            let s = self.state();
            let g = s.grants.get(grant_id)?;
            (
                g.task_id.clone(),
                g.session_id.clone(),
                g.position_id.clone(),
                g.worker.clone(),
                super::owner_files::root_of(g),
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
            root,
        })
    }

    /// A change the AI tool of `grant_id` is still writing: shown only once its file is known
    /// and would be allowed (or asked about) for this worker; its text, secrets hidden, up to
    /// its last complete line (a line still being written could hold the start of a secret,
    /// which is recognised only once it is whole).
    pub(super) fn preview(&self, grant_id: &str, preview: WritePreview) {
        let Some(path) = preview.path.as_deref() else {
            // The file comes first; text that comes before it waits.
            return;
        };
        // Guard's settings as they are now (as Guard will check the change): when they cannot be
        // read, nothing of the text is shown.
        let config = self.inner.guard.config().ok();
        let (workspace, level) = {
            let s = self.state();
            let Some(g) = s.grants.get(grant_id) else {
                return;
            };
            let granted = g
                .levels
                .get(&Capability::FilesystemWrite)
                .copied()
                .unwrap_or_default();
            // The step's permission, and the settings now: the stricter decides.
            let now = config.as_ref().map_or(Level::Blocked, |c| {
                level_for(c, &g.scope, Capability::FilesystemWrite).level
            });
            if g.revoked || granted == Level::Blocked {
                // It cannot change files at all: there is nothing to show.
                return;
            }
            (g.workspace.clone(), granted.min(now))
        };
        // Resolved outside the broker's lock (it looks at the disk).
        let gate = workspace.as_ref().map(|ws| ws.resolve(path));
        let Some(who) = self.who(grant_id) else {
            return;
        };
        let hub = self.watch();
        let shown = |rel: &str| {
            level != Level::Blocked
                && config
                    .as_ref()
                    .is_some_and(|c| blocked_by(&c.blocked_files, rel).is_none())
        };
        match gate {
            Some(Ok(r)) if !r.in_git_dir() && shown(&r.rel) => {
                let text = if preview.done {
                    preview.text.as_str()
                } else {
                    preview
                        .text
                        .rfind('\n')
                        .map_or("", |end| &preview.text[..=end])
                };
                hub.writing(&who, &preview.call, &r.rel, &self.redact(text));
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
    /// runs; after a restart, from the Ledger's record of each saved change (without its
    /// lines), and both when the objective began before the restart.
    pub fn watch_view(&self, position_id: &str) -> WatchView {
        // Its own work and its team's (Phase 25, item 1.8): a lead's Watch shows the changes
        // of the workers it handed the work to.
        let team = self.watched_tasks(position_id);
        let memory = match &team {
            Some((objective, tasks)) => self.watch().view_of(position_id, objective, tasks),
            None => self.watch().view(position_id),
        };
        let record = self.watch_from_the_record(position_id, team.as_ref());
        let mut view = match (memory, record) {
            (Some(mut view), Some(record))
                if view.objective_task_id == record.objective_task_id =>
            {
                // The files changed before the restart that nothing changed since.
                let mut added = false;
                for change in record.changes {
                    if !view.changes.iter().any(|c| c.path == change.path) {
                        view.changes.push(change);
                        added = true;
                    }
                }
                view.changes.sort_by_key(|c| std::cmp::Reverse(c.at));
                view.from_the_record = added;
                view
            }
            (Some(view), _) => view,
            (None, Some(record)) => record,
            (None, None) => WatchView {
                position_id: position_id.to_owned(),
                ..WatchView::default()
            },
        };
        if let Some((_, tasks)) = &team {
            let mut ids: Vec<String> = tasks.iter().cloned().collect();
            ids.sort();
            view.team_task_ids = ids;
        }
        if view.changes.is_empty() {
            view.quiet = self.why_quiet(team.as_ref());
        }
        view
    }

    /// The objective `position_id` works on now, and the tasks in it whose changes its Watch
    /// shows: its own, and every task under them (the work it handed on to its team).
    fn watched_tasks(
        &self,
        position_id: &str,
    ) -> Option<(String, std::collections::HashSet<String>)> {
        let ledger = self.ledger();
        let task = ledger
            .position_tasks(position_id, 1)
            .ok()?
            .into_iter()
            .next()?;
        let objective = ledger.objective_of(&task.id).ok()?;
        let mut all: Vec<plenipo_ledger::Task> = Vec::new();
        if let Ok(Some(root)) = ledger.task(&objective) {
            all.push(root);
        }
        all.extend(
            ledger
                .descendant_tasks(&objective)
                .ok()?
                .into_iter()
                .map(|(t, _)| t),
        );
        let mut tasks = std::collections::HashSet::new();
        for own in all
            .iter()
            .filter(|t| t.metadata["workforce"]["positionId"].as_str() == Some(position_id))
        {
            tasks.insert(own.id.clone());
            if let Ok(under) = ledger.descendant_tasks(&own.id) {
                tasks.extend(under.into_iter().map(|(t, _)| t.id));
            }
        }
        Some((objective, tasks))
    }

    /// Why a Watch has nothing to show, when Plenipo knows: no work yet, or a worker that got no
    /// tools to change files with (Guard's own words, as recorded).
    fn why_quiet(
        &self,
        team: Option<&(String, std::collections::HashSet<String>)>,
    ) -> Option<String> {
        let Some((_, tasks)) = team else {
            return Some("It hasn't been given any work yet.".into());
        };
        let ledger = self.ledger();
        tasks
            .iter()
            .filter_map(|id| ledger.events_for_task(id).ok())
            .flatten()
            .filter(|e| e.event_type == "guard.grant_skipped")
            .max_by_key(|e| e.created_at)
            .and_then(|e| e.payload["reason"].as_str().map(str::to_owned))
    }

    fn watch_from_the_record(
        &self,
        position_id: &str,
        team: Option<&(String, std::collections::HashSet<String>)>,
    ) -> Option<WatchView> {
        let ledger = self.ledger();
        let task = ledger
            .position_tasks(position_id, 1)
            .ok()?
            .into_iter()
            .next()?;
        let objective = ledger.objective_of(&task.id).ok()?;
        // The objective's tasks that are this agent's own (its steps, or its workers' tasks), and
        // its team's.
        let mut tasks: Vec<plenipo_ledger::Task> = Vec::new();
        if let Ok(Some(root)) = ledger.task(&objective) {
            tasks.push(root);
        }
        tasks.extend(
            ledger
                .descendant_tasks(&objective)
                .ok()?
                .into_iter()
                .map(|(t, _)| t),
        );
        tasks.retain(|t| {
            t.metadata["workforce"]["positionId"].as_str() == Some(position_id)
                || team.is_some_and(|(_, team)| team.contains(&t.id))
        });
        let mut changes: Vec<WatchChange> = Vec::new();
        for task in tasks {
            let Ok(events) = ledger.events_for_task(&task.id) else {
                continue;
            };
            let session_id = task.metadata["sessionId"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_default();
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
                    task_id: task.id.clone(),
                    session_id: session_id.clone(),
                    position_id: Some(
                        task.metadata["workforce"]["positionId"]
                            .as_str()
                            .unwrap_or(position_id)
                            .to_owned(),
                    ),
                    worker: e.payload["worker"].as_str().unwrap_or("").to_owned(),
                    objective_task_id: objective.clone(),
                    path: path.to_owned(),
                    root: c["root"].as_str().map(str::to_owned),
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
            ..WatchView::default()
        })
    }

    /// One change's file, its lines marked (or a summary).
    pub fn watch_change(&self, change_id: &str) -> Option<WatchFileView> {
        self.watch().file(change_id)
    }
}

impl Broker {
    /// A change to `path` (inside the working copy, as Watch lists it) was refused, by Guard or
    /// not approved: its name and why, no text. `who` is taken when the call began, so a step
    /// that ended meanwhile still settles it.
    pub(super) fn watch_refused(&self, who: Option<&Who>, path: &str, why: &str) {
        if let Some(who) = who {
            self.watch().refused(who, path, &self.redact(why));
        }
    }

    /// A change to a file as the worker named it (not resolved: it may be outside the working
    /// copy) was refused.
    pub(super) fn watch_refused_named(&self, who: Option<&Who>, named: &str, why: &str) {
        if let Some(who) = who {
            self.watch()
                .refused(who, &self.redact(&shown_path(named)), &self.redact(why));
        }
    }

    /// A change to `path` waits for the owner's approval.
    pub(super) fn watch_waiting(&self, who: Option<&Who>, path: &str) {
        if let Some(who) = who {
            self.watch().waiting(who, path);
        }
    }

    /// A change to `path` could not be made.
    pub(super) fn watch_not_saved(&self, who: Option<&Who>, path: &str, why: &str) {
        if let Some(who) = who {
            self.watch().not_saved(who, path, &self.redact(why));
        }
    }

    /// Plenipo saved a change to `path`: Watch shows it, with secrets hidden in the file before
    /// and after; returns what the Ledger's record of the call keeps (the file, whether it was
    /// made, the lines added and removed when known, and its size — never its contents). Slow
    /// for a large file (secrets are looked for in all of it): call it off the async threads.
    pub(super) fn watch_saved(
        &self,
        who: Option<&Who>,
        path: &str,
        written: crate::watch::Written,
    ) -> Option<Value> {
        use crate::watch::{Before, Written};
        let who = who?;
        let before = match written.before {
            Before::Text(t) => Before::Text(self.redact(&t)),
            other => other,
        };
        let created = before == Before::Missing;
        let written = Written {
            before,
            after: self.redact(&written.after),
            after_bytes: written.after_bytes,
        };
        let (_, counts) = self.watch().saved(who, path, &written);
        let mut record = serde_json::json!({
            "path": path,
            "kind": if created { "created" } else { "changed" },
            "bytes": written.after_bytes,
        });
        if let Some(root) = &who.root {
            record["root"] = root.as_str().into();
        }
        if let Some(c) = counts {
            record["added"] = c.added.into();
            record["removed"] = c.removed.into();
        }
        Some(record)
    }
}
