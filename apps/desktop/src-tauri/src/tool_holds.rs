//! An AI tool's update and every organization (Phase 21, ADR-094 §4). The AI tools page (the
//! first organization's) updates a tool once for the PC and waits for the first organization's
//! workers; this holds every other organization's new work for that tool while the update runs:
//! at once where the tool is free, and in an organization whose workers are using it as soon as
//! they are done (the update itself does not wait for them; ADR-094, limits).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use plenipo_capabilities::ai_tools::{AiToolUpdateState, AiTools};
use plenipo_runtime::agent::{HoldFor, NotFree, RuntimeHold};
use tauri::{AppHandle, Manager as _, Runtime};

use crate::orgs::{self, OrgStack};

/// How often the updates going on are looked at.
const LOOK_EVERY: Duration = Duration::from_secs(1);

/// The holds on other organizations' AI tools while an update runs: (organization, tool) → hold.
#[derive(Default)]
pub struct UpdateHolds(Mutex<HashMap<(String, String), RuntimeHold>>);

impl UpdateHolds {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<(String, String), RuntimeHold>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Hold `runtime_id` in every organization but the first where it is free. `Err`: the
    /// names of the organizations whose workers are using it now (held once they are done);
    /// every other one is held.
    pub fn hold_all(&self, stacks: &[Arc<OrgStack>], runtime_id: &str) -> Result<(), Vec<String>> {
        let tries = stacks
            .iter()
            .filter(|s| !s.place.is_first())
            .map(|s| ((s.id().to_owned(), runtime_id.to_owned()), s))
            .filter(|(key, _)| !self.lock().contains_key(key))
            .map(|(key, s)| {
                (
                    key,
                    s.agents.hold_if_free(runtime_id, HoldFor::Update),
                    orgs::name_in(&s.ledger),
                )
            });
        let (made, busy) = sort_out(tries);
        let mut held = self.lock();
        for (key, hold) in made {
            held.insert(key, hold);
        }
        if busy.is_empty() {
            Ok(())
        } else {
            Err(busy)
        }
    }

    /// Keep the holds to the updates going on now: a new one (an update by itself) is held where
    /// the tool is free; a finished one lets every organization's work go on.
    fn follow(&self, stacks: &[Arc<OrgStack>], updating: &[String]) {
        self.lock().retain(|(org, tool), _| {
            updating.contains(tool) && stacks.iter().any(|s| s.id() == org)
        });
        for tool in updating {
            let _ = self.hold_all(stacks, tool);
        }
    }
}

/// Each organization's try to hold a tool: the holds made, and the names of the organizations
/// whose workers are using it. One busy organization never stops the others being held.
fn sort_out<K, H>(
    tries: impl IntoIterator<Item = (K, Result<H, NotFree>, String)>,
) -> (Vec<(K, H)>, Vec<String>) {
    let (mut made, mut busy) = (Vec::new(), Vec::new());
    for (key, tried, name) in tries {
        match tried {
            Ok(hold) => made.push((key, hold)),
            // Already held (its own sign-in tab): its work waits anyway.
            Err(NotFree::Held(_)) => {}
            Err(NotFree::Tasks(_)) => busy.push(name),
        }
    }
    (made, busy)
}

/// The AI tools being updated now (waiting, updating, or checking after).
fn updating(tools: &AiTools) -> Vec<String> {
    tools
        .page()
        .tools
        .into_iter()
        .filter(|t| {
            matches!(
                t.update.state,
                AiToolUpdateState::Waiting
                    | AiToolUpdateState::Updating
                    | AiToolUpdateState::Checking
            )
        })
        .map(|t| t.runtime_id)
        .collect()
}

/// Follow the updates going on, for as long as Plenipo runs.
pub fn watch<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("plenipo-update-holds".into())
        .spawn(move || loop {
            std::thread::sleep(LOOK_EVERY);
            let stacks = orgs::all_stacks(&app);
            let (Some(tools), Some(holds)) = (
                app.try_state::<AiTools>(),
                app.try_state::<Arc<UpdateHolds>>(),
            ) else {
                continue;
            };
            // One organization: the AI tools page holds its work itself.
            if stacks.len() <= 1 {
                holds.follow(&stacks, &[]);
                continue;
            }
            holds.follow(&stacks, &updating(&tools));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_organization_using_the_tool_never_stops_the_others_being_held() {
        let (made, busy) = sort_out([
            (
                "b",
                Err(NotFree::Tasks(vec!["t1".into()])),
                "Client Co".to_owned(),
            ),
            ("c", Ok(()), "Shop".to_owned()),
            ("d", Err(NotFree::Held(HoldFor::SignIn)), "Lab".to_owned()),
            ("e", Ok(()), "Studio".to_owned()),
        ]);
        assert_eq!(
            made.iter().map(|(k, ())| *k).collect::<Vec<_>>(),
            vec!["c", "e"]
        );
        assert_eq!(busy, vec!["Client Co".to_owned()]);
    }
}
