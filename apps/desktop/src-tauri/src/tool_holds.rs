//! An AI tool's update and every organization (Phase 21, ADR-094 §4). The AI tools page (the
//! first organization's) updates a tool once for the PC and waits for the first organization's
//! workers; this holds every other organization's new work for that tool while the update runs,
//! and refuses an update you start while another organization's workers are using the tool.

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

    /// Hold `runtime_id` in every organization but the first. `Err`: the name of an
    /// organization whose workers are using it now (nothing is held then).
    pub fn hold_all(&self, stacks: &[Arc<OrgStack>], runtime_id: &str) -> Result<(), String> {
        let mut made = Vec::new();
        for s in stacks.iter().filter(|s| !s.place.is_first()) {
            let key = (s.id().to_owned(), runtime_id.to_owned());
            if self.lock().contains_key(&key) {
                continue;
            }
            match s.agents.hold_if_free(runtime_id, HoldFor::Update) {
                Ok(hold) => made.push((key, hold)),
                // Already held (its own sign-in tab): its work waits anyway.
                Err(NotFree::Held(_)) => {}
                Err(NotFree::Tasks(_)) => return Err(orgs::name_in(&s.ledger)),
            }
        }
        let mut held = self.lock();
        for (key, hold) in made {
            held.insert(key, hold);
        }
        Ok(())
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
