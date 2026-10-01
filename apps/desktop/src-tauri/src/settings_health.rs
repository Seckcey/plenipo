//! Damaged settings (Phase 13, "corrupted config"). Plenipo keeps its settings in the Ledger.
//! When a settings document cannot be read, Plenipo does not guess: Guard refuses every tool
//! until it can read the permissions again (it fails closed), and the window says so, with two
//! ways out — restore a backup of the Ledger from Diagnostics, or reset those settings to their
//! starting values (after a backup of the Ledger, so nothing is lost).

use plenipo_core::SettingsProblem;
use plenipo_guard::{Guard, GuardConfig};
use plenipo_ledger::{BackupKind, Ledger};
use plenipo_router::config::RoutingConfig;
use plenipo_router::Router;
use serde_json::json;

/// The settings a problem can be about.
pub const GUARD: &str = plenipo_guard::SETTING;
pub const ROUTING: &str = plenipo_router::SETTING;
pub const SPENDING: &str = plenipo_ledger::spending::SETTING;

/// The settings that cannot be read now.
pub fn problems(guard: &Guard, router: &Router) -> Vec<SettingsProblem> {
    let mut out = Vec::new();
    // The spending caps (Phase 16 Wave 3, ADR-085): unreadable, no paid task starts (a cap is
    // never needed, but caps the owner set must be readable to be kept).
    if let Err(e) = guard.ledger().has_business_cap() {
        log::error!("the spending caps cannot be read: {e}");
        out.push(SettingsProblem {
            key: SPENDING.into(),
            label: "Spending caps".into(),
            message: "Plenipo could not read your spending caps, so no paid AI key can be used \
                      until this is fixed. What was spent is kept either way. Restore a backup of \
                      the Ledger from Diagnostics, or reset spending caps: that removes every cap \
                      and switches paid AI keys off, so set your caps again before you turn them \
                      back on."
                .into(),
        });
    }
    if let Err(e) = guard.config() {
        log::error!("the permission settings cannot be read: {e}");
        out.push(SettingsProblem {
            key: GUARD.into(),
            label: "Permissions".into(),
            message: "Plenipo could not read your permission settings, so no worker can use any \
                      tool until this is fixed. Restore a backup of the Ledger from Diagnostics, \
                      or reset permissions to their starting settings."
                .into(),
        });
    }
    if let Err(e) = router.config() {
        log::error!("the AI model settings cannot be read: {e}");
        out.push(SettingsProblem {
            key: ROUTING.into(),
            label: "AI models".into(),
            message: "Plenipo could not read your AI model settings, so it cannot choose a model \
                      for new work. Restore a backup of the Ledger from Diagnostics, or reset AI \
                      models to their starting settings."
                .into(),
        });
    }
    out
}

/// Reset one damaged settings document to its starting values, after backing up the Ledger
/// (the damaged document is kept in that backup). Returns the backup's name.
pub fn reset(ledger: &Ledger, guard: &Guard, key: &str) -> Result<Option<String>, String> {
    let fresh = match key {
        GUARD => GuardConfig::with_defaults().to_value(),
        ROUTING => serde_json::to_value(RoutingConfig::default()).map_err(|e| e.to_string())?,
        // No caps, and (below) paid AI keys switched off: with no cap, paid work would have no
        // dollar limit, so nothing paid runs until the owner sets caps again and turns paid keys
        // back on. What was spent stays in the spending records, which a reset never touches.
        SPENDING => json!({}),
        _ => return Err("Those are not settings Plenipo can reset.".into()),
    };
    let backup = if ledger.path().is_some() {
        let info = ledger
            .backup_of_kind(BackupKind::Manual, None)
            .map_err(|e| {
                format!("The Ledger could not be backed up first ({e}); nothing was reset.")
            })?;
        crate::backup_host::record(ledger, BackupKind::Manual, &info, "owner");
        std::path::Path::new(&info.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    } else {
        None
    };
    let event = match key {
        GUARD => "guard.settings_reset",
        SPENDING => "spending.settings_reset",
        _ => "routing.settings_reset",
    };
    ledger
        .update_setting(key, event, "owner", |_damaged| {
            Ok((fresh, json!({ "backup": backup })))
        })
        .map_err(|e| e.to_string())?;
    if key == GUARD {
        guard
            .seed_template_roles()
            .map_err(|e| format!("The built-in roles did not get their permissions back ({e})."))?;
    }
    if key == SPENDING {
        // Unreadable permission settings already stop every paid task (Guard fails closed).
        if let Ok(config) = guard.config() {
            if config.switches.paid_ai_keys {
                let switches = plenipo_guard::dto::Switches {
                    paid_ai_keys: false,
                    ..config.switches.clone()
                };
                guard.set_switches(&switches).map_err(|e| {
                    format!(
                        "The spending caps were reset, but paid AI keys could not be switched \
                         off ({e}): turn off Settings → Switches → Let workers use paid AI keys."
                    )
                })?;
            }
        }
    }
    log::warn!("the {key} settings were reset to their starting values");
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn damaged_permissions_are_reported_and_can_be_reset_after_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Arc::new(Ledger::open(&dir.path().join("ledger").join("plenipo.db")).unwrap());
        let guard = Guard::new(ledger.clone());
        let router = Router::with_tools(ledger.clone(), Arc::new(Vec::new));
        assert!(problems(&guard, &router).is_empty());
        // The permission settings get damaged (a shape Guard cannot read).
        ledger
            .put_setting(GUARD, &json!({ "sets": "not a list" }), "test")
            .unwrap();
        let found = problems(&guard, &router);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].key, GUARD);
        assert!(found[0].message.contains("no worker can use any tool"));
        // Reset: a backup first, then the starting settings.
        let backup = reset(&ledger, &guard, GUARD).unwrap().unwrap();
        assert!(backup.starts_with("plenipo-backup-"));
        assert!(problems(&guard, &router).is_empty());
        assert!(guard.config().is_ok());
        assert!(!ledger
            .events_of_types(&["guard.settings_reset"], 5)
            .unwrap()
            .is_empty());
        // The damaged document is in the backup.
        let kept = Ledger::open(&ledger.backups_dir().unwrap().join(&backup)).unwrap();
        assert_eq!(kept.setting(GUARD).unwrap().unwrap()["sets"], "not a list");
        // Unknown settings are refused.
        assert!(reset(&ledger, &guard, "organization").is_err());
    }

    #[test]
    fn damaged_spending_caps_are_reported_and_reset_to_none() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Arc::new(Ledger::open(&dir.path().join("ledger").join("plenipo.db")).unwrap());
        let guard = Guard::new(ledger.clone());
        let router = Router::with_tools(ledger.clone(), Arc::new(Vec::new));
        let now = plenipo_ledger::now_ms();
        ledger
            .set_spending_cap(
                &plenipo_ledger::CapCovers::Business,
                50_000_000,
                "owner",
                now,
            )
            .unwrap();
        // A later version's cap kind (a downgrade), or a damaged setting.
        ledger
            .put_setting(
                SPENDING,
                &json!({ "caps": [{ "covers": { "kind": "everyone" } }] }),
                "test",
            )
            .unwrap();
        let found = problems(&guard, &router);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].key, SPENDING);
        assert!(found[0].message.contains("no paid AI key can be used"));
        assert!(found[0].message.contains("switches paid AI keys off"));
        // Reset: no caps, and paid AI keys switched off, so nothing paid runs with no limit.
        guard
            .set_switches(&plenipo_guard::dto::Switches {
                paid_ai_keys: true,
                ..plenipo_guard::dto::Switches::default()
            })
            .unwrap();
        reset(&ledger, &guard, SPENDING).unwrap();
        assert!(problems(&guard, &router).is_empty());
        assert!(!ledger.has_business_cap().unwrap());
        assert!(!guard.config().unwrap().switches.paid_ai_keys);
        assert!(!ledger
            .events_of_types(&["spending.settings_reset"], 5)
            .unwrap()
            .is_empty());
    }
}
