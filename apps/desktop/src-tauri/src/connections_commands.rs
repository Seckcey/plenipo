//! Phase 20 commands: Settings → Connections (ADR-062 to ADR-065) — connect a service in the
//! owner's own browser, cancel a sign-in, disconnect, and choose its parts, who may use it, the
//! list it may send to without asking, and (Advanced) the organization's own app. Every one is
//! the main window's alone (capabilities/default.json): the sign window and web pages are
//! refused.
//!
//! A sign-in happens on the service's own page in the owner's browser, and its token goes only
//! into the Vault. The secrets any of them take — the owner's own Google app's secret (ADR-070
//! §4), and the keys typed into the HubSpot, Stripe, and website cards (ADR-071 §1) — go straight
//! to the Vault and never come back out. Part 20C adds add-on tools (ADR-066): a program the owner
//! installed, added off, its tools looked at and marked, and who may use it. Guard decides each
//! action and records it (never a token, a key, or a secret, never the account's address).

use std::collections::BTreeMap;

use plenipo_capabilities::connections::keyed::{KeyInput, MAX_KEY};
use plenipo_capabilities::connections::{AppInput, ConnectionsPage, MAX_APP_SECRET};
use plenipo_capabilities::Broker;
use plenipo_core::CommandError;
use plenipo_guard::add_ons::{MAX_ARGS, MAX_ARG_CHARS, MAX_SECRETS, MAX_TOOLS};
use plenipo_guard::connections::{service_of, MAX_ACCESS, MAX_SEND_LIST};
use plenipo_guard::{
    Access, AccountKind, AddOnChange, AddOnInput, OwnApp, Part, PartLevel, Service, ToolMark, Who,
};
use tauri::State;

use crate::commands::{bounded, bounded_optional, validate_id, with_broker};

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

/// Disconnect: its tools stop at once, its sign-in leaves the Vault, and the service cancels it
/// where it can (Slack, Google). Always allowed.
#[tauri::command]
pub async fn disconnect_connection(
    broker: State<'_, Broker>,
    connection_id: String,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    let broker = broker.inner().clone();
    broker
        .disconnect_connection(&connection_id)
        .await
        .map_err(crate::commands::broker_error)
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
        if let Some(t) = &a.tenant {
            bounded("the organization", t)?;
        }
    }
    with_broker(&broker, move |b| {
        b.set_connection_own_app(&connection_id, app.as_ref())
    })
    .await
}

/// The owner's own Slack app (its client ID) or Google app (its client ID and secret), or none
/// (`null`). A Google app's secret goes straight to the Vault; nothing returns it.
#[tauri::command]
pub async fn save_connection_app(
    broker: State<'_, Broker>,
    connection_id: String,
    app: Option<AppInput>,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    if let Some(a) = &app {
        bounded("the client ID", &a.client_id)?;
        if a.secret.as_ref().is_some_and(|s| s.len() > MAX_APP_SECRET) {
            return Err(CommandError::invalid_input("that secret is too long"));
        }
    }
    with_broker(&broker, move |b| {
        b.save_connection_app(&connection_id, app.as_ref())
    })
    .await
}

/// Add another account of a service that may have more than one (a Slack workspace).
#[tauri::command]
pub async fn add_connection(
    broker: State<'_, Broker>,
    service: Service,
) -> Result<ConnectionsPage, CommandError> {
    with_broker(&broker, move |b| b.add_connection(service)).await
}

/// Remove a card that is not connected (a Slack workspace).
#[tauri::command]
pub async fn remove_connection(
    broker: State<'_, Broker>,
    connection_id: String,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    with_broker(&broker, move |b| b.remove_connection(&connection_id)).await
}

/// HubSpot's, Stripe's, or the website's key, typed into its card (ADR-071 §1): checked with one
/// reading call, kept only in the Vault if the service accepts it, and never returned.
#[tauri::command]
pub async fn save_connection_key(
    broker: State<'_, Broker>,
    connection_id: String,
    key: KeyInput,
) -> Result<ConnectionsPage, CommandError> {
    validate_connection_id(&connection_id)?;
    for v in [&key.key, &key.password, &key.store_key, &key.store_secret]
        .into_iter()
        .flatten()
    {
        if v.chars().count() > MAX_KEY {
            return Err(CommandError::invalid_input("that key is too long"));
        }
    }
    bounded_optional("the site's address", key.site.as_deref())?;
    bounded_optional("the user name", key.user.as_deref())?;
    let broker = broker.inner().clone();
    broker
        .save_connection_key(&connection_id, &key)
        .await
        .map_err(crate::commands::broker_error)
}

/// An add-on's ID: small letters and digits, 1–14.
fn validate_add_on_id(id: &str) -> Result<(), CommandError> {
    let ok = (1..=14).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    if ok {
        Ok(())
    } else {
        Err(CommandError::invalid_input("invalid add-on id"))
    }
}

fn check_program_fields(
    program: Option<&str>,
    args: Option<&[String]>,
    secrets: Option<&[String]>,
) -> Result<(), CommandError> {
    bounded_optional("the program", program)?;
    if let Some(args) = args {
        if args.len() > MAX_ARGS {
            return Err(CommandError::invalid_input(format!(
                "at most {MAX_ARGS} arguments"
            )));
        }
        if args.iter().any(|a| a.chars().count() > MAX_ARG_CHARS) {
            return Err(CommandError::invalid_input("an argument is too long"));
        }
    }
    if let Some(secrets) = secrets {
        if secrets.len() > MAX_SECRETS {
            return Err(CommandError::invalid_input(format!(
                "at most {MAX_SECRETS} stored secrets"
            )));
        }
        secrets
            .iter()
            .try_for_each(|n| bounded("a secret's name", n))?;
    }
    Ok(())
}

/// Add a program that offers tools (ADR-066 §1): off, with no tool marked and nobody allowed.
/// Shells and programs that download code each time they start are refused.
#[tauri::command]
pub async fn add_add_on(
    broker: State<'_, Broker>,
    add_on: AddOnInput,
) -> Result<ConnectionsPage, CommandError> {
    bounded("the name", &add_on.name)?;
    check_program_fields(
        Some(&add_on.program),
        Some(&add_on.args),
        Some(&add_on.secrets),
    )?;
    with_broker(&broker, move |b| b.add_add_on(&add_on)).await
}

/// Change an add-on: its name, program, arguments, secrets, on or off (switching it on looks at
/// its tools first), or who may use it.
#[tauri::command]
pub async fn change_add_on(
    broker: State<'_, Broker>,
    add_on_id: String,
    change: AddOnChange,
) -> Result<ConnectionsPage, CommandError> {
    validate_add_on_id(&add_on_id)?;
    bounded_optional("the name", change.name.as_deref())?;
    check_program_fields(
        change.program.as_deref(),
        change.args.as_deref(),
        change.secrets.as_deref(),
    )?;
    if let Some(access) = &change.access {
        if access.len() > MAX_ACCESS {
            return Err(CommandError::invalid_input(format!(
                "at most {MAX_ACCESS} lines on Who may use it"
            )));
        }
        for a in access {
            match &a.who {
                Who::Role { id } => validate_id("role", id)?,
                Who::Agent { id } => validate_id("agent", id)?,
            }
        }
    }
    let broker = broker.inner().clone();
    broker
        .change_add_on(&add_on_id, &change)
        .await
        .map_err(crate::commands::broker_error)
}

/// Remove an add-on.
#[tauri::command]
pub async fn remove_add_on(
    broker: State<'_, Broker>,
    add_on_id: String,
) -> Result<ConnectionsPage, CommandError> {
    validate_add_on_id(&add_on_id)?;
    with_broker(&broker, move |b| b.remove_add_on(&add_on_id)).await
}

/// Start the program once, list its tools, and stop it (ADR-066 §2): new or changed tools are
/// Off.
#[tauri::command]
pub async fn check_add_on_tools(
    broker: State<'_, Broker>,
    add_on_id: String,
) -> Result<ConnectionsPage, CommandError> {
    validate_add_on_id(&add_on_id)?;
    let broker = broker.inner().clone();
    broker
        .check_add_on_tools(&add_on_id)
        .await
        .map_err(crate::commands::broker_error)
}

/// Mark an add-on's tools Off, Reading (goes ahead), or Changing (asks every time).
#[tauri::command]
pub async fn set_add_on_tools(
    broker: State<'_, Broker>,
    add_on_id: String,
    marks: BTreeMap<String, ToolMark>,
) -> Result<ConnectionsPage, CommandError> {
    validate_add_on_id(&add_on_id)?;
    if marks.len() > MAX_TOOLS {
        return Err(CommandError::invalid_input(format!(
            "at most {MAX_TOOLS} tools"
        )));
    }
    marks.keys().try_for_each(|n| bounded("a tool's name", n))?;
    with_broker(&broker, move |b| b.set_add_on_tools(&add_on_id, &marks)).await
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
        assert_eq!(commands.len(), 17, "{commands:?}");
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
