//! The diagnostics file (Phase 13): one `.zip` the owner can save and send when something went
//! wrong. It holds what helps to understand a problem and nothing of the owner's work:
//!
//! - `about.json`: the version, Windows, the Ledger's health and its backups, how the last run
//!   ended, the AI tools found (names, versions, ready or not), Plenipo's settings for starting
//!   and closing, and the notices Plenipo raised;
//! - `recent-events.json`: what happened when (event type, time, and where it came from), never
//!   what an event said;
//! - `logs/`: Plenipo's log files.
//!
//! No task text, no worker answers, no program output, nothing typed in the terminal, and no
//! secrets: every text in it passes the secret filter (the owner's stored secrets and anything
//! that looks like a key), and the Ledger database itself is not included.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use plenipo_core::DiagnosticsFile;
use plenipo_ledger::LedgerEvent;
use serde_json::{json, Value};

use crate::logs::Filter;

/// How many diagnostics files are kept.
pub const KEEP_FILES: usize = 5;
/// How many recent events it lists.
pub const EVENTS: u32 = 500;
const PREFIX: &str = "plenipo-diagnostics-";

const README: &str = "Plenipo diagnostics

This file helps someone understand a problem with Plenipo. It holds:

- about.json: this version of Plenipo, Windows, the health of the Ledger and its backups, how
  Plenipo last stopped, the AI tools it found, and the notices it showed.
- recent-events.json: what happened and when (the kind of event only, never what it said).
- logs: Plenipo's own log files.

It does not hold your tasks, your workers' answers, what programs printed, anything you typed in
the terminal, your Ledger, or your secrets. Secrets that look like keys or passwords, and the
secrets you stored in Plenipo, are hidden by Plenipo in every file.
";

/// What goes into `about.json`, gathered by the caller.
pub struct Contents {
    pub about: Value,
    pub events: Vec<LedgerEvent>,
    pub logs: Vec<PathBuf>,
}

/// An event without what it said: its kind, time, and source.
fn event_line(e: &LedgerEvent) -> Value {
    json!({
        "seq": e.seq,
        "at": e.created_at,
        "type": e.event_type,
        "source": e.source,
        "task": e.task_id.is_some(),
    })
}

/// Hide secrets in every string of `value`.
fn filtered(value: &Value, filter: &Filter) -> Value {
    match value {
        Value::String(s) => Value::String(filter(s)),
        Value::Array(items) => Value::Array(items.iter().map(|v| filtered(v, filter)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), filtered(v, filter)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn file_stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Write the diagnostics file into `dir`, keeping only the newest [`KEEP_FILES`].
pub fn write(dir: &Path, contents: &Contents, filter: &Filter) -> std::io::Result<DiagnosticsFile> {
    std::fs::create_dir_all(dir)?;
    let mut path = dir.join(format!("{PREFIX}{}.zip", file_stamp()));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{PREFIX}{}-{n}.zip", file_stamp()));
        n += 1;
    }
    let tmp = path.with_extension("zip.part");
    let written = write_zip(&tmp, contents, filter);
    let entries = match written {
        Ok(entries) => entries,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    };
    std::fs::rename(&tmp, &path)?;
    prune(dir);
    Ok(DiagnosticsFile {
        size_bytes: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
        path: path.display().to_string(),
        created_at: plenipo_ledger::now_ms(),
        contents: entries,
    })
}

fn write_zip(path: &Path, contents: &Contents, filter: &Filter) -> std::io::Result<Vec<String>> {
    let file = std::fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut entries = Vec::new();
    let mut add = |name: &str, bytes: &[u8]| -> std::io::Result<()> {
        zip.start_file(name, options)
            .map_err(std::io::Error::other)?;
        zip.write_all(bytes)?;
        entries.push(name.to_owned());
        Ok(())
    };
    add("README.txt", README.as_bytes())?;
    let about = filtered(&contents.about, filter);
    add(
        "about.json",
        &serde_json::to_vec_pretty(&about).map_err(std::io::Error::other)?,
    )?;
    let events: Vec<Value> = contents.events.iter().map(event_line).collect();
    add(
        "recent-events.json",
        &serde_json::to_vec_pretty(&filtered(&Value::Array(events), filter))
            .map_err(std::io::Error::other)?,
    )?;
    for log in &contents.logs {
        let Some(name) = log.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Lines were filtered when written; filter them again with today's secrets.
        let text = std::fs::read_to_string(log).unwrap_or_default();
        let text: String = text.lines().map(|l| filter(l) + "\n").collect();
        add(&format!("logs/{name}"), text.as_bytes())?;
    }
    zip.finish().map_err(std::io::Error::other)?;
    Ok(entries)
}

fn prune(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(PREFIX) && n.ends_with(".zip"))
        })
        .collect();
    files.sort();
    let excess = files.len().saturating_sub(KEEP_FILES);
    for old in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;
    use std::sync::Arc;

    fn read_zip(path: &str) -> Vec<(String, String)> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        (0..zip.len())
            .map(|i| {
                let mut f = zip.by_index(i).unwrap();
                let mut text = String::new();
                f.read_to_string(&mut text).unwrap();
                (f.name().to_owned(), text)
            })
            .collect()
    }

    fn event(event_type: &str, payload: Value) -> LedgerEvent {
        LedgerEvent {
            seq: 7,
            id: "e".into(),
            task_id: Some("t".into()),
            execution_id: None,
            source: "agent:claude-code".into(),
            destination: None,
            event_type: event_type.into(),
            payload,
            created_at: 1,
        }
    }

    #[test]
    fn the_file_holds_what_helps_and_nothing_of_the_owners_work_or_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("plenipo.log");
        std::fs::write(&log, "a line\nthe server key is hunter2-owner-secret\n").unwrap();
        let contents = Contents {
            about: json!({
                "version": "1.9.0",
                "notices": ["the push said ghp_abcdefghijklmnopqrstuvwxyz0123456789"],
            }),
            events: vec![event(
                "agent.message",
                json!({ "text": "Here is the confidential customer list" }),
            )],
            logs: vec![log],
        };
        // The owner's stored secret, and recognizable ones.
        let redactor = plenipo_guard::redact::Redactor::new(vec![(
            "hunter2-owner-secret".to_owned(),
            "Server password".to_owned(),
        )]);
        let filter: Filter = Arc::new(move |t: &str| redactor.redact(t).into_owned());
        let out = dir.path().join("out");
        let file = write(&out, &contents, &filter).unwrap();
        assert!(file.size_bytes > 0);
        assert_eq!(
            file.contents,
            [
                "README.txt",
                "about.json",
                "recent-events.json",
                "logs/plenipo.log"
            ]
        );
        let all: String = read_zip(&file.path)
            .into_iter()
            .map(|(name, text)| format!("== {name}\n{text}"))
            .collect();
        assert!(all.contains("\"version\": \"1.9.0\""));
        assert!(all.contains("agent.message"), "the kind of event is kept");
        assert!(
            !all.contains("confidential customer list"),
            "never what it said"
        );
        assert!(
            !all.contains("hunter2-owner-secret"),
            "stored secrets are hidden"
        );
        assert!(!all.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"));
        assert!(all.contains(plenipo_guard::redact::MARKER));
    }

    #[test]
    fn only_the_newest_files_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        for n in 0..8 {
            std::fs::write(
                dir.path().join(format!("{PREFIX}2026010{n}-000000.zip")),
                b"x",
            )
            .unwrap();
        }
        std::fs::write(dir.path().join("other.zip"), b"x").unwrap();
        prune(dir.path());
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left.len(), KEEP_FILES + 1);
        assert!(left.contains(&"other.zip".to_owned()));
        assert!(!left.contains(&format!("{PREFIX}20260100-000000.zip")));
    }
}
