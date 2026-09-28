//! Phase 20 commands: Settings → Connections (ADR-062 to ADR-065) — connect a service in the
//! owner's own browser, cancel a sign-in, disconnect, and choose its parts, who may use it, the
//! list it may send to without asking, and (Advanced) the organization's own app. Every one is
//! the main window's alone (capabilities/default.json): the sign window and web pages are
//! refused.
//!
//! None takes a password, a key, a token, a program, or an address: a sign-in happens on the
//! service's own page in the owner's browser, and its token goes only into the Vault. Guard
//! decides each action and records it (never the token, never the account's address).

use std::collections::BTreeMap;

use plenipo_capabilities::connections::ConnectionsPage;
use plenipo_capabilities::Broker;
use plenipo_core::CommandError;
use plenipo_guard::connections::{service_of, MAX_ACCESS, MAX_SEND_LIST};
use plenipo_guard::{Access, AccountKind, OwnApp, Part, PartLevel, Who};
use tauri::State;

use crate::commands::{bounded, validate_id, with_broker};

/// The longest connection ID (`microsoft365`, `slack-12`).
const MAX_CONNECTION_ID: usize = 40;

/// A connection's ID: a service Plenipo knows, and nothing else.
fn validate_connection_id(id: &str) -> Result<(), CommandError> {
    let shaped = !id.is_empty()
        && id.len() <= MAX_CONNECTION_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !shaped || service_of(id).is_none() {
        return Err(CommandError::invalid_input("invalid connection id"));
    }
    Ok(())
}

/// Everything Settings → Connections shows: each service, its connection's state, parts, who
/// may use it, and its list — never a sign-in.
#[tauri::command]
pub async fn get_connections(broker: State<'_, Broker>) -> Result<ConnectionsPage, CommandError> {
    with_broker(&broker, Broker::connections_page).await
}

/// Connect: the service's sign-in page opens in the owner's own browser, and Plenipo waits for
/// it in the background (the page shows it waiting).
#[tauri::command]
pub async fn connect_connection(
    broker: State<'_, Broker>,
    connection_id: String,
    kind: AccountKind,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    let broker = broker.inner().clone();
    broker
        .connect_connection(&connection_id, kind)
        .await
        .map_err(crate::commands::broker_error)
}

/// Stop a sign-in still waiting in the owner's browser.
#[tauri::command]
pub async fn cancel_connection_sign_in(
    broker: State<'_, Broker>,
    connection_id: String,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    with_broker(&broker, move |b| {
        b.cancel_connection_sign_in(&connection_id)
    })
    .await
}

/// Disconnect: its tools stop at once, and its sign-in leaves the Vault. Always allowed.
#[tauri::command]
pub async fn disconnect_connection(
    broker: State<'_, Broker>,
    connection_id: String,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    with_broker(&broker, move |b| b.disconnect_connection(&connection_id)).await
}

/// Each part: Off, Read only, or Full access.
#[tauri::command]
pub async fn set_connection_parts(
    broker: State<'_, Broker>,
    connection_id: String,
    parts: BTreeMap<Part, PartLevel>,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    with_broker(&broker, move |b| {
        b.set_connection_parts(&connection_id, &parts)
    })
    .await
}

/// **Who may use it**: roles and agents, each at Read only or Read and write.
#[tauri::command]
pub async fn set_connection_access(
    broker: State<'_, Broker>,
    connection_id: String,
    access: Vec<Access>,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    if access.len() > MAX_ACCESS {
        return Err(CommandError::invalid_input(format!(
            "at most {MAX_ACCESS} lines on Who may use it"
        )));
    }
    for a in &access {
        match &a.who {
            Who::Role { id } => validate_id("role", id)?,
            Who::Agent { id } => validate_id("agent", id)?,
        }
    }
    with_broker(&broker, move |b| {
        b.set_connection_access(&connection_id, &access)
    })
    .await
}

/// **Send without asking to**: addresses, `@domains`, and channels (used only while the switch
/// "Sending forms and messages (without asking)" is on).
#[tauri::command]
pub async fn set_connection_send_list(
    broker: State<'_, Broker>,
    connection_id: String,
    list: Vec<String>,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    if list.len() > MAX_SEND_LIST {
        return Err(CommandError::invalid_input(format!(
            "at most {MAX_SEND_LIST} entries on Send without asking to"
        )));
    }
    list.iter().try_for_each(|e| bounded("an entry", e))?;
    with_broker(&broker, move |b| {
        b.set_connection_send_list(&connection_id, &list)
    })
    .await
}

/// Advanced: sign in with the organization's own app (its app ID and domain; neither is a
/// secret), or with 8 West's (`None`). Only while not connected.
#[tauri::command]
pub async fn set_connection_own_app(
    broker: State<'_, Broker>,
    connection_id: String,
    app: Option<OwnApp>,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    if let Some(a) = &app {
        bounded("the app ID", &a.app_id)?;
        bounded("the organization", &a.tenant)?;
    }
    with_broker(&broker, move |b| {
        b.set_connection_own_app(&connection_id, app.as_ref())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tauri runs a command that is not `async` on the window's own thread; these read the
    /// Ledger and the Vault, which can be slow, so none may freeze the window.
    #[test]
    fn every_connections_command_runs_off_the_windows_thread() {
        let source = include_str!("connections_commands.rs");
        let commands: Vec<&str> = source
            .split("#[tauri::command]\n")
            .skip(1)
            .map(|rest| rest.lines().next().unwrap_or_default())
            .collect();
        assert_eq!(commands.len(), 8, "{commands:?}");
        for line in commands {
            assert!(line.starts_with("pub async fn "), "not async: {line}");
        }
    }

    #[test]
    fn only_known_services_are_connection_ids() {
        for ok in ["microsoft365", "slack", "slack-2", "google", "stripe"] {
            assert!(validate_connection_id(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "Microsoft365",
            "../microsoft365",
            "microsoft365 ",
            "outlook",
            "slack-0",
            "slack-1",
            "slack-x",
            &"a".repeat(41),
        ] {
            assert!(validate_connection_id(bad).is_err(), "{bad:?}");
        }
    }
}
