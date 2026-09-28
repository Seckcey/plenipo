//! Test helper, hard-killed by tests:
//!
//! - `ledger-writer <db-path>` creates a running task, then appends events forever, printing
//!   `committed:<n>` after each commit (durability).
//! - `ledger-writer migrate <db-path>` opens the Ledger with one more, very slow layout change
//!   (a migration), printing `migrating` first (an interrupted migration, Phase 13).

use std::io::Write as _;
use std::path::Path;

use plenipo_ledger::migrate::{self, Migration};
use plenipo_ledger::{Ledger, NewEvent, NewTask, TaskState, MIGRATIONS};
use serde_json::json;

/// A layout change that takes far longer than any test waits.
const SLOW: &str = "CREATE TABLE slow_migration_filler (n INTEGER, pad TEXT);
    WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 500000000)
    INSERT INTO slow_migration_filler SELECT n, 'padding padding padding padding' FROM c;";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("migrate") {
        let path = args.get(2).expect("usage: ledger-writer migrate <db-path>");
        let mut all = MIGRATIONS.to_vec();
        all.push(Migration {
            version: migrate::latest(MIGRATIONS) + 1,
            name: "slow_test_migration",
            up: SLOW,
            down: "DROP TABLE slow_migration_filler;",
        });
        println!("migrating");
        let _ = Ledger::open_with(Path::new(path), &all);
        println!("migrated");
        return;
    }
    let path = args.get(1).expect("usage: ledger-writer <db-path>");
    let ledger = Ledger::open(Path::new(&path)).expect("open ledger");
    let task = ledger
        .create_task(
            NewTask {
                requested_by: "crash-test".into(),
                objective: "Synthetic task written by a process that will be killed".into(),
                ..NewTask::default()
            },
            "crash-test",
        )
        .expect("create task");
    ledger
        .transition_task(&task.id, TaskState::Running, "crash-test", None)
        .expect("start task");
    let mut out = std::io::stdout().lock();
    writeln!(out, "task:{}", task.id).unwrap();
    out.flush().unwrap();
    for n in 1u64..=u64::MAX {
        ledger
            .append_event(NewEvent {
                task_id: Some(task.id.clone()),
                source: "crash-test".into(),
                event_type: "synthetic.tick".into(),
                payload: json!({ "n": n }),
                ..NewEvent::default()
            })
            .expect("append");
        writeln!(out, "committed:{n}").unwrap();
        out.flush().unwrap();
    }
}
