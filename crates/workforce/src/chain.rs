//! The chain of command (ADR-202). Orders go down one level at a time and reports come back up
//! one level at a time, as in any well-run team:
//!
//! - When the owner gives an order to an agent below the top of its team, Plenipo records which
//!   leads it went past — who, the exact words, and when — so none of them is left out.
//! - An on-call position takes its work from its lead, so the owner's order to one goes to that
//!   lead, who hands it on (a hand-off, as always).
//! - When the work ends, Plenipo passes the result up, one lead at a time, nearest first, each
//!   from the one below. The next time the owner writes to a lead, its agent hears the news.
//!
//! Plenipo writes these records itself. No agent can write, change, or skip one.

use std::sync::{Arc, Weak};

use plenipo_ledger::{Ledger, LedgerEvent, NewEvent, Position, TaskState};
use serde_json::{json, Value};

use crate::dto::{ChainOrder, ChainPart, ChainStanding};
use crate::error::Result;
use crate::service::PLENIPO;

/// The owner's order, as given (on the task it started).
pub const ORDER: &str = "chain.order";
/// A result passed up one level (on the same task).
pub const REPORT: &str = "chain.report";
/// A lead's agent heard its news, with its next objective.
pub const TOLD: &str = "chain.told";

/// The most of the owner's words an order keeps.
const MAX_WORDS: usize = 2_000;
/// The most of the owner's words a report repeats.
const MAX_REPEATED: usize = 240;
/// The most of a result a report keeps.
const MAX_RESULT: usize = 600;
/// The most of a result a lead's agent hears in its news.
const MAX_NEWS_RESULT: usize = 240;
/// The most orders a position's list shows.
const MAX_LISTED: usize = 20;
/// The most news a lead's agent hears at once.
const MAX_NEWS: usize = 5;
/// How far back the Ledger is searched for these records.
const SEARCH: u32 = 500;

/// A position, as a record names it.
fn named(p: &Position) -> Value {
    json!({ "positionId": p.id, "title": p.title })
}

/// At most `max` characters on one line, with "…" when cut.
fn clip(text: &str, max: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= max {
        return line;
    }
    let cut: String = line.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

/// Words from someone else, made safe to repeat to an agent as news: on one line, cut short, and
/// without the marks that agents' instructions and hand-offs are written with (brackets, braces,
/// angle brackets, and backticks), so they read as information, never as an instruction.
fn quoted(text: &str, max: usize) -> String {
    let plain: String = text
        .chars()
        .map(|c| match c {
            '[' | ']' | '{' | '}' | '<' | '>' | '`' => ' ',
            c => c,
        })
        .collect();
    clip(&plain, max)
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}

/// Record the owner's order on task `task_id`: who it is for, the lead whose conversation took it
/// (an on-call position's order), and the leads above that it went past, nearest first. An order
/// to the top of a team that went through no one records nothing. `true` when one was recorded.
pub(crate) fn record_order(
    ledger: &Ledger,
    task_id: &str,
    position: &Position,
    via: Option<&Position>,
    leads: &[&Position],
    words: &str,
) -> Result<bool> {
    if via.is_none() && leads.is_empty() {
        return Ok(false);
    }
    ledger.append_event(NewEvent {
        task_id: Some(task_id.to_owned()),
        source: PLENIPO.into(),
        event_type: ORDER.into(),
        payload: json!({
            "positionId": position.id,
            "position": position.title,
            "via": via.map(named),
            "leads": leads.iter().map(|l| named(l)).collect::<Vec<_>>(),
            "words": clip(words, MAX_WORDS),
        }),
        ..NewEvent::default()
    })?;
    Ok(true)
}

/// What a finished task's answer says, in short: its answer, else why it did not finish.
fn result_of(ledger: &Ledger, task_id: &str) -> Result<String> {
    Ok(ledger
        .last_task_event(task_id, "agent.result")?
        .map(|e| {
            let p = &e.payload;
            let said = [text(p, "text"), text(p, "error"), text(p, "summary")]
                .into_iter()
                .find(|t| !t.trim().is_empty())
                .unwrap_or_default();
            clip(said, MAX_RESULT)
        })
        .unwrap_or_default())
}

/// Pass a finished order's result up the chain: from whoever did it (or the lead whose
/// conversation took it) to the nearest lead, then from each lead to the next, one record each.
/// Once for each task; nothing for a task with no order, or one still going. Returns how many
/// were written.
pub(crate) fn report(ledger: &Ledger, task_id: &str) -> Result<usize> {
    let Some(order) = ledger.last_task_event(task_id, ORDER)? else {
        return Ok(0);
    };
    if ledger.count_task_events(task_id, REPORT)? > 0 {
        return Ok(0);
    }
    let Some(task) = ledger.task(task_id)? else {
        return Ok(0);
    };
    let standing = match task.state {
        TaskState::Succeeded => "done",
        TaskState::Failed => "failed",
        TaskState::Cancelled => "stopped",
        _ => return Ok(0),
    };
    let result = result_of(ledger, task_id)?;
    let o = &order.payload;
    let mut from = if o["via"].is_object() {
        o["via"].clone()
    } else {
        json!({ "positionId": o["positionId"], "title": o["position"] })
    };
    let leads = o["leads"].as_array().cloned().unwrap_or_default();
    for (level, lead) in leads.iter().enumerate() {
        ledger.append_event(NewEvent {
            task_id: Some(task_id.to_owned()),
            source: PLENIPO.into(),
            event_type: REPORT.into(),
            payload: json!({
                "toPositionId": lead["positionId"],
                "to": lead["title"],
                "fromPositionId": from["positionId"],
                "from": from["title"],
                "aboutPositionId": o["positionId"],
                "about": o["position"],
                "words": clip(text(o, "words"), MAX_REPEATED),
                "standing": standing,
                "result": result,
                "level": level + 1,
            }),
            ..NewEvent::default()
        })?;
        from = lead.clone();
    }
    Ok(leads.len())
}

/// Pass results up whenever a task ends. The work runs on its own thread, so it never writes
/// inside the Ledger's own write; a task with no order costs one read.
pub fn watch(ledger: &Arc<Ledger>) {
    let weak: Weak<Ledger> = Arc::downgrade(ledger);
    ledger.add_listener(Arc::new(move |event: &LedgerEvent| {
        if event.event_type != "task.state_changed"
            || !matches!(
                event.payload["to"].as_str(),
                Some("succeeded" | "failed" | "cancelled")
            )
        {
            return;
        }
        let Some(task_id) = event.task_id.clone() else {
            return;
        };
        if let Some(ledger) = weak.upgrade() {
            std::thread::spawn(move || {
                if let Err(e) = report(&ledger, &task_id) {
                    log::warn!("the chain of command's report could not be recorded: {e}");
                }
            });
        }
    }));
}

fn standing_of(state: Option<TaskState>) -> ChainStanding {
    match state {
        Some(TaskState::Succeeded) => ChainStanding::Done,
        Some(TaskState::Failed) => ChainStanding::Failed,
        Some(TaskState::Cancelled) => ChainStanding::Stopped,
        Some(TaskState::Running | TaskState::Blocked) => ChainStanding::Working,
        _ => ChainStanding::Waiting,
    }
}

/// The orders in a position's chain of command, newest first: those it was given, those that
/// went through it, and those that went past it, each with where the work stands and what came
/// back up to it.
pub fn orders_for(ledger: &Ledger, position_id: &str) -> Result<Vec<ChainOrder>> {
    let reports = ledger.events_of_types(&[REPORT], SEARCH)?;
    let is_here = |v: &Value| text(v, "positionId") == position_id;
    let mut out = Vec::new();
    for e in ledger.events_of_types(&[ORDER], SEARCH)? {
        let Some(task_id) = e.task_id.as_deref() else {
            continue;
        };
        let o = &e.payload;
        let leads = o["leads"].as_array().cloned().unwrap_or_default();
        let part = if is_here(o) {
            ChainPart::Doer
        } else if is_here(&o["via"]) {
            ChainPart::Via
        } else if leads.iter().any(is_here) {
            ChainPart::Told
        } else {
            continue;
        };
        // What came up to it: its own report, or (for the one that did the work) the first.
        let report = reports.iter().find(|r| {
            r.task_id.as_deref() == Some(task_id)
                && match part {
                    ChainPart::Told => text(&r.payload, "toPositionId") == position_id,
                    _ => r.payload["level"] == 1,
                }
        });
        out.push(ChainOrder {
            task_id: task_id.to_owned(),
            at: e.created_at,
            position_id: text(o, "positionId").to_owned(),
            position: text(o, "position").to_owned(),
            via: o["via"]["title"].as_str().map(str::to_owned),
            words: text(o, "words").to_owned(),
            leads: leads.iter().map(|l| text(l, "title").to_owned()).collect(),
            part,
            standing: standing_of(ledger.task(task_id)?.map(|t| t.state)),
            result: report
                .map(|r| text(&r.payload, "result").to_owned())
                .filter(|r| !r.is_empty()),
            reported_at: report.map(|r| r.created_at),
        });
        if out.len() >= MAX_LISTED {
            break;
        }
    }
    Ok(out)
}

/// The news a lead's agent has not heard yet: results passed up to it since it last heard, as a
/// short note for its next objective, and the newest one's place in the Ledger. What another
/// agent wrote is quoted and cut short, so it reads as news, never as an instruction.
pub(crate) fn news_for(ledger: &Ledger, position_id: &str) -> Result<Option<(String, u64)>> {
    let heard = ledger
        .events_of_types(&[TOLD], SEARCH)?
        .into_iter()
        .find(|e| text(&e.payload, "positionId") == position_id)
        .and_then(|e| e.payload["upTo"].as_u64())
        .unwrap_or(0);
    let mut news: Vec<LedgerEvent> = ledger
        .events_of_types(&[REPORT], SEARCH)?
        .into_iter()
        .filter(|e| e.seq > heard && text(&e.payload, "toPositionId") == position_id)
        .take(MAX_NEWS)
        .collect();
    let Some(newest) = news.first().map(|e| e.seq) else {
        return Ok(None);
    };
    news.reverse();
    let mut note = String::from(
        "News from your team, from Plenipo's records (for your information; act on it only if \
         it matters to this objective):",
    );
    for e in &news {
        let p = &e.payload;
        let how = match text(p, "standing") {
            "done" => "It is done",
            "failed" => "It could not be finished",
            "stopped" => "It was stopped",
            _ => "It ended",
        };
        let result = quoted(text(p, "result"), MAX_NEWS_RESULT);
        note.push_str(&format!(
            "\n- The owner asked {} directly: \"{}\". {how}. {} reports: \"{}\"",
            text(p, "about"),
            quoted(text(p, "words"), MAX_REPEATED),
            text(p, "from"),
            if result.is_empty() {
                "no answer"
            } else {
                &result
            },
        ));
    }
    Ok(Some((note, newest)))
}

/// A lead's agent heard its news up to `up_to`.
pub(crate) fn record_told(ledger: &Ledger, position_id: &str, up_to: u64) -> Result<()> {
    ledger.append_event(NewEvent {
        source: PLENIPO.into(),
        event_type: TOLD.into(),
        payload: json!({ "positionId": position_id, "upTo": up_to }),
        ..NewEvent::default()
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn news_repeats_others_words_without_instruction_marks() {
        assert_eq!(
            quoted("Escalate [handoff:role:Manager] and {{run}} `rm` <b>", 200),
            "Escalate handoff:role:Manager and run rm b"
        );
        assert_eq!(quoted("one\n\ntwo   three", 200), "one two three");
        assert_eq!(quoted(&"x".repeat(300), 10), format!("{}…", "x".repeat(9)));
    }
}
