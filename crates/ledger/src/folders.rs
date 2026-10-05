//! The organization folder (ADR-205): where the organization's own folders are on this PC.
//! Plenipo's capability side makes them (`plenipo_capabilities::org_folder`); this module records
//! each one once, with its `folder.*` event in the same transaction. Only a path's shape is
//! checked here (absolute, one line, at most 1,000 characters); where a folder may be is Guard's
//! (`plenipo_guard::places`).

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension as _};
use serde_json::json;

use crate::dto::{Folder, FolderKind, FolderOrigin, NewEvent, NewFolder};
use crate::error::{LedgerError, Result};
use crate::rows::{parse_enum, u64_of};
use crate::{events, Ledger};

const COLS: &str = "id, kind, ref_id, path, made_by_plenipo, created_at";

/// The longest folder path Plenipo records.
pub const MAX_FOLDER_PATH: usize = 1000;

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: r.get(0)?,
        kind: parse_enum(1, r.get(1)?, FolderKind::parse)?,
        ref_id: r.get(2)?,
        path: r.get(3)?,
        made_by_plenipo: r.get::<_, i64>(4)? != 0,
        created_at: u64_of(r.get(5)?),
    })
}

fn get(conn: &Connection, kind: FolderKind, ref_id: Option<&str>) -> Result<Option<Folder>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLS} FROM folders WHERE kind = ?1 AND COALESCE(ref_id, '') = ?2"
            ),
            params![kind.as_str(), ref_id.unwrap_or("")],
            row,
        )
        .optional()?)
}

fn by_id(conn: &Connection, id: &str) -> Result<Option<Folder>> {
    Ok(conn
        .query_row(&format!("SELECT {COLS} FROM folders WHERE id = ?1"), [id], row)
        .optional()?)
}

fn invalid(message: impl Into<String>) -> LedgerError {
    LedgerError::InvalidInput(message.into())
}

/// A folder path as Plenipo records it: absolute on this PC, one line, at most 1,000 characters,
/// with nothing around it.
fn check_path(path: &str) -> Result<()> {
    if path.is_empty() || path.trim() != path {
        return Err(invalid("a folder's path can't be empty or start or end with a space"));
    }
    if path.chars().count() > MAX_FOLDER_PATH {
        return Err(invalid(format!(
            "a folder's path is limited to {MAX_FOLDER_PATH} characters"
        )));
    }
    if path.chars().any(char::is_control) {
        return Err(invalid("a folder's path can't hold control characters"));
    }
    if !Path::new(path).is_absolute() {
        return Err(invalid(format!("{path} is not a full path on this computer")));
    }
    Ok(())
}

/// The table that holds what a folder of `kind` belongs to.
fn owner_table(kind: FolderKind) -> &'static [&'static str] {
    match kind {
        FolderKind::Organization => &[],
        FolderKind::Department | FolderKind::DepartmentFiles => &["departments"],
        FolderKind::Project | FolderKind::ProjectFiles => &["projects"],
        FolderKind::ScratchPads => &["departments", "projects"],
        FolderKind::ScratchPad => &["positions"],
    }
}

fn folder_event(actor: &str, event_type: &str, f: &Folder) -> NewEvent {
    NewEvent {
        source: actor.into(),
        event_type: event_type.into(),
        payload: json!({
            "folderId": f.id,
            "kind": f.kind,
            "refId": f.ref_id,
            "path": f.path,
        }),
        ..NewEvent::default()
    }
}

impl Ledger {
    /// Every folder recorded, oldest first.
    pub fn folders(&self) -> Result<Vec<Folder>> {
        self.read(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {COLS} FROM folders ORDER BY created_at, rowid"
            ))?;
            let rows = stmt.query_map([], row)?.collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// The folder of `kind` that belongs to `ref_id` (`None`: the organization's own).
    pub fn folder(&self, kind: FolderKind, ref_id: Option<&str>) -> Result<Option<Folder>> {
        self.read(|c| get(c, kind, ref_id))
    }

    /// The organization folder, once there is one.
    pub fn organization_folder(&self) -> Result<Option<Folder>> {
        self.folder(FolderKind::Organization, None)
    }

    /// Record one of the organization's folders, with `folder.made` (Plenipo made it) or
    /// `folder.adopted` (a plain folder that was already there). Each department, project,
    /// position, and the organization has at most one folder of each kind, and no two records
    /// name the same place (letter case aside). Every folder but the organization folder needs
    /// the organization folder first.
    pub fn record_folder(
        &self,
        new: &NewFolder,
        origin: FolderOrigin,
        actor: &str,
    ) -> Result<Folder> {
        check_path(&new.path)?;
        match (new.kind.belongs_to_something(), new.ref_id.is_some()) {
            (Some(true), false) => {
                return Err(invalid(format!(
                    "a {} folder belongs to something",
                    new.kind.as_str()
                )))
            }
            (Some(false), true) => {
                return Err(invalid("the organization folder belongs to the organization"))
            }
            _ => {}
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.write(|tx, out| {
            if let Some(ref_id) = &new.ref_id {
                let mut found = false;
                for table in owner_table(new.kind) {
                    let hit: Option<String> = tx
                        .query_row(
                            &format!("SELECT id FROM {table} WHERE id = ?1"),
                            [ref_id],
                            |r| r.get(0),
                        )
                        .optional()?;
                    found |= hit.is_some();
                }
                if !found {
                    return Err(LedgerError::NotFound(format!(
                        "what the {} folder belongs to ({ref_id})",
                        new.kind.as_str()
                    )));
                }
            }
            if new.kind != FolderKind::Organization
                && get(tx, FolderKind::Organization, None)?.is_none()
            {
                return Err(invalid("this organization has no organization folder yet"));
            }
            if get(tx, new.kind, new.ref_id.as_deref())?.is_some() {
                return Err(invalid(format!(
                    "it already has its {} folder",
                    new.kind.as_str()
                )));
            }
            let taken: Option<String> = tx
                .query_row(
                    "SELECT id FROM folders WHERE path = ?1 COLLATE NOCASE",
                    [&new.path],
                    |r| r.get(0),
                )
                .optional()?;
            if taken.is_some() {
                return Err(invalid(format!(
                    "{} is already one of the organization's folders",
                    new.path
                )));
            }
            tx.execute(
                "INSERT INTO folders (id, kind, ref_id, path, made_by_plenipo, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id,
                    new.kind.as_str(),
                    new.ref_id,
                    new.path,
                    i64::from(origin == FolderOrigin::Made),
                    crate::now_ms() as i64
                ],
            )?;
            let folder = by_id(tx, &id)?
                .ok_or_else(|| LedgerError::NotFound(format!("folder {id}")))?;
            let event_type = match origin {
                FolderOrigin::Made => "folder.made",
                FolderOrigin::Adopted => "folder.adopted",
            };
            out.push(events::insert(tx, folder_event(actor, event_type, &folder))?);
            Ok(folder)
        })
    }

    /// A recorded folder had gone missing and Plenipo made it again at its recorded path
    /// (`folder.remade`; ADR-205: never somewhere else).
    pub fn record_folder_remade(&self, id: &str, actor: &str) -> Result<()> {
        self.write(|tx, out| {
            let folder =
                by_id(tx, id)?.ok_or_else(|| LedgerError::NotFound(format!("folder {id}")))?;
            out.push(events::insert(tx, folder_event(actor, "folder.remade", &folder))?);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::ledger;

    fn place(name: &str) -> String {
        std::env::temp_dir()
            .join("plenipo-folder-test")
            .join(name)
            .display()
            .to_string()
    }

    fn new(kind: FolderKind, ref_id: Option<&str>, path: &str) -> NewFolder {
        NewFolder {
            kind,
            ref_id: ref_id.map(str::to_owned),
            path: path.to_owned(),
        }
    }

    #[test]
    fn the_organization_folder_comes_first_and_each_thing_has_one_of_each_kind() {
        let l = ledger();
        assert!(l.organization_folder().unwrap().is_none());
        // Nothing but the organization folder can be recorded before it.
        let early = l.record_folder(
            &new(FolderKind::ScratchPads, None, &place("Acme/Scratch pads")),
            FolderOrigin::Made,
            "plenipo",
        );
        assert!(early.is_err(), "{early:?}");
        let org = l
            .record_folder(
                &new(FolderKind::Organization, None, &place("Acme")),
                FolderOrigin::Made,
                "owner",
            )
            .unwrap();
        assert!(org.made_by_plenipo);
        assert_eq!(l.organization_folder().unwrap(), Some(org.clone()));
        // One organization folder.
        assert!(l
            .record_folder(
                &new(FolderKind::Organization, None, &place("Other")),
                FolderOrigin::Made,
                "owner",
            )
            .is_err());
        let pads = l
            .record_folder(
                &new(FolderKind::ScratchPads, None, &place("Acme/Scratch pads")),
                FolderOrigin::Adopted,
                "plenipo",
            )
            .unwrap();
        assert!(!pads.made_by_plenipo);
        // The same place twice, in other letters too, is refused.
        let d = l.create_department("Marketing", "", None, "owner").unwrap();
        let again = l.record_folder(
            &new(FolderKind::Department, Some(&d.id), &place("ACME/scratch PADS")),
            FolderOrigin::Made,
            "plenipo",
        );
        assert!(again.is_err(), "{again:?}");
        let dept = l
            .record_folder(
                &new(FolderKind::Department, Some(&d.id), &place("Acme/Marketing")),
                FolderOrigin::Made,
                "plenipo",
            )
            .unwrap();
        assert_eq!(
            l.folder(FolderKind::Department, Some(&d.id)).unwrap(),
            Some(dept.clone())
        );
        // One department folder for the department.
        assert!(l
            .record_folder(
                &new(FolderKind::Department, Some(&d.id), &place("Acme/Marketing (2)")),
                FolderOrigin::Made,
                "plenipo",
            )
            .is_err());
        assert_eq!(l.folders().unwrap(), vec![org, pads, dept]);
    }

    #[test]
    fn what_a_folder_belongs_to_must_exist_and_paths_must_be_full() {
        let l = ledger();
        l.record_folder(
            &new(FolderKind::Organization, None, &place("Acme")),
            FolderOrigin::Made,
            "owner",
        )
        .unwrap();
        for (kind, ref_id) in [
            (FolderKind::Department, Some("nope")),
            (FolderKind::ScratchPad, Some("nope")),
            (FolderKind::ProjectFiles, Some("nope")),
            (FolderKind::Department, None),
        ] {
            let got = l.record_folder(
                &new(kind, ref_id, &place("Acme/x")),
                FolderOrigin::Made,
                "plenipo",
            );
            assert!(got.is_err(), "{kind:?} {ref_id:?}: {got:?}");
        }
        let d = l.create_department("Sales", "", None, "owner").unwrap();
        for bad in ["relative\\path", "", " padded", "line\nbreak"] {
            let got = l.record_folder(
                &new(FolderKind::Department, Some(&d.id), bad),
                FolderOrigin::Made,
                "plenipo",
            );
            assert!(got.is_err(), "{bad:?}");
        }
        let long = place(&"x".repeat(MAX_FOLDER_PATH));
        assert!(l
            .record_folder(
                &new(FolderKind::Department, Some(&d.id), &long),
                FolderOrigin::Made,
                "plenipo",
            )
            .is_err());
    }

    #[test]
    fn each_record_has_its_event_and_rows_are_never_deleted() {
        let l = ledger();
        let org = l
            .record_folder(
                &new(FolderKind::Organization, None, &place("Acme")),
                FolderOrigin::Made,
                "owner",
            )
            .unwrap();
        l.record_folder_remade(&org.id, "plenipo").unwrap();
        let events = l.recent_events(10).unwrap();
        let kinds: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
        assert!(kinds.contains(&"folder.made"), "{kinds:?}");
        assert!(kinds.contains(&"folder.remade"), "{kinds:?}");
        let made = events
            .iter()
            .find(|e| e.event_type == "folder.made")
            .unwrap();
        assert_eq!(made.source, "owner");
        assert_eq!(made.payload["path"], org.path);
        assert_eq!(made.payload["kind"], "organization");
        assert!(l.record_folder_remade("nope", "plenipo").is_err());
        let deleted = l.read(|c| {
            c.execute("DELETE FROM folders", [])
                .map_err(LedgerError::from)
        });
        assert!(deleted.is_err(), "rows are never deleted");
    }
}
