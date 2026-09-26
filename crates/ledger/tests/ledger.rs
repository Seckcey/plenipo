//! Ledger integration tests: migrations, durability, concurrency, corruption, backup/export.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use plenipo_ledger::migrate::{self, Migration};
use plenipo_ledger::{Ledger, LedgerError, NewEvent, NewTask, TaskState, DB_FILE_NAME, MIGRATIONS};
use rusqlite::Connection;
use serde_json::json;

fn db_path(dir: &Path) -> PathBuf {
    dir.join("ledger").join(DB_FILE_NAME)
}

fn new_task(l: &Ledger, objective: &str) -> plenipo_ledger::Task {
    l.create_task(
        NewTask {
            requested_by: "owner".into(),
            objective: objective.into(),
            ..NewTask::default()
        },
        "owner",
    )
    .unwrap()
}

/// Normalized schema (tables, indexes, triggers) for comparisons.
fn schema(conn: &Connection) -> Vec<(String, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT type, name FROM sqlite_master
             WHERE name NOT LIKE 'sqlite_%' AND name != 'schema_migrations' ORDER BY type, name",
        )
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

// ---- Migrations ------------------------------------------------------------------------

#[test]
fn migrations_apply_roll_back_and_reapply_cleanly() {
    let conn = Connection::open_in_memory().unwrap();
    let report = migrate::migrate(&conn, MIGRATIONS, |_| panic!("no backup for a new db")).unwrap();
    assert_eq!((report.from, report.to), (0, migrate::latest(MIGRATIONS)));
    let full = schema(&conn);
    assert!(full.iter().any(|(_, n)| n == "tasks"));
    assert!(full
        .iter()
        .any(|(t, n)| t == "trigger" && n == "events_are_append_only_update"));

    migrate::rollback_to(&conn, MIGRATIONS, 0).unwrap();
    assert!(
        schema(&conn).is_empty(),
        "down scripts remove everything: {:?}",
        schema(&conn)
    );
    assert_eq!(migrate::current_version(&conn).unwrap(), 0);

    migrate::migrate(&conn, MIGRATIONS, |_| Ok(())).unwrap();
    assert_eq!(
        schema(&conn),
        full,
        "up after down reproduces the same schema"
    );

    // Idempotent: nothing pending.
    let again = migrate::migrate(&conn, MIGRATIONS, |_| panic!("nothing to do")).unwrap();
    assert!(again.applied.is_empty());
}

/// A synthetic migration one past the newest real one (simulates a future Plenipo).
const NEXT: Migration = Migration {
    version: 5,
    name: "test_add_column",
    up: "ALTER TABLE tasks ADD COLUMN estimate_minutes INTEGER;",
    down: "ALTER TABLE tasks DROP COLUMN estimate_minutes;",
};

fn with_next() -> Vec<Migration> {
    let mut all = MIGRATIONS.to_vec();
    all.push(NEXT);
    assert_eq!(migrate::latest(MIGRATIONS) + 1, NEXT.version);
    all
}

#[test]
fn upgrading_an_existing_ledger_takes_a_backup_first() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    let current = migrate::latest(MIGRATIONS);
    let task_id = {
        let l = Ledger::open(&path).unwrap();
        new_task(&l, "exists before upgrade").id
    };
    let l = Ledger::open_with(&path, &with_next()).unwrap();
    assert_eq!(l.schema_version().unwrap(), NEXT.version);
    assert!(
        l.task(&task_id).unwrap().is_some(),
        "data survives the upgrade"
    );
    let status = l.status().unwrap();
    let notice = status
        .notices
        .iter()
        .find(|n| n.contains(&format!("upgraded from version {current}")))
        .unwrap();
    let backups: Vec<_> = std::fs::read_dir(path.parent().unwrap().join("backups"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(backups.len(), 1);
    assert!(backups[0].starts_with(&format!("pre-migration-v{current}-")));
    assert!(notice.contains(&backups[0]));

    // The backup is a usable ledger of the previous version (the production rollback path).
    let backup = path.parent().unwrap().join("backups").join(&backups[0]);
    let restored = Ledger::open(&backup).unwrap();
    assert_eq!(restored.schema_version().unwrap(), current);
    assert!(restored.task(&task_id).unwrap().is_some());
}

#[test]
fn phase2_ledger_upgrades_to_runtime_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    let task_id = {
        let v1 = Ledger::open_with(&path, &MIGRATIONS[..1]).unwrap();
        assert_eq!(v1.schema_version().unwrap(), 1);
        let t = new_task(&v1, "from Phase 2");
        v1.transition_task(&t.id, TaskState::Running, "w", None)
            .unwrap();
        t.id
    };
    let l = Ledger::open(&path).unwrap();
    assert_eq!(l.schema_version().unwrap(), migrate::latest(MIGRATIONS));
    let status = l.status().unwrap();
    assert!(status
        .notices
        .iter()
        .any(|n| n.contains("upgraded from version 1")));
    // Phase 2 history is intact, and Phase 2 tasks are not session turns.
    assert_eq!(l.events_for_task(&task_id).unwrap().len(), 2);
    assert!(l.unfinished_session_tasks().unwrap().is_empty());
    let s = l
        .open_runtime_session(
            plenipo_ledger::NewRuntimeSession {
                id: "s-1".into(),
                runtime: "codex".into(),
                provider: "openai".into(),
                title: "t".into(),
                working_dir: "/w".into(),
                ..Default::default()
            },
            "owner",
        )
        .unwrap();
    assert_eq!(s.turn_count, 0);
    assert!(l.integrity_check().unwrap().ok);
}

#[test]
fn phase3_ledger_upgrades_to_liaison_messages() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    let (task_id, session_id) = {
        let v2 = Ledger::open_with(&path, &MIGRATIONS[..2]).unwrap();
        assert_eq!(v2.schema_version().unwrap(), 2);
        let s = v2
            .open_runtime_session(
                plenipo_ledger::NewRuntimeSession {
                    id: "s-1".into(),
                    runtime: "codex".into(),
                    provider: "openai".into(),
                    title: "t".into(),
                    working_dir: "/w".into(),
                    ..Default::default()
                },
                "owner",
            )
            .unwrap();
        let t = v2
            .create_task(
                NewTask {
                    requested_by: "owner".into(),
                    objective: "a Phase 3 turn".into(),
                    metadata: json!({ "sessionId": s.id, "turn": 1 }),
                    ..NewTask::default()
                },
                "owner",
            )
            .unwrap();
        v2.transition_task(&t.id, TaskState::Running, "agent:codex", None)
            .unwrap();
        v2.transition_task(&t.id, TaskState::Succeeded, "agent:codex", None)
            .unwrap();
        (t.id, s.id)
    };
    let l = Ledger::open(&path).unwrap();
    assert_eq!(l.schema_version().unwrap(), migrate::latest(MIGRATIONS));
    assert!(l
        .status()
        .unwrap()
        .notices
        .iter()
        .any(|n| n.contains("upgraded from version 2")));
    // Phase 3 sessions and turns are intact and carry no Liaison messages.
    assert_eq!(l.session_tasks(&session_id).unwrap()[0].id, task_id);
    assert_eq!(l.events_for_task(&task_id).unwrap().len(), 3);
    assert!(l.liaison_messages_for_task(&task_id).unwrap().is_empty());
    assert!(l.liaison_open_requests().unwrap().is_empty());
    assert!(l.integrity_check().unwrap().ok);
}

#[test]
fn phase4_ledger_upgrades_to_the_workforce() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    let task_id = {
        let v3 = Ledger::open_with(&path, &MIGRATIONS[..3]).unwrap();
        assert_eq!(v3.schema_version().unwrap(), 3);
        new_task(&v3, "from Phase 4").id
    };
    // Organization rows as a Phase 4 build wrote them (its own column set).
    let (role_id, department_id, project_id, agent_id) = ("r-1", "d-1", "p-1", "a-1");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "INSERT INTO roles (id, name, role_type, persistent, created_at)
                 VALUES ('r-1', 'Development Superintendent', 'superintendent', 1, 1);
             INSERT INTO departments (id, name, manager_role_id, created_at)
                 VALUES ('d-1', 'Development', 'r-1', 1);
             INSERT INTO projects (id, name, department_id, created_at)
                 VALUES ('p-1', 'Cloudline', 'd-1', 1);
             INSERT INTO agent_instances (id, role_id, project_id, lifecycle_state, created_at, last_seen_at)
                 VALUES ('a-1', 'r-1', 'p-1', 'active', 1, 1);",
        )
        .unwrap();
    }
    let l = Ledger::open(&path).unwrap();
    assert_eq!(l.schema_version().unwrap(), 4);
    assert!(l
        .status()
        .unwrap()
        .notices
        .iter()
        .any(|n| n.contains("upgraded from version 3")));
    // Earlier records are intact, with the new columns at their defaults.
    let records = l.org_records().unwrap();
    let dept = records
        .departments
        .iter()
        .find(|d| d.id == department_id)
        .unwrap();
    assert_eq!(dept.manager_role_id.as_deref(), Some(role_id));
    assert_eq!(dept.head_position_id, None);
    let project = records
        .projects
        .iter()
        .find(|p| p.id == project_id)
        .unwrap();
    assert_eq!(project.status, "active");
    assert!(
        project.allowed_runtimes.is_empty(),
        "no runtime is allowed until chosen"
    );
    assert_eq!(project.coordinator_position_id, None);
    let agent = l.agent_instance(agent_id).unwrap().unwrap();
    assert_eq!(
        (agent.position_id, agent.task_id, agent.retired_at),
        (None, None, None)
    );
    assert!(records.positions.is_empty() && records.oversight.is_empty());
    assert_eq!(l.events_for_task(&task_id).unwrap().len(), 1);
    assert!(l.integrity_check().unwrap().ok);
}

#[test]
fn newer_schema_is_refused_and_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    drop(Ledger::open_with(&path, &with_next()).unwrap());
    let before = std::fs::read(&path).unwrap();
    match Ledger::open(&path) {
        Err(LedgerError::NewerSchema { db: 5, app: 4 }) => {}
        other => panic!("expected NewerSchema, got {:?}", other.map(|_| ())),
    }
    assert!(path.exists(), "not quarantined");
    assert_eq!(std::fs::read(&path).unwrap().len(), before.len());
}

#[test]
fn edited_or_skipped_migrations_are_detected() {
    let conn = Connection::open_in_memory().unwrap();
    migrate::migrate(&conn, MIGRATIONS, |_| Ok(())).unwrap();
    let edited = Migration {
        up: "-- changed\nCREATE TABLE x (y);",
        ..MIGRATIONS[0]
    };
    assert!(matches!(
        migrate::migrate(&conn, &[edited], |_| Ok(())),
        Err(LedgerError::ModifiedMigration(1))
    ));

    // History says the next version is applied but v1 never was.
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO schema_migrations VALUES (?1, 'test_add_column', ?2, 0)",
        rusqlite::params![NEXT.version, NEXT.checksum()],
    )
    .unwrap();
    assert!(matches!(
        migrate::migrate(&conn, &with_next(), |_| Ok(())),
        Err(LedgerError::InconsistentMigrations(_))
    ));
}

// ---- Durability -----------------------------------------------------------------------

#[test]
fn history_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    let (id, trail) = {
        let l = Ledger::open(&path).unwrap();
        let t = new_task(&l, "durable");
        l.transition_task(&t.id, TaskState::Running, "worker", None)
            .unwrap();
        l.transition_task(&t.id, TaskState::Blocked, "worker", Some("input needed"))
            .unwrap();
        (t.id.clone(), l.events_for_task(&t.id).unwrap())
    };
    let l = Ledger::open(&path).unwrap();
    assert_eq!(l.task(&id).unwrap().unwrap().state, TaskState::Blocked);
    assert_eq!(l.events_for_task(&id).unwrap(), trail);
    assert!(l.status().unwrap().notices.is_empty());
    let mode: String = Connection::open(&path)
        .unwrap()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn committed_history_survives_a_hard_killed_writer() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_ledger-writer"))
        .arg(&path)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let task_id = lines
        .next()
        .unwrap()
        .unwrap()
        .trim_start_matches("task:")
        .to_owned();
    let mut committed = 0;
    for line in lines.by_ref() {
        committed = line
            .unwrap()
            .trim_start_matches("committed:")
            .parse::<u64>()
            .unwrap();
        if committed >= 200 {
            break;
        }
    }
    child.kill().unwrap(); // SIGKILL / TerminateProcess mid-stream: no clean shutdown
    child.wait().unwrap();

    let l = Ledger::open(&path).unwrap();
    assert!(
        l.status().unwrap().notices.is_empty(),
        "no corruption after a hard kill"
    );
    let task = l.task(&task_id).unwrap().unwrap();
    assert_eq!(task.state, TaskState::Running);
    let trail = l.events_for_task(&task_id).unwrap();
    let ticks: Vec<u64> = trail
        .iter()
        .filter(|e| e.event_type == "synthetic.tick")
        .map(|e| e.payload["n"].as_u64().unwrap())
        .collect();
    assert!(
        ticks.len() as u64 >= committed,
        "every acknowledged commit survived"
    );
    assert_eq!(
        ticks,
        (1..=ticks.len() as u64).collect::<Vec<_>>(),
        "no gaps or reordering"
    );
    assert!(l.integrity_check().unwrap().ok);
}

// ---- Concurrency ----------------------------------------------------------------------

#[test]
fn concurrent_writers_on_one_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let l = Arc::new(Ledger::open(&db_path(dir.path())).unwrap());
    let t = new_task(&l, "shared");
    let handles: Vec<_> = (0..8)
        .map(|w| {
            let l = l.clone();
            let id = t.id.clone();
            std::thread::spawn(move || {
                for n in 0..50 {
                    l.append_event(NewEvent {
                        task_id: Some(id.clone()),
                        source: format!("writer-{w}"),
                        event_type: "synthetic.tick".into(),
                        payload: json!({ "w": w, "n": n }),
                        ..NewEvent::default()
                    })
                    .unwrap();
                }
            })
        })
        .collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
    assert_ordered_per_writer(&l, &t.id, 8, 50);
}

#[test]
fn concurrent_writers_on_separate_connections() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    let id = new_task(&Ledger::open(&path).unwrap(), "multi-connection").id;
    let handles: Vec<_> = (0..4)
        .map(|w| {
            let path = path.clone();
            let id = id.clone();
            std::thread::spawn(move || {
                let l = Ledger::open(&path).unwrap(); // its own connection
                for n in 0..100 {
                    l.append_event(NewEvent {
                        task_id: Some(id.clone()),
                        source: format!("writer-{w}"),
                        event_type: "synthetic.tick".into(),
                        payload: json!({ "w": w, "n": n }),
                        ..NewEvent::default()
                    })
                    .unwrap();
                }
            })
        })
        .collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
    assert_ordered_per_writer(&Ledger::open(&path).unwrap(), &id, 4, 100);
}

#[test]
fn writers_wait_out_a_lock_held_longer_than_the_busy_timeout() {
    // Another connection holds the write lock for longer than SQLite's 5 s busy timeout
    // (e.g. a slow disk flush under contention). The ledger must wait, not fail.
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    let l = Ledger::open(&path).unwrap();
    let t = new_task(&l, "patient");
    let blocker = Connection::open(&path).unwrap();
    blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(6_500));
        blocker.execute_batch("COMMIT").unwrap();
    });
    let started = std::time::Instant::now();
    l.append_event(NewEvent {
        task_id: Some(t.id.clone()),
        source: "writer".into(),
        event_type: "synthetic.tick".into(),
        ..NewEvent::default()
    })
    .expect("write must succeed once the lock is released");
    assert!(started.elapsed() >= std::time::Duration::from_secs(6));
    release.join().unwrap();
    assert_eq!(l.events_for_task(&t.id).unwrap().len(), 2);
}

fn assert_ordered_per_writer(l: &Ledger, task_id: &str, writers: u64, each: u64) {
    let ticks: Vec<_> = l
        .events_for_task(task_id)
        .unwrap()
        .into_iter()
        .filter(|e| e.event_type == "synthetic.tick")
        .collect();
    assert_eq!(ticks.len() as u64, writers * each, "no lost writes");
    assert!(
        ticks.windows(2).all(|p| p[0].seq < p[1].seq),
        "unique, increasing seq"
    );
    for w in 0..writers {
        let ns: Vec<u64> = ticks
            .iter()
            .filter(|e| e.payload["w"] == w)
            .map(|e| e.payload["n"].as_u64().unwrap())
            .collect();
        assert_eq!(
            ns,
            (0..each).collect::<Vec<_>>(),
            "writer {w} order preserved"
        );
    }
}

// ---- Corruption -----------------------------------------------------------------------

fn populated(path: &Path) {
    let l = Ledger::open(path).unwrap();
    for i in 0..300 {
        new_task(&l, &format!("task {i} {}", "padding ".repeat(20)));
    }
    // Dropping the connection checkpoints the WAL into the main file.
}

fn assert_quarantined(path: &Path) {
    let l = Ledger::open(path).unwrap();
    let status = l.status().unwrap();
    let notice = status
        .notices
        .iter()
        .find(|n| n.contains("failed its integrity check"))
        .expect("corruption must be reported");
    assert!(notice.contains(".corrupt-"), "{notice}");
    assert_eq!(status.task_count, 0, "a fresh ledger was started");
    let quarantined = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("plenipo.db.corrupt-")
        });
    assert!(quarantined, "damaged file kept for recovery");
    // The new ledger works.
    new_task(&l, "after recovery");
}

#[test]
fn garbage_file_is_quarantined_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        b"this is not a sqlite database, just some bytes ".repeat(100),
    )
    .unwrap();
    assert_quarantined(&path);
}

#[test]
fn damaged_pages_are_quarantined_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    populated(&path);
    let mut bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() > 16 * 4096);
    // Keep the 100-byte header valid; scramble interior b-tree pages.
    for (i, b) in bytes.iter_mut().enumerate().skip(4096 * 2).take(4096 * 6) {
        *b = (i % 251) as u8;
    }
    std::fs::write(&path, bytes).unwrap();
    assert_quarantined(&path);
}

#[test]
fn truncated_file_is_quarantined_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    populated(&path);
    let len = std::fs::metadata(&path).unwrap().len();
    let f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    f.set_len(len / 3).unwrap();
    drop(f);
    assert_quarantined(&path);
}

// ---- Backup / export -------------------------------------------------------------------

#[test]
fn backup_is_consistent_verified_and_restorable() {
    let dir = tempfile::tempdir().unwrap();
    let path = db_path(dir.path());
    let l = Ledger::open(&path).unwrap();
    let t = new_task(&l, "backed up");
    l.transition_task(&t.id, TaskState::Running, "w", None)
        .unwrap();
    let info = l.backup(None).unwrap();
    assert!(info.verified);
    assert!(info.size_bytes > 0);
    assert_eq!(l.status().unwrap().last_backup.unwrap(), info);

    // Changes after the backup are not in it.
    l.transition_task(&t.id, TaskState::Succeeded, "w", None)
        .unwrap();
    let restored = Ledger::open(Path::new(&info.path)).unwrap();
    assert_eq!(
        restored.task(&t.id).unwrap().unwrap().state,
        TaskState::Running
    );
    assert_eq!(restored.events_for_task(&t.id).unwrap().len(), 2);
}

#[test]
fn backups_are_pruned_to_the_newest_ten() {
    let dir = tempfile::tempdir().unwrap();
    let l = Ledger::open(&db_path(dir.path())).unwrap();
    new_task(&l, "x");
    for _ in 0..13 {
        l.backup(None).unwrap();
    }
    let count = std::fs::read_dir(l.backups_dir().unwrap()).unwrap().count();
    assert_eq!(count, 10);
}

#[test]
fn json_export_contains_every_table() {
    let dir = tempfile::tempdir().unwrap();
    let l = Ledger::open(&db_path(dir.path())).unwrap();
    let t = new_task(&l, "exported");
    l.transition_task(&t.id, TaskState::Running, "w", None)
        .unwrap();
    let info = l.export_json(None).unwrap();
    let doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&info.path).unwrap()).unwrap();
    assert_eq!(doc["format"], "plenipo-ledger-export");
    assert_eq!(doc["schemaVersion"], migrate::latest(MIGRATIONS));
    for table in [
        "tasks",
        "events",
        "executions",
        "approvals",
        "artifacts",
        "roles",
        "departments",
        "projects",
        "agent_instances",
        "runtime_sessions",
        "liaison_messages",
    ] {
        assert!(doc["tables"][table].is_array(), "{table}");
    }
    assert_eq!(doc["tables"]["tasks"][0]["objective"], "exported");
    assert_eq!(doc["tables"]["events"].as_array().unwrap().len(), 2);
}

#[test]
fn integrity_check_reports_ok() {
    let dir = tempfile::tempdir().unwrap();
    let l = Ledger::open(&db_path(dir.path())).unwrap();
    new_task(&l, "healthy");
    let report = l.integrity_check().unwrap();
    assert!(report.ok);
    assert_eq!(report.messages, ["ok"]);
    assert_eq!(l.status().unwrap().last_integrity_check.unwrap(), report);
}
