//! Paid AI keys (Phase 16 Wave 3, ADR-085, paid AI keys with spending caps).
//!
//! - [`Gate`] is what the agent runtime asks before a paid task: the key (from the Vault, only
//!   while "Let workers use paid AI keys" is on), and the spending caps (the most the task could
//!   cost set aside in the Ledger, and its bill recorded). A cap is the owner's choice, never
//!   needed (2026-09-30): with none, paid work has no dollar limit and is still recorded.
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
        // No cap is needed, but caps the owner set must be readable to be kept: unreadable,
        // nothing paid is offered (the Router moves on to the next route) and nothing starts.
        Ok(_) => match broker.guard().ledger().has_business_cap() {
            Ok(_) => None,
            Err(e) => Some(format!(
                "Plenipo could not read your spending caps, so no paid key is used ({e})."
            )),
        },
    }
}

impl PaidGate for Gate {
    fn key(&self, runtime_id: &str) -> Result<PaidKey, String> {
        // This organization's switch and caps; the key itself is the PC's, kept with the first
        // organization (Phase 25, item 1.4).
        if let Some(why) = not_allowed(&self.broker) {
            return Err(why);
        }
        let keeper = self.broker.paid_key_keeper();
        let config = keeper.guard().config().map_err(|e| e.to_string())?;
        let info = config.paid_key(runtime_id).ok_or_else(|| {
            "No paid key is saved for this AI tool: add one on its card on the AI tools page."
                .to_owned()
        })?;
        let value = vault::read(keeper.secret_store().as_ref(), &info.id)
            .map_err(|e| format!("The saved key could not be read from the Vault ({e})."))?
            .ok_or_else(|| {
                "The saved key is missing from the Vault: add it again on its card on the AI tools \
                 page."
                    .to_owned()
            })?;
        Ok(PaidKey::new(info.id.clone(), info.name.clone(), value))
    }

    fn set_aside(&self, charge: &PaidCharge) -> Result<String, String> {
        // Switched off since the check: nothing starts.
        if let Some(why) = not_allowed(&self.broker) {
            return Err(why);
        }
        // The service's own address, checked by Guard here too (a refusal is recorded), before
        // the helper checks it again.
        if let Some(service) = PaidService::from_id(&charge.runtime_id) {
            let address = format!("{}{}", service.base_url(), service.chat_path());
            self.broker
                .guard()
                .check_outbound(
                    &plenipo_guard::outbound::OutboundRules::default(),
                    plenipo_guard::outbound::Purpose::PaidAi(service),
                    &address,
                )
                .map_err(|why| {
                    format!("Guard stopped the request to {}: {why}", service.label())
                })?;
        }
        let ledger = self.broker.guard().ledger();
        let unread = |e: plenipo_ledger::LedgerError| {
            format!("The task's record could not be read, so the task did not start ({e}).")
        };
        // The position doing the work, and the team it counts for (the team it is lent to while
        // on loan), from the task's own record. A record that cannot be read stops the task:
        // the department's and the position's caps would not count otherwise.
        let workforce = ledger
            .task(&charge.task_id)
            .map_err(unread)?
            .map(|t| t.metadata["workforce"].clone())
            .unwrap_or_default();
        let position = match workforce["positionId"].as_str() {
            Some(id) => ledger
                .position(id)
                .map_err(unread)?
                .map(|p| (p.id, p.title)),
            None => None,
        };
        let department = match workforce["departmentId"].as_str() {
            Some(id) => ledger
                .department(id)
                .map_err(unread)?
                .map(|d| (d.id, d.name)),
            None => None,
        };
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
        // A busy Ledger gets two more tries: a bill left set aside counts only at what was set
        // aside after a restart, even when the service billed more.
        let ledger = self.broker.guard().ledger();
        let mut tries = 0;
        loop {
            match ledger.settle_spending(ticket, &bill, now_ms()) {
                Ok(settled) => return settled.passed,
                Err(e) if tries < 2 => {
                    tries += 1;
                    log::warn!("a paid task's spending could not be recorded yet: {e}");
                    std::thread::sleep(std::time::Duration::from_millis(200 * tries));
                }
                Err(e) => {
                    log::warn!("a paid task's spending could not be recorded: {e}");
                    return Vec::new();
                }
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

/// Saving and removing keys happen one at a time, so a rollback never undoes another save.
static KEY_CHANGES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Erase `id` from the Vault; a failure is logged (the caller keeps a reference to it, or says
/// so).
fn erase_logged(store: &dyn vault::SecretStore, id: &str) -> bool {
    match vault::erase(store, id) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("a paid key could not be erased from the Vault: {e}");
            false
        }
    }
}

/// Save the paid key for `runtime_id` (replacing one saved before), typed only into Plenipo's
/// own screen. Refused while paid keys are off in `allowed_by` (the organization the owner is
/// looking at; Phase 25, item 1.4), whose `agents` check it; no spending cap is needed. `broker`
/// keeps it: the first organization's, for the whole PC. The key is
/// put in the Vault, checked with one cheap read call, and kept only if the service accepts it;
/// otherwise nothing changes. Never returns or records the key. A key is never left in the Vault
/// with nothing pointing to it: the one being replaced stays listed with the new one until the
/// new one's check passes.
pub async fn save_key(
    broker: &Broker,
    allowed_by: &Broker,
    agents: &AgentRuntime,
    runtime_id: &str,
    name: &str,
    key: &str,
) -> Result<PaidKeyInfo, String> {
    let service = PaidService::from_id(runtime_id)
        .ok_or_else(|| "That AI tool takes no paid key.".to_owned())?;
    if let Some(why) = not_allowed(allowed_by) {
        return Err(why);
    }
    let key = clean_key(key)?;
    // A name that is the key itself (or any key the secret filter knows the look of) would be
    // kept in the settings and the Ledger's record.
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 60 || trimmed.chars().any(char::is_control) {
        return Err("A key's name is one line of 1 to 60 characters.".into());
    }
    let looks_secret = broker.text_filter().redact(trimmed) != trimmed;
    if looks_secret
        || (trimmed.chars().count() >= MIN_KEY && (key.contains(trimmed) || trimmed.contains(&key)))
    {
        return Err(
            "That name looks like the key itself: give the key a name such as \"Office key\"."
                .into(),
        );
    }
    let _one_at_a_time = KEY_CHANGES.lock().await;
    let store = broker.secret_store();
    let label = store.label();
    // A key a save left behind (Plenipo stopped during its check) goes first.
    if let Some(left) = broker
        .guard()
        .config()
        .ok()
        .and_then(|c| c.paid_key(runtime_id).and_then(|k| k.replaces.clone()))
    {
        vault::erase(store.as_ref(), &left).map_err(|e| {
            format!("An earlier key could not be removed from {label} ({e}); nothing was saved.")
        })?;
    }
    let id = plenipo_guard::paid::new_key_id();
    vault::put(store.as_ref(), &id, &key)
        .map_err(|e| format!("The key could not be kept in {label} ({e}); nothing was saved."))?;
    let (info, previous) = match broker.guard().save_paid_key(runtime_id, name, &id) {
        Ok(saved) => saved,
        Err(e) => {
            erase_logged(store.as_ref(), &id);
            return Err(e.to_string());
        }
    };
    // Hidden from every log and record from now on, the check's included.
    broker.refresh_redactor();
    allowed_by.refresh_redactor();
    let checked = agents.recheck(runtime_id).await.map(|(_, info)| info);
    if checked
        .as_ref()
        .is_some_and(|c| c.auth.state == AuthState::PaidKey)
    {
        // The key it replaces goes now; if the Vault will not let it go, it stays listed with
        // the new key, for the next save or Remove.
        let erased = previous
            .as_ref()
            .is_none_or(|p| erase_logged(store.as_ref(), &p.id));
        if erased {
            let _ = broker.guard().confirm_paid_key(runtime_id, &id);
        }
        broker.refresh_redactor();
        allowed_by.refresh_redactor();
        return Ok(info);
    }
    // Refused, or not checked: put everything back as it was (unless another save has since).
    let _ = broker.guard().restore_paid_key(
        runtime_id,
        &id,
        previous.map(|p| PaidKeyInfo {
            replaces: None,
            ..p
        }),
    );
    erase_logged(store.as_ref(), &id);
    broker.refresh_redactor();
    allowed_by.refresh_redactor();
    let _ = agents.recheck(runtime_id).await;
    let Some(checked) = checked else {
        return Err(format!("Plenipo has no AI tool called {runtime_id:?}"));
    };
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

/// Remove the paid key for `runtime_id`: the key in the Vault first (and any key it was
/// replacing), then its reference, so a key the Vault would not let go stays listed, for Remove
/// to try again.
pub async fn remove_key(
    broker: &Broker,
    agents: &AgentRuntime,
    runtime_id: &str,
) -> Result<PaidKeyInfo, String> {
    let _one_at_a_time = KEY_CHANGES.lock().await;
    let store = broker.secret_store();
    let saved = broker
        .guard()
        .config()
        .map_err(|e| e.to_string())?
        .paid_key(runtime_id)
        .cloned()
        .ok_or_else(|| "That AI tool has no paid key.".to_owned())?;
    for id in saved.vault_ids() {
        vault::erase(store.as_ref(), id).map_err(|e| {
            format!(
                "The key could not be removed from {} ({e}); it is still saved. Try again.",
                store.label()
            )
        })?;
    }
    let info = broker
        .guard()
        .remove_paid_key(runtime_id)
        .map_err(|e| e.to_string())?;
    broker.refresh_redactor();
    let _ = agents.recheck(runtime_id).await;
    Ok(info)
}

/// Whether any paid key is saved (the PC's keys, wherever they are kept).
pub fn any_key(broker: &Broker) -> bool {
    broker
        .paid_key_keeper()
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
