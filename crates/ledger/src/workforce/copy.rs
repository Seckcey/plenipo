//! A new organization as a copy of another one's setup (Phase 21, ADR-094 §15): its roles and
//! specialties, its departments and their positions (the org chart, with no one hired), where
//! the owner placed their tiles, its permission sets and switches, its AI model choices, its
//! learning choices, and its titles. Never its work, the Activity trail, approvals, lessons,
//! experience, projects (and the positions that staff them), servers, connections, add-on
//! programs, or any secret.
//!
//! The copy goes into a new, empty Ledger before any service starts on it, so every row keeps
//! its ID and every setting that names a role, department, or position still names the same one.

use std::collections::{HashMap, HashSet};

use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, Connection};
use serde_json::{json, Value};

use super::canvas::{ORGANIZATION_TILE, OWNER_TILE};
use super::{invalid, now, org_event};
use crate::error::Result;
use crate::Ledger;

/// How much of the setup a copy brought.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetupCopied {
    pub roles: usize,
    pub specialties: usize,
    pub departments: usize,
    pub positions: usize,
}

/// Guard's settings a copy keeps: permission sets and who has which, command rules, blocked
/// files, what always asks, websites, switches, and the browser choice. Never the secrets'
/// names, servers, connections, or add-on programs.
const GUARD_KEPT: [&str; 10] = [
    "sets",
    "roles",
    "departments",
    "commands",
    "blockedFiles",
    "sensitive",
    "options",
    "websites",
    "switches",
    "browserChoice",
];

/// Rows of one table: its columns' names and each row's values.
struct Rows {
    columns: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
}

impl Rows {
    fn read(c: &Connection, sql: &str) -> Result<Self> {
        let mut stmt = c.prepare(sql)?;
        let columns: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|n| (*n).to_owned())
            .collect();
        let n = columns.len();
        let rows = stmt
            .query_map([], |r| (0..n).map(|i| r.get::<_, SqlValue>(i)).collect())?
            .collect::<rusqlite::Result<Vec<Vec<SqlValue>>>>()?;
        Ok(Self { columns, rows })
    }

    fn at(&self, column: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == column)
    }

    fn text(row: &[SqlValue], i: Option<usize>) -> Option<&str> {
        match i.and_then(|i| row.get(i)) {
            Some(SqlValue::Text(t)) => Some(t),
            _ => None,
        }
    }

    /// Insert `rows` into `table` of `tx` (a table name from this file, never from outside).
    fn insert(&self, tx: &Connection, table: &str, rows: &[Vec<SqlValue>]) -> Result<usize> {
        let marks: Vec<String> = (1..=self.columns.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "INSERT INTO {table} ({}) VALUES ({})",
            self.columns.join(", "),
            marks.join(", ")
        );
        let mut stmt = tx.prepare(&sql)?;
        for row in rows {
            stmt.execute(params_from_iter(row.iter()))?;
        }
        Ok(rows.len())
    }
}

/// What a copy reads from the organization it copies.
struct Setup {
    roles: Rows,
    specialties: Rows,
    departments: Rows,
    positions: Rows,
    /// Positions that lead a project (a project's Supervisor): they and everyone below them stay.
    project_leads: HashSet<String>,
    oversight: Rows,
    places: Rows,
    settings: HashMap<String, Value>,
}

impl Setup {
    fn read(c: &Connection) -> Result<Self> {
        let project_leads = {
            let mut stmt = c.prepare(
                "SELECT coordinator_position_id FROM projects
                 WHERE coordinator_position_id IS NOT NULL",
            )?;
            let leads = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<HashSet<String>>>()?;
            leads
        };
        let mut settings = HashMap::new();
        for key in ["guard", "routing", "learning", "organization"] {
            let value: Option<String> = c
                .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                    r.get(0)
                })
                .ok();
            if let Some(v) = value.and_then(|v| serde_json::from_str(&v).ok()) {
                settings.insert(key.to_owned(), v);
            }
        }
        Ok(Self {
            roles: Rows::read(c, "SELECT * FROM roles ORDER BY created_at, id")?,
            specialties: Rows::read(c, "SELECT * FROM specialties ORDER BY created_at, id")?,
            departments: Rows::read(
                c,
                "SELECT * FROM departments
                 WHERE archived_at IS NULL AND deleted_at IS NULL ORDER BY created_at, id",
            )?,
            positions: Rows::read(
                c,
                "SELECT * FROM positions
                 WHERE state = 'active' AND deleted_at IS NULL ORDER BY created_at, id",
            )?,
            project_leads,
            oversight: Rows::read(c, "SELECT * FROM oversight WHERE state = 'active'")?,
            places: Rows::read(c, "SELECT * FROM canvas_places")?,
            settings,
        })
    }

    /// The positions to copy, each after the one it reports to: every active one outside a
    /// project's team.
    fn positions(&self) -> Vec<Vec<SqlValue>> {
        let id_at = self.positions.at("id");
        let up_at = self.positions.at("reports_to");
        let up: HashMap<&str, Option<&str>> = self
            .positions
            .rows
            .iter()
            .filter_map(|r| Some((Rows::text(r, id_at)?, Rows::text(r, up_at))))
            .collect();
        // Outside every project's team: no lead above (or itself) leads a project.
        let kept = |id: &str| -> bool {
            let mut at = Some(id);
            let mut steps = 0;
            while let Some(p) = at {
                if self.project_leads.contains(p) || steps > up.len() {
                    return false;
                }
                at = up.get(p).copied().flatten();
                steps += 1;
            }
            true
        };
        let mut left: Vec<&Vec<SqlValue>> = self
            .positions
            .rows
            .iter()
            .filter(|r| Rows::text(r, id_at).is_some_and(kept))
            .collect();
        let mut placed: HashSet<String> = HashSet::new();
        let mut ordered = Vec::new();
        while !left.is_empty() {
            let before = left.len();
            left.retain(|r| {
                let ready =
                    Rows::text(r, up_at).is_none_or(|u| placed.contains(u) || !up.contains_key(u));
                if ready {
                    if let Some(id) = Rows::text(r, id_at) {
                        placed.insert(id.to_owned());
                    }
                    ordered.push((*r).clone());
                }
                !ready
            });
            if left.len() == before {
                break;
            }
        }
        // One reporting to a position that stays behind (an archived one) reports to you.
        if let Some(i) = up_at {
            for r in &mut ordered {
                if Rows::text(r, Some(i)).is_some_and(|u| !placed.contains(u)) {
                    r[i] = SqlValue::Null;
                }
            }
        }
        ordered
    }
}

/// Guard's settings without what never leaves an organization.
fn guard_kept(guard: &Value) -> Value {
    let mut kept = serde_json::Map::new();
    if let Some(all) = guard.as_object() {
        for key in GUARD_KEPT {
            if let Some(v) = all.get(key) {
                kept.insert(key.to_owned(), v.clone());
            }
        }
    }
    Value::Object(kept)
}

/// The Router's settings, but not the usage limits the owner cleared (the other organization's
/// usage).
fn routing_kept(routing: &Value) -> Value {
    let mut kept = routing.clone();
    if let Some(all) = kept.as_object_mut() {
        all.remove("clearedLimits");
    }
    kept
}

impl Ledger {
    /// Copy `source`'s setup into this Ledger, which must be new and empty (Phase 21, ADR-094
    /// §15). `from` is the source organization's name, for the record.
    pub fn copy_setup_from(&self, source: &Ledger, from: &str, actor: &str) -> Result<SetupCopied> {
        let setup = source.read(Setup::read)?;
        let positions = setup.positions();
        self.write(|tx, out| {
            let used: i64 = tx.query_row(
                "SELECT (SELECT COUNT(*) FROM roles) + (SELECT COUNT(*) FROM positions)
                      + (SELECT COUNT(*) FROM departments)",
                [],
                |r| r.get(0),
            )?;
            if used > 0 {
                return Err(invalid("a setup is copied only into a new organization"));
            }
            let at = now();
            let copied = SetupCopied {
                roles: setup.roles.insert(tx, "roles", &setup.roles.rows)?,
                specialties: setup.specialties.insert(
                    tx,
                    "specialties",
                    &setup.specialties.rows,
                )?,
                departments: {
                    // Their heads come after the positions.
                    let head = setup.departments.at("head_position_id");
                    let made = setup.departments.at("created_at");
                    let rows: Vec<Vec<SqlValue>> = setup
                        .departments
                        .rows
                        .iter()
                        .map(|r| {
                            let mut r = r.clone();
                            if let Some(i) = head {
                                r[i] = SqlValue::Null;
                            }
                            if let Some(i) = made {
                                r[i] = SqlValue::Integer(at);
                            }
                            r
                        })
                        .collect();
                    setup.departments.insert(tx, "departments", &rows)?
                },
                positions: {
                    let p = &setup.positions;
                    let rows: Vec<Vec<SqlValue>> = positions
                        .iter()
                        .map(|r| {
                            let mut r = r.clone();
                            for column in ["created_at", "updated_at"] {
                                if let Some(i) = p.at(column) {
                                    r[i] = SqlValue::Integer(at);
                                }
                            }
                            // Where it came from on loan, or an archive's note, is the other
                            // organization's.
                            if let Some(i) = p.at("metadata") {
                                if let SqlValue::Text(m) = &r[i] {
                                    let mut v: Value =
                                        serde_json::from_str(m).unwrap_or_else(|_| json!({}));
                                    if let Some(o) = v.as_object_mut() {
                                        o.remove("archive");
                                        o.remove("loan");
                                    }
                                    r[i] = SqlValue::Text(v.to_string());
                                }
                            }
                            r
                        })
                        .collect();
                    p.insert(tx, "positions", &rows)?
                },
            };
            // The departments' heads, when they came.
            let ids: HashSet<String> = positions
                .iter()
                .filter_map(|r| Rows::text(r, setup.positions.at("id")).map(str::to_owned))
                .collect();
            let (dept_id, head_id) = (
                setup.departments.at("id"),
                setup.departments.at("head_position_id"),
            );
            for d in &setup.departments.rows {
                if let (Some(id), Some(head)) = (Rows::text(d, dept_id), Rows::text(d, head_id)) {
                    if ids.contains(head) {
                        tx.execute(
                            "UPDATE departments SET head_position_id = ?2 WHERE id = ?1",
                            [id, head],
                        )?;
                    }
                }
            }
            // Review, QA, and security between positions that both came.
            let (over, target) = (
                setup.oversight.at("overseer_id"),
                setup.oversight.at("target_id"),
            );
            let oversight: Vec<Vec<SqlValue>> = setup
                .oversight
                .rows
                .iter()
                .filter(|r| {
                    Rows::text(r, over).is_some_and(|o| ids.contains(o))
                        && Rows::text(r, target).is_some_and(|t| ids.contains(t))
                })
                .cloned()
                .collect();
            setup.oversight.insert(tx, "oversight", &oversight)?;
            // Where the owner placed the tiles that came.
            let tile = setup.places.at("tile_id");
            let places: Vec<Vec<SqlValue>> = setup
                .places
                .rows
                .iter()
                .filter(|r| {
                    Rows::text(r, tile).is_some_and(|t| {
                        t == OWNER_TILE || t == ORGANIZATION_TILE || ids.contains(t)
                    })
                })
                .cloned()
                .collect();
            setup.places.insert(tx, "canvas_places", &places)?;
            // The settings that are setup.
            let mut settings: Vec<(&str, Value)> = Vec::new();
            if let Some(g) = setup.settings.get("guard") {
                settings.push(("guard", guard_kept(g)));
            }
            if let Some(r) = setup.settings.get("routing") {
                settings.push(("routing", routing_kept(r)));
            }
            if let Some(l) = setup.settings.get("learning") {
                settings.push(("learning", l.clone()));
            }
            if let Some(titles) = setup.settings.get("organization").map(|o| &o["titles"]) {
                if !titles.is_null() {
                    settings.push(("organization", json!({ "titles": titles })));
                }
            }
            for (key, value) in &settings {
                tx.execute(
                    "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params![key, value.to_string(), at],
                )?;
            }
            org_event(
                out,
                tx,
                actor,
                "setup_copied",
                json!({
                    "from": from,
                    "roles": copied.roles,
                    "specialties": copied.specialties,
                    "departments": copied.departments,
                    "positions": copied.positions,
                }),
            )?;
            Ok(copied)
        })
    }
}
