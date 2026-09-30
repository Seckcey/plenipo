//! Paid AI keys (Phase 16 Wave 3, ADR-085, paid AI keys with spending caps).
//!
//! - [`Gate`] is what the agent runtime asks before a paid task: the key (from the Vault, only
//!   while "Let workers use paid AI keys" is on and the business's spending cap exists), and the
//!   spending caps (the most the task could cost set aside in the Ledger, and its bill recorded).
//! - [`save_key`] and [`remove_key`] keep a paid key: typed only into Plenipo's own screen,
//!   checked with one cheap read call, then kept only in the Vault (Windows Credential Manager),
//!   never shown again, and hidden from every log and record by the secret filter.
//! - [`helper`] is the program a paid task runs: Plenipo itself in its paid helper mode.

pub mod helper;

use std::sync::Arc;

use plenipo_guard::{PaidKeyInfo, PaidService};
use plenipo_ledger::{now_ms, Bill, PaidTask, PricedBy};
use plenipo_runtime::agent::paid::{PaidBill, PaidCharge, PaidGate, PaidKey};
use plenipo_runtime::agent::{AgentRuntime, AuthState};

use crate::broker::Broker;
use crate::vault;

/// The shortest and longest key Plenipo accepts (a real one is 40 to 200 characters).
const MIN_KEY: usize = 8;
const MAX_KEY: usize = 400;

/// What a paid task needs, from Guard's settings, the Ledger, and the Vault.
#[derive(Clone)]
pub struct Gate {
    broker: Broker,
}

impl Gate {
    pub fn new(broker: Broker) -> Self {
        Self { broker }
    }
}

/// Why no paid key can be used now, in plain words; none when it can.
pub fn not_allowed(broker: &Broker) -> Option<String> {
    match broker.guard().config() {
        Err(e) => Some(format!(
            "Plenipo could not read your permission settings, so no paid key is used ({e})."
        )),
        Ok(c) if !c.switches.paid_ai_keys => Some(
            "Paid AI keys are switched off: turn on Settings → Switches → Let workers use paid \
             AI keys."
                .into(),
        ),
        Ok(_) => match broker.guard().ledger().has_business_cap() {
            Ok(true) => None,
            Ok(false) => Some(
                "Paid AI keys need the business's monthly spending cap first (Settings → \
                 Spending caps)."
                    .into(),
            ),
            Err(e) => Some(format!(
                "Plenipo could not read your spending caps, so no paid key is used ({e})."
            )),
        },
    }
}

impl PaidGate for Gate {
    fn key(&self, runtime_id: &str) -> Result<PaidKey, String> {
        if let Some(why) = not_allowed(&self.broker) {
            return Err(why);
        }
        let config = self.broker.guard().config().map_err(|e| e.to_string())?;
        let info = config.paid_key(runtime_id).ok_or_else(|| {
            "No paid key is saved for this AI tool: add one on its card (Settings → AI tools)."
                .to_owned()
        })?;
        let value = vault::read(self.broker.secret_store().as_ref(), &info.id)
            .map_err(|e| format!("The saved key could not be read from the Vault ({e})."))?
            .ok_or_else(|| {
                "The saved key is missing from the Vault: add it again on its card (Settings → \
                 AI tools)."
                    .to_owned()
            })?;
        Ok(PaidKey::new(info.id.clone(), info.name.clone(), value))
    }

    fn set_aside(&self, charge: &PaidCharge) -> Result<String, String> {
        // Switched off, or the business's cap removed, since the check: nothing starts.
        if let Some(why) = not_allowed(&self.broker) {
            return Err(why);
        }
        let ledger = self.broker.guard().ledger();
        // The position doing the work, and the team it counts for (the team it is lent to while
        // on loan), from the task's own record.
        let workforce = ledger
            .task(&charge.task_id)
            .ok()
            .flatten()
            .map(|t| t.metadata["workforce"].clone())
            .unwrap_or_default();
        let position = workforce["positionId"]
            .as_str()
            .and_then(|id| ledger.position(id).ok().flatten().map(|p| (p.id, p.title)));
        let department = workforce["departmentId"]
            .as_str()
            .and_then(|id| ledger.department(id).ok().flatten().map(|d| (d.id, d.name)));
        let task = PaidTask {
            task_id: Some(charge.task_id.clone()),
            execution_id: None,
            position,
            department,
            runtime: charge.runtime_id.clone(),
            model: charge.model.clone(),
            key: Some((charge.key_id.clone(), charge.key_name.clone())),
            most_micros: charge.most_micros,
        };
        match ledger.set_aside_spending(&task, now_ms()) {
            Ok(Ok(set)) => Ok(set.record_id),
            Ok(Err(refusal)) => Err(refusal.reason),
            Err(e) => Err(format!(
                "The spending caps could not be checked, so the task did not start ({e})."
            )),
        }
    }

    fn settle(&self, ticket: &str, bill: &PaidBill) -> Vec<String> {
        let bill = match bill {
            PaidBill::Spent { micros, by_service } => Bill::Spent {
                micros: *micros,
                priced_by: if *by_service {
                    PricedBy::Service
                } else {
                    PricedBy::PriceList
                },
            },
            PaidBill::NotPriced(detail) => Bill::NotPriced {
                detail: detail.clone(),
            },
            PaidBill::NotSent => Bill::Released,
        };
        match self
            .broker
            .guard()
            .ledger()
            .settle_spending(ticket, &bill, now_ms())
        {
            Ok(settled) => settled.passed,
            Err(e) => {
                log::warn!("a paid task's spending could not be recorded: {e}");
                Vec::new()
            }
        }
    }
}

/// A key as typed: trimmed, 8 to 400 characters, no spaces or control characters inside.
fn clean_key(key: &str) -> Result<String, String> {
    let key = key.trim();
    if key.chars().count() < MIN_KEY || key.chars().count() > MAX_KEY {
        return Err(format!(
            "A key is {MIN_KEY} to {MAX_KEY} characters: paste the whole key, as the service \
             shows it."
        ));
    }
    if key.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("A key has no spaces or line breaks in it: paste it again.".into());
    }
    Ok(key.to_owned())
}

/// Save the paid key for `runtime_id` (replacing one saved before), typed only into Plenipo's
/// own screen. Refused while paid keys are off or the business's cap does not exist. The key is
/// put in the Vault, checked with one cheap read call, and kept only if the service accepts it;
/// otherwise nothing changes. Never returns or records the key.
pub async fn save_key(
    broker: &Broker,
    agents: &AgentRuntime,
    runtime_id: &str,
    name: &str,
    key: &str,
) -> Result<PaidKeyInfo, String> {
    let service = PaidService::from_id(runtime_id)
        .ok_or_else(|| "That AI tool takes no paid key.".to_owned())?;
    if let Some(why) = not_allowed(broker) {
        return Err(why);
    }
    let key = clean_key(key)?;
    let store = broker.secret_store();
    let id = plenipo_guard::paid::new_key_id();
    vault::put(store.as_ref(), &id, &key)
        .map_err(|e| format!("The key could not be kept in the Vault ({e}); nothing was saved."))?;
    let (info, previous) = match broker.guard().save_paid_key(runtime_id, name, &id) {
        Ok(saved) => saved,
        Err(e) => {
            let _ = vault::erase(store.as_ref(), &id);
            return Err(e.to_string());
        }
    };
    // Hidden from every log and record from now on, the check's included.
    broker.refresh_redactor();
    let (_, checked) = agents
        .recheck(runtime_id)
        .await
        .ok_or_else(|| format!("Plenipo has no AI tool called {runtime_id:?}"))?;
    if checked.auth.state == AuthState::PaidKey {
        if let Some(previous) = previous {
            let _ = vault::erase(store.as_ref(), &previous.id);
        }
        broker.refresh_redactor();
        return Ok(info);
    }
    // Refused, or not checked: put everything back as it was.
    let _ = broker.guard().remove_paid_key(runtime_id);
    if let Some(previous) = &previous {
        let _ = broker
            .guard()
            .save_paid_key(runtime_id, &previous.name, &previous.id);
    }
    let _ = vault::erase(store.as_ref(), &id);
    broker.refresh_redactor();
    let _ = agents.recheck(runtime_id).await;
    let detail = checked.auth.detail.unwrap_or_default();
    Err(if checked.auth.state == AuthState::SignedOut {
        format!(
            "{} did not accept the key, so it was not saved. {detail}",
            service.label()
        )
    } else {
        format!(
            "{} could not check the key, so it was not saved. {detail}",
            service.label()
        )
    }
    .trim()
    .to_owned())
}

/// Remove the paid key for `runtime_id`: its reference, and the key in the Vault.
pub async fn remove_key(
    broker: &Broker,
    agents: &AgentRuntime,
    runtime_id: &str,
) -> Result<PaidKeyInfo, String> {
    let info = broker
        .guard()
        .remove_paid_key(runtime_id)
        .map_err(|e| e.to_string())?;
    let _ = vault::erase(broker.secret_store().as_ref(), &info.id);
    broker.refresh_redactor();
    let _ = agents.recheck(runtime_id).await;
    Ok(info)
}

/// Whether any paid key is saved (the business's cap then stays, ADR-085 §2.4).
pub fn any_key(broker: &Broker) -> bool {
    broker
        .guard()
        .config()
        .map(|c| !c.paid_keys.is_empty())
        .unwrap_or(true)
}

/// The paid gate, for the agent runtime.
pub fn gate(broker: &Broker) -> Arc<dyn PaidGate> {
    Arc::new(Gate::new(broker.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_cleaned_and_checked_for_its_shape_only() {
        assert_eq!(
            clean_key("  sk-or-v1-abcdef0123456789  ").unwrap(),
            "sk-or-v1-abcdef0123456789"
        );
        for bad in ["", "short", "has a space in it", "line\nbreak-in-the-key"] {
            assert!(clean_key(bad).is_err(), "{bad:?}");
        }
        assert!(clean_key(&"k".repeat(MAX_KEY + 1)).is_err());
        // Refusals never repeat the key.
        let err = clean_key("sk-or-v1 secret part").unwrap_err();
        assert!(!err.contains("secret part"), "{err}");
    }
}
