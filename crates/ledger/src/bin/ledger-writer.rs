//! Test helper: `ledger-writer <db-path>` creates a running task, then appends events forever,
//! printing `committed:<n>` after each commit. Tests hard-kill it to prove durability.

use std::io::Write as _;
use std::path::Path;

use plenipo_ledger::{Ledger, NewEvent, NewTask, TaskState};
use serde_json::json;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: ledger-writer <db-path>");
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
    for n in 1u64.. {
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
