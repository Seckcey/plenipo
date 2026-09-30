//! Phase 21 commands: your organizations (ADR-094) — the list, a new one (from a template, a
//! copy of another one's setup, or from scratch), showing one in this window or a new one, and
//! archiving, bringing back, and deleting one for good. Organizations' windows only
//! (`capabilities/default.json`); a pop-out, the sign, and web pages cannot call them.

use std::sync::Arc;

use plenipo_core::{
    CommandError, OrgDeletePreview, OrgListing, OrgOpened, OrgStart, OrgSummary, OrgWorker,
};
use plenipo_ledger::Ledger;
use tauri::{AppHandle, Manager as _, Runtime, State, WebviewWindow};

use crate::commands::ledger_error;
use crate::org_host::{self, Defaults, Opening};
use crate::orgs::{self, OrgEntry, OrgPlace, OrgStack, Orgs, FIRST, MAX_NAME};
use crate::workspace_windows::{self, is_org_window, MAIN};

/// Recorded as the one who acted.
const OWNER: &str = "owner";
/// How long an organization's work gets to stop the normal way when it is archived.
const GRACE: std::time::Duration = std::time::Duration::from_secs(5);

/// The calling window, when it is an organization's window.
fn org_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<String, CommandError> {
    let label = window.label();
    if is_org_window(label) {
        Ok(label.to_owned())
    } else {
        Err(CommandError::invalid_input(
            "only an organization's window can do that",
        ))
    }
}

fn checked_id(id: &str) -> Result<(), CommandError> {
    if orgs::is_org_id(id) {
        Ok(())
    } else {
        Err(CommandError::invalid_input(
            "That is not one of your organizations.",
        ))
    }
}

fn entry(orgs: &Orgs, id: &str) -> Result<OrgEntry, CommandError> {
    checked_id(id)?;
    orgs.entry(id)
        .ok_or_else(|| CommandError::invalid_input("That is not one of your organizations."))
}

fn first(orgs: &Orgs) -> Result<Arc<OrgStack>, CommandError> {
    orgs.first()
        .ok_or_else(|| CommandError::internal("the first organization is not open"))
}

/// Where organization `id` lives in this run.
fn place<R: Runtime>(app: &AppHandle<R>, id: &str) -> OrgPlace {
    let defaults = app.state::<Defaults>();
    let data = match defaults.persistence {
        crate::runtime_host::Persistence::AppData => app.path().app_local_data_dir().ok(),
        crate::runtime_host::Persistence::InMemory => None,
    };
    OrgPlace {
        id: id.to_owned(),
        folder: data.as_ref().map(|d| orgs::folder_of(d, id)),
        data,
        vault: orgs::vault_name(&app.config().identifier, id),
    }
}

/// Open organization `place` (new, or brought back), with `ledger` when it is already open.
fn open<R: Runtime>(
    app: &AppHandle<R>,
    orgs: &Orgs,
    place: OrgPlace,
    ledger: Option<Arc<Ledger>>,
) -> Result<Arc<OrgStack>, CommandError> {
    let first = first(orgs)?;
    let defaults = *app.state::<Defaults>();
    let version = app.package_info().version.to_string();
    let ledger = ledger.unwrap_or_else(|| {
        org_host::open_ledger(app, &place, defaults.persistence, &version, |_| {}).ledger
    });
    let stack = org_host::build(
        app,
        place,
        ledger,
        &Opening {
            persistence: defaults.persistence,
            notices: defaults.notices,
            gather: defaults.gather,
            version: &version,
            previous: &crate::recovery::PreviousEnd::Clean,
            run: defaults.run,
            control: orgs.control(),
            first: Some(&first),
        },
    );
    orgs.insert(stack.clone());
    orgs::filter_logs(orgs);
    Ok(stack)
}

/// The list, as the window `label` sees it.
pub fn listing(orgs: &Orgs, label: &str) -> OrgListing {
    let current = orgs
        .org_of_window(label)
        .unwrap_or_else(|| FIRST.to_owned());
    let mut organizations: Vec<OrgSummary> = orgs
        .entries()
        .into_iter()
        .map(|e| {
            let stack = orgs.stack(&e.id);
            OrgSummary {
                first: e.id == FIRST,
                archived: e.archived_at.is_some(),
                here: e.id == current,
                in_window: orgs.window_of(&e.id).is_some(),
                working: stack
                    .as_ref()
                    .map_or(0, |s| u32::try_from(s.working()).unwrap_or(u32::MAX)),
                name: stack
                    .as_ref()
                    .map_or_else(|| e.name.clone(), |s| orgs::name_in(&s.ledger)),
                created_at: e.created_at,
                id: e.id,
            }
        })
        .collect();
    organizations.sort_by_key(|o| (!o.first, o.created_at));
    OrgListing {
        current,
        organizations,
        // A place for templates: none yet (ADR-091 §5).
        templates: Vec::new(),
    }
}

/// Your organizations, and which one this window shows.
#[tauri::command]
pub fn get_organizations<R: Runtime>(
    window: WebviewWindow<R>,
    orgs: State<'_, Arc<Orgs>>,
) -> Result<OrgListing, CommandError> {
    let label = org_window(&window)?;
    // A name changed in Settings → Organization: the list follows.
    for s in orgs.stacks() {
        orgs.set_name(s.id(), &orgs::name_in(&s.ledger));
    }
    Ok(listing(&orgs, &label))
}

/// Make a new organization (ADR-094 §15).
#[tauri::command]
pub async fn create_organization<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    orgs: State<'_, Arc<Orgs>>,
    name: String,
    start: OrgStart,
) -> Result<OrgSummary, CommandError> {
    let label = org_window(&window)?;
    let name = plenipo_ledger::workforce::clean_line("the organization's name", &name, MAX_NAME)
        .map_err(ledger_error)?;
    let source = match &start {
        OrgStart::Template { .. } => {
            return Err(CommandError::invalid_input("Templates are coming later."));
        }
        OrgStart::Copy { from } => {
            checked_id(from)?;
            Some(orgs.stack(from).ok_or_else(|| {
                CommandError::invalid_input(
                    "Copy from an organization that is open (bring it back first).",
                )
            })?)
        }
        OrgStart::Scratch => None,
    };
    let orgs = orgs.inner().clone();
    let orgs2 = orgs.clone();
    let app2 = app.clone();
    let id = orgs::new_id();
    let made = id.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), CommandError> {
        let orgs = orgs2;
        let place = place(&app2, &made);
        if let Some(folder) = &place.folder {
            std::fs::create_dir_all(folder)
                .and_then(|()| plenipo_ledger::owner_only::folder(folder))
                .map_err(|e| {
                    CommandError::internal(format!("its folder could not be made: {e}"))
                })?;
        }
        let defaults = *app2.state::<Defaults>();
        let version = app2.package_info().version.to_string();
        let ledger =
            org_host::open_ledger(&app2, &place, defaults.persistence, &version, |_| {}).ledger;
        // Its setup, copied before anything starts on it (ADR-094 §15).
        if let Some(source) = &source {
            ledger
                .copy_setup_from(&source.ledger, &orgs::name_in(&source.ledger), OWNER)
                .map_err(ledger_error)?;
        }
        let stack = open(&app2, &orgs, place, Some(ledger))?;
        stack
            .workforce
            .rename(&name)
            .map_err(crate::commands::workforce_error)?;
        orgs.add(OrgEntry {
            id: made.clone(),
            name: name.clone(),
            created_at: plenipo_ledger::now_ms(),
            archived_at: None,
        });
        // Recorded in the first organization's Ledger too: it keeps the list's history.
        if let Some(first) = orgs.first() {
            let _ = first.ledger.append_event(plenipo_ledger::NewEvent {
                source: OWNER.into(),
                event_type: "organization.created".into(),
                payload: serde_json::json!({ "id": made, "name": name, "start": start }),
                ..plenipo_ledger::NewEvent::default()
            });
        }
        Ok(())
    })
    .await
    .map_err(|e| CommandError::internal(format!("making the organization failed: {e}")))??;
    orgs::list_changed(&app);
    listing(&orgs, &label)
        .organizations
        .into_iter()
        .find(|o| o.id == id)
        .ok_or_else(|| CommandError::internal("the new organization is not in the list"))
}

/// Show organization `id` in this window: the page loads again, and this window's terminals and
/// pop-outs close (ADR-094 §11). Brings its window to the front when another window shows it.
#[tauri::command]
pub fn switch_organization<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    orgs: State<'_, Arc<Orgs>>,
    id: String,
) -> Result<OrgOpened, CommandError> {
    let label = org_window(&window)?;
    let e = entry(&orgs, &id)?;
    if e.archived_at.is_some() || orgs.stack(&id).is_none() {
        return Err(CommandError::invalid_input(
            "That organization is archived: bring it back first.",
        ));
    }
    if orgs.org_of_window(&label).as_deref() == Some(id.as_str()) {
        return Ok(OrgOpened::Focused);
    }
    if let Some(other) = orgs.window_of(&id) {
        focus(&app, &other);
        return Ok(OrgOpened::Focused);
    }
    leave(
        &app,
        &orgs,
        &label,
        "you showed another organization in its window",
    );
    orgs.bind(&label, &id);
    set_title(&window, &orgs, &id);
    window
        .reload()
        .map_err(|e| CommandError::internal(format!("the window could not load it: {e}")))?;
    orgs::list_changed(&app);
    Ok(OrgOpened::Switching)
}

/// Open organization `id` in a window of its own (or bring its window to the front).
#[tauri::command]
pub fn open_organization_window<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    orgs: State<'_, Arc<Orgs>>,
    id: String,
) -> Result<OrgOpened, CommandError> {
    org_window(&window)?;
    let e = entry(&orgs, &id)?;
    if e.archived_at.is_some() || orgs.stack(&id).is_none() {
        return Err(CommandError::invalid_input(
            "That organization is archived: bring it back first.",
        ));
    }
    if let Some(other) = orgs.window_of(&id) {
        focus(&app, &other);
        return Ok(OrgOpened::Focused);
    }
    let label = orgs::window_label(&id);
    orgs.bind(&label, &id);
    match workspace_windows::build_org_window(&app, &label, true) {
        Ok(new) => {
            set_title(&new, &orgs, &id);
            let _ = new.set_focus();
        }
        Err(e) => {
            orgs.unbind(&label);
            return Err(CommandError::internal(format!(
                "its window could not open: {e}"
            )));
        }
    }
    orgs::list_changed(&app);
    Ok(OrgOpened::Opened)
}

/// Stop an organization's work and hide it from the list (ADR-094 §17).
#[tauri::command]
pub async fn archive_organization<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    orgs: State<'_, Arc<Orgs>>,
    id: String,
) -> Result<OrgListing, CommandError> {
    let label = org_window(&window)?;
    let e = entry(&orgs, &id)?;
    if id == FIRST {
        return Err(CommandError::invalid_input(
            "Your first organization keeps your Workforce and your tile, so it stays.",
        ));
    }
    if e.archived_at.is_some() {
        return Ok(listing(&orgs, &label));
    }
    let Some(stack) = orgs.stack(&id) else {
        return Err(CommandError::internal("that organization is not open"));
    };
    if stack.busy() {
        return Err(CommandError::invalid_input(format!(
            "{} has work going. Let it finish, or stop it, then archive it.",
            e.name
        )));
    }
    // Its window closes; the first window shows the first organization again.
    if let Some(shown) = orgs.window_of(&id) {
        if shown == MAIN {
            leave(&app, &orgs, MAIN, "its organization was archived");
            orgs.bind(MAIN, FIRST);
            if let Some(main) = app.get_webview_window(MAIN) {
                set_title(&main, &orgs, FIRST);
                let _ = main.reload();
            }
        } else {
            leave(&app, &orgs, &shown, "its organization was archived");
            orgs.unbind(&shown);
            if let Some(w) = app.get_webview_window(&shown) {
                let _ = w.destroy();
            }
        }
    }
    orgs.take(&id);
    orgs.set_archived(&id, Some(plenipo_ledger::now_ms()));
    org_host::stop(&stack, "its organization was archived", GRACE).await;
    let _ = stack.ledger.append_event(plenipo_ledger::NewEvent {
        source: OWNER.into(),
        event_type: "organization.archived".into(),
        payload: serde_json::json!({ "id": id }),
        ..plenipo_ledger::NewEvent::default()
    });
    orgs.park(stack);
    orgs::filter_logs(&orgs);
    orgs::list_changed(&app);
    Ok(listing(&orgs, &label))
}

/// Open an archived organization again.
#[tauri::command]
pub async fn bring_back_organization<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    orgs: State<'_, Arc<Orgs>>,
    id: String,
) -> Result<OrgListing, CommandError> {
    let label = org_window(&window)?;
    let e = entry(&orgs, &id)?;
    if e.archived_at.is_none() {
        return Ok(listing(&orgs, &label));
    }
    let orgs2 = orgs.inner().clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), CommandError> {
        // Kept in memory only (the tests): its Ledger is the one it had.
        let parked = orgs2.unpark(&id);
        let ledger = parked.and_then(|p| p.place.folder.is_none().then(|| p.ledger.clone()));
        let stack = open(&app2, &orgs2, place(&app2, &id), ledger)?;
        orgs2.set_archived(&id, None);
        let _ = stack.ledger.append_event(plenipo_ledger::NewEvent {
            source: OWNER.into(),
            event_type: "organization.brought_back".into(),
            payload: serde_json::json!({ "id": id }),
            ..plenipo_ledger::NewEvent::default()
        });
        Ok(())
    })
    .await
    .map_err(|e| CommandError::internal(format!("opening it failed: {e}")))??;
    orgs::list_changed(&app);
    Ok(listing(&orgs, &label))
}

/// An archived organization's services, to read it or save its workers before it is deleted:
/// the ones kept since it was archived, or opened now without running anything.
fn archived<R: Runtime>(
    app: &AppHandle<R>,
    orgs: &Orgs,
    id: &str,
) -> Result<Arc<OrgStack>, CommandError> {
    let e = entry(orgs, id)?;
    if e.archived_at.is_none() {
        return Err(CommandError::invalid_input(
            "Archive the organization first; then it can be deleted for good.",
        ));
    }
    if let Some(parked) = orgs.parked(id) {
        return Ok(parked);
    }
    let first = first(orgs)?;
    let defaults = *app.state::<Defaults>();
    let version = app.package_info().version.to_string();
    let place = place(app, id);
    let ledger = org_host::open_ledger(app, &place, defaults.persistence, &version, |_| {}).ledger;
    let stack = org_host::build(
        app,
        place,
        ledger,
        &Opening {
            persistence: defaults.persistence,
            notices: crate::notices::Output::Kept,
            gather: defaults.gather,
            version: &version,
            previous: &crate::recovery::PreviousEnd::Clean,
            run: false,
            control: orgs.control(),
            first: Some(&first),
        },
    );
    orgs.park(stack.clone());
    Ok(stack)
}

/// Its workers with experience, and which of them your first organization can hire.
fn experienced(
    stack: &OrgStack,
    first: &Ledger,
) -> Result<(Vec<OrgWorker>, Vec<OrgWorker>), CommandError> {
    let records = stack.ledger.org_records().map_err(ledger_error)?;
    let counts = stack.ledger.experience_counts().map_err(ledger_error)?;
    let first_roles: std::collections::HashSet<String> = first
        .list_roles()
        .map_err(ledger_error)?
        .into_iter()
        .map(|r| r.name)
        .collect();
    let role_name = |id: &str| {
        records
            .roles
            .iter()
            .find(|r| r.id == id)
            .map_or_else(String::new, |r| r.name.clone())
    };
    let (mut can, mut cannot) = (Vec::new(), Vec::new());
    for p in records.positions.iter().filter(|p| p.deleted_at.is_none()) {
        // Its work here, and any it brought back from your Workforce.
        let here = counts.get(&p.id);
        let carried = &p.metadata["experience"];
        let count = |v: &serde_json::Value| {
            v.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or_default()
        };
        let tasks_done = here.map_or(0, |c| c.tasks_done) + count(&carried["tasksDone"]);
        let kept_lessons = here.map_or(0, |c| c.kept_lessons) + count(&carried["keptLessons"]);
        if tasks_done == 0 && kept_lessons == 0 {
            continue;
        }
        let role = role_name(&p.role_id);
        let worker = OrgWorker {
            position_id: p.id.clone(),
            title: p.title.clone(),
            tasks_done,
            kept_lessons,
            role_name: role.clone(),
        };
        if first_roles.contains(&role) {
            can.push(worker);
        } else {
            cannot.push(worker);
        }
    }
    // Agents in its own Workforce (their role is not your first organization's), after the
    // ones that can move have moved: they go with it.
    for saved in stack.ledger.saved_agents().map_err(ledger_error)? {
        let count = |key: &str| {
            saved.experience[key]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or_default()
        };
        cannot.push(OrgWorker {
            position_id: saved.id.clone(),
            title: saved.title.clone(),
            tasks_done: count("tasksDone"),
            kept_lessons: count("keptLessons"),
            role_name: role_name(&saved.role_id),
        });
    }
    Ok((can, cannot))
}

/// What deleting an archived organization for good removes, and whom it can save.
#[tauri::command]
pub async fn preview_delete_organization<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    orgs: State<'_, Arc<Orgs>>,
    id: String,
) -> Result<OrgDeletePreview, CommandError> {
    org_window(&window)?;
    let orgs = orgs.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let stack = archived(&app, &orgs, &id)?;
        // Agents it kept in its own Workforce go to yours, where they can.
        stack.workforce.send_saved_home();
        let (experienced, not_saved) = experienced(&stack, &first(&orgs)?.ledger)?;
        Ok(OrgDeletePreview {
            name: orgs::name_in(&stack.ledger),
            id,
            experienced,
            not_saved,
        })
    })
    .await
    .map_err(|e| CommandError::internal(format!("reading it failed: {e}")))?
}

/// Delete an archived organization for good (ADR-094 §18): the workers in `save` move to your
/// Workforce first; then its folder and its secrets go.
#[tauri::command]
pub async fn delete_organization_for_good<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    orgs: State<'_, Arc<Orgs>>,
    id: String,
    save: Vec<String>,
) -> Result<OrgListing, CommandError> {
    let label = org_window(&window)?;
    if id == FIRST {
        return Err(CommandError::invalid_input(
            "Your first organization keeps your Workforce and your tile, so it stays.",
        ));
    }
    if save.len() > 500 {
        return Err(CommandError::invalid_input(
            "too many workers to save at once",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let save: Vec<String> = save
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect();
    let orgs2 = orgs.inner().clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), CommandError> {
        let stack = archived(&app2, &orgs2, &id)?;
        let first = first(&orgs2)?;
        let name = orgs::name_in(&stack.ledger);
        stack.workforce.send_saved_home();
        let (can, _) = experienced(&stack, &first.ledger)?;
        let mut saved = Vec::new();
        for position in &save {
            if !can.iter().any(|w| &w.position_id == position) {
                return Err(CommandError::invalid_input(
                    "Only its experienced workers can be saved to your Workforce.",
                ));
            }
        }
        for position in &save {
            if let Some(s) = stack
                .workforce
                .save_copy_to(&id, position, &first.ledger)
                .map_err(crate::commands::workforce_error)?
            {
                saved.push(serde_json::json!({ "savedId": s.id, "title": s.title }));
            }
        }
        // Its secrets, forgotten in the Vault.
        let guard = stack.ledger.setting(plenipo_guard::SETTING).ok().flatten();
        let folder = stack.place.folder.clone();
        let vault = stack.place.vault.clone();
        orgs2.unpark(&id);
        drop(stack);
        if let (Some(settings), Some(_)) = (guard, &folder) {
            let ids = plenipo_capabilities::vault::stored_ids_in(&settings);
            let store = plenipo_capabilities::OsSecretStore::new(vault);
            let (_, problems) = plenipo_capabilities::vault::forget_ids(&store, &ids);
            for p in problems {
                log::warn!("a deleted organization's secret could not be forgotten: {p}");
            }
        }
        let left = folder.as_deref().is_some_and(|f| !orgs::remove_folder(f));
        orgs2.remove(&id, left);
        let _ = first.ledger.append_event(plenipo_ledger::NewEvent {
            source: OWNER.into(),
            event_type: "organization.deleted".into(),
            payload: serde_json::json!({ "id": id, "name": name, "saved": saved }),
            ..plenipo_ledger::NewEvent::default()
        });
        Ok(())
    })
    .await
    .map_err(|e| CommandError::internal(format!("deleting it failed: {e}")))??;
    orgs::list_changed(&app);
    Ok(listing(&orgs, &label))
}

// ---- Windows --------------------------------------------------------------------------------

fn focus<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        workspace_windows::show_popouts(app, label);
    }
}

/// The window's title names its organization (the first one's is just "Plenipo").
pub fn set_title<R: Runtime>(window: &WebviewWindow<R>, orgs: &Orgs, id: &str) {
    let title = if id == FIRST {
        "Plenipo".to_owned()
    } else {
        let name = orgs.stack(id).map_or_else(
            || orgs.entry(id).map(|e| e.name).unwrap_or_default(),
            |s| orgs::name_in(&s.ledger),
        );
        format!("{name} · Plenipo")
    };
    let _ = window.set_title(&title);
}

/// Window `label` stops showing its organization (a reload, another organization, or it
/// closed): the terminals it shows end, and its pop-outs close (ADR-091 §11).
pub fn leave<R: Runtime>(app: &AppHandle<R>, orgs: &Orgs, label: &str, why: &str) {
    let shown = orgs.stack_for_window(label);
    let first = orgs.first();
    // Its AI tools' sign-ins, which the first organization's broker runs.
    let signing: Vec<String> = orgs.ai_terminals_of(label);
    if let Some(first) = &first {
        for t in &signing {
            let _ = first.broker.close_terminal(t, why);
            orgs.forget_ai_terminal(t);
        }
    }
    match shown {
        // The first organization's broker also runs other windows' sign-ins: those stay.
        Some(stack) if stack.place.is_first() => {
            stack.broker.close_terminals_but(why, |t| {
                orgs.ai_terminal_window(t).is_some_and(|w| w != label)
            });
        }
        Some(stack) => stack.broker.close_all_terminals(why),
        None => {}
    }
    workspace_windows::close_popouts(app, label);
}
