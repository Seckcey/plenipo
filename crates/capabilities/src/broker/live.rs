//! The canvas's live view (Phase 18, ADR-053 §17–§19): for each worker in a step now, where its
//! work runs besides its AI company's cloud (a program, Plenipo's browser, or the screen on this
//! PC; a server by name), and what it touched last in its task (a folder in its working copy, a
//! server, a website, or the screen) — from Guard's own record of its calls. Hand-offs come
//! from Liaison's records. Nothing is guessed or stored.

use std::path::Path;

use crate::control::ControlKind;
use crate::dto::{LiveHandoff, LivePlace, LiveView, LiveWorker, Touching};

use super::{Broker, Prepared};

/// How long a touch counts as "now".
pub const TOUCHED_FOR_MS: u64 = 2 * 60 * 1000;
/// How far back hand-offs are shown.
pub const HANDOFFS_FOR_MS: u64 = 5 * 60 * 1000;
/// How many tasks' last touch are remembered at once.
const REMEMBERED: usize = 500;

fn now() -> u64 {
    plenipo_ledger::now_ms()
}

/// What a call Guard let through touched.
pub(super) fn touched_by(prepared: &Prepared, project: Option<&str>) -> Option<Touching> {
    if let Some(s) = &prepared.server {
        return Some(Touching::Server {
            name: s.server.name.clone(),
            production: s.server.environment == plenipo_guard::Environment::Production,
        });
    }
    if let Some(site) = &prepared.site {
        return Some(Touching::Website {
            host: site.host.clone(),
        });
    }
    if matches!(
        prepared.capability,
        plenipo_guard::Capability::ComputerObserve | plenipo_guard::Capability::ComputerControl
    ) {
        return Some(Touching::Screen);
    }
    let file = prepared.files.first()?;
    let folder = Path::new(&file.rel)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    Some(Touching::Folder {
        project: project.map(str::to_owned),
        folder,
    })
}

/// Where a step's work runs now, besides its AI company's cloud: a server it is connected to
/// (by name), the screen or Plenipo's browser it controls, or a program it runs on this PC.
pub(super) fn runs_on(
    servers: &[(String, bool)],
    controlling: &[ControlKind],
    running_program: bool,
) -> Option<LivePlace> {
    let here = |what: &str| {
        Some(LivePlace::ThisPc {
            what: what.to_owned(),
        })
    };
    if let Some((name, production)) = servers.first() {
        Some(LivePlace::Server {
            name: name.clone(),
            production: *production,
        })
    } else if controlling.contains(&ControlKind::Desktop) {
        here("the screen")
    } else if controlling.contains(&ControlKind::Browser) {
        here("Plenipo's browser")
    } else if running_program {
        here("a program")
    } else {
        None
    }
}

impl Broker {
    /// Remember what task `task_id` touched now.
    pub(super) fn note_touched(&self, task_id: &str, touching: Touching) {
        let mut state = self.state();
        let at = now();
        if state.touched.len() >= REMEMBERED && !state.touched.contains_key(task_id) {
            if let Some(oldest) = state
                .touched
                .iter()
                .min_by_key(|(_, (_, at))| *at)
                .map(|(k, _)| k.clone())
            {
                state.touched.remove(&oldest);
            }
        }
        state.touched.insert(task_id.to_owned(), (touching, at));
    }

    /// Who is working where now, and the hand-offs of the last few minutes.
    pub fn live_view(&self) -> LiveView {
        let at = now();
        let sessions = self.control_status().sessions;
        let workers = {
            let state = self.state();
            let mut workers: Vec<LiveWorker> = state
                .grants
                .values()
                .filter(|g| !g.revoked)
                .map(|g| {
                    let kinds: Vec<ControlKind> = sessions
                        .iter()
                        .filter(|s| s.grant_id == g.id)
                        .map(|s| s.kind)
                        .collect();
                    let runs_on = runs_on(&g.ssh.connected(), &kinds, !g.running.is_empty());
                    let touching = state
                        .touched
                        .get(&g.task_id)
                        .filter(|(_, when)| at.saturating_sub(*when) <= TOUCHED_FOR_MS)
                        .map(|(t, _)| t.clone());
                    LiveWorker {
                        grant_id: g.id.clone(),
                        task_id: g.task_id.clone(),
                        position_id: g.position_id.clone(),
                        worker: g.worker.clone(),
                        runtime_id: g.runtime_id.clone(),
                        runs_on,
                        touching,
                    }
                })
                .collect();
            workers.sort_by(|a, b| a.worker.cmp(&b.worker).then(a.grant_id.cmp(&b.grant_id)));
            workers
        };
        let handoffs = self
            .ledger()
            .recent_handoffs(at.saturating_sub(HANDOFFS_FOR_MS), 100)
            .unwrap_or_default()
            .into_iter()
            .filter(|h| h.from_position_id.is_some() || h.to_position_id.is_some())
            .filter_map(|h| {
                let kind = match (h.kind.as_str(), h.state.as_str()) {
                    ("request", "accepted" | "dispatched") => "asked",
                    ("reply", _) => "answered",
                    _ => return None,
                };
                Some(LiveHandoff {
                    id: h.id,
                    kind: kind.into(),
                    from_position_id: h.from_position_id,
                    to_position_id: h.to_position_id,
                    at: h.at,
                })
            })
            .collect();
        LiveView {
            workers,
            handoffs,
            at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn here(what: &str) -> Option<LivePlace> {
        Some(LivePlace::ThisPc { what: what.into() })
    }

    #[test]
    fn the_live_view_says_where_the_work_runs_in_each_case() {
        let shop = vec![("Shop".to_owned(), true)];
        assert_eq!(
            runs_on(&shop, &[ControlKind::Server], true),
            Some(LivePlace::Server {
                name: "Shop".into(),
                production: true
            }),
            "a server, by name, before anything on this PC"
        );
        assert_eq!(
            runs_on(&[], &[ControlKind::Desktop, ControlKind::Browser], true),
            here("the screen")
        );
        assert_eq!(
            runs_on(&[], &[ControlKind::Browser], true),
            here("Plenipo's browser")
        );
        assert_eq!(runs_on(&[], &[], true), here("a program"));
        assert_eq!(
            runs_on(&[], &[], false),
            None,
            "only its AI company's cloud"
        );
    }
}
