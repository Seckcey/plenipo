//! Pop-up notices (Phase 12): what a committed event means for the owner, if anything, and the
//! owner's choices (Settings → Notifications). The desktop app shows the notices; this decides
//! what they say, and gathers those that arrive together into one.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::dto::LedgerEvent;
use crate::error::Result;
use crate::Ledger;

/// Where the owner's own choices are kept (with the terminal's shell).
pub const PREFERENCES: &str = "preferences";
const FIELD: &str = "notices";

/// The same notice is not shown again within this long.
pub const NOTICE_REPEAT_MS: u64 = 60_000;
/// Longest line of a notice.
const MAX_LINE: usize = 140;
/// Lines listed in a notice that stands for several.
const MAX_LISTED: usize = 3;

/// What a notice is about. The owner turns each kind on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoticeKind {
    /// A worker waits for the owner's OK.
    Approvals,
    /// A check that a person is using a website (a CAPTCHA), handed to the owner.
    Checks,
    /// An objective failed, a server's ID changed, or an AI tool signed out or reached its
    /// usage limit.
    Problems,
    /// An objective's result is ready.
    Finished,
    /// A worker learned something for the owner to keep or discard.
    Lessons,
    /// Plenipo itself: it recovered from closing unexpectedly, or a new version is ready
    /// (Phase 13).
    Plenipo,
    /// Spending on paid AI keys (Phase 16 Wave 3, ADR-085): 80% of a spending cap is used, or a
    /// cap stopped paid work.
    Spending,
}

/// The owner's choices for pop-up notices (Settings → Notifications).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct NoticeSettings {
    pub approvals: bool,
    pub checks: bool,
    pub problems: bool,
    pub finished: bool,
    pub lessons: bool,
    /// Plenipo itself: it closed unexpectedly, or a new version is ready (Phase 13).
    pub plenipo: bool,
    /// Spending caps: a warning at 80%, or paid work stopped (Phase 16 Wave 3).
    pub spending: bool,
    /// Only while Plenipo's window is not in front.
    pub only_when_away: bool,
}

impl Default for NoticeSettings {
    fn default() -> Self {
        Self {
            approvals: true,
            checks: true,
            problems: true,
            finished: true,
            lessons: true,
            plenipo: true,
            spending: true,
            only_when_away: true,
        }
    }
}

impl NoticeSettings {
    /// The owner wants notices of this kind.
    pub fn wants(&self, kind: NoticeKind) -> bool {
        match kind {
            NoticeKind::Approvals => self.approvals,
            NoticeKind::Checks => self.checks,
            NoticeKind::Problems => self.problems,
            NoticeKind::Finished => self.finished,
            NoticeKind::Lessons => self.lessons,
            NoticeKind::Plenipo => self.plenipo,
            NoticeKind::Spending => self.spending,
        }
    }
}

/// One pop-up notice: a short title and a line or two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub kind: NoticeKind,
    pub title: String,
    pub body: String,
    /// The one thing it is about, when it can be answered right from a phone's notice (Phase 14,
    /// ADR-144 §6): an approval or a lesson. Notices joined together have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<NoticeItem>,
}

/// What a notice is about, by its ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NoticeItem {
    Approval { id: String },
    Lesson { id: String },
}

impl Notice {
    fn new(kind: NoticeKind, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            body: body.into(),
            item: None,
        }
    }

    fn about(mut self, item: Option<NoticeItem>) -> Self {
        self.item = item;
        self
    }
}

/// Events that can mean a notice (the rest are never read twice).
pub fn may_notify(event_type: &str) -> bool {
    matches!(
        event_type,
        "approval.requested"
            | "task.state_changed"
            | "ssh.host_key_changed"
            | "agent.result"
            | "lesson.added"
            | "plenipo.recovered"
            | "plenipo.update_available"
            | "ai_tool.update_available"
            | "ai_tool.updated"
            | "ai_tool.update_failed"
            | "ai_tool.update_by_hand"
            | "spending.warning"
            | "spending.stopped"
            | "liaison.answer_sent_back"
    )
}

/// A week, for counting a worker's answers sent back (Phase 25, item 4.8).
const WEEK_MS: u64 = 7 * 24 * 60 * 60 * 1000;
/// The counts within a week at which the owner hears that a worker's answers keep not matching
/// Plenipo's record: the third, then the tenth.
const REPEAT_MISMATCHES: [u32; 2] = [3, 10];

fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v[key].as_str().map(str::trim).filter(|s| !s.is_empty())
}

/// The first line of `s`, at most [`MAX_LINE`] characters.
fn line(s: &str) -> String {
    let first = s.trim().lines().next().unwrap_or("").trim();
    if first.chars().count() > MAX_LINE {
        let cut: String = first.chars().take(MAX_LINE - 1).collect();
        format!("{cut}…")
    } else {
        first.to_owned()
    }
}

fn capitalized(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

/// Plenipo started again after an unclean end (Phase 13): what happened and what stopped.
fn recovered_notice(p: &Value) -> Notice {
    // "Windows closed Plenipo", "Your Mac closed Plenipo" (ADR-155); the cause keeps its name.
    let title = match text(p, "cause") {
        Some("windowsRestart") => format!(
            "{} closed Plenipo while it was running",
            plenipo_core::words::sentence_start(plenipo_core::WORDS.the_system)
        ),
        Some("layoutChange") => "Plenipo was stopped while updating the Ledger".to_owned(),
        Some("crash") => "Plenipo closed unexpectedly".to_owned(),
        _ => "Plenipo did not close normally".to_owned(),
    };
    let stopped = p["stoppedTasks"].as_array().map_or(0, Vec::len);
    let programs = p["stoppedPrograms"].as_u64().unwrap_or(0);
    let body = match stopped {
        0 if programs == 1 => "1 program that was running was stopped.".to_owned(),
        0 if programs > 1 => format!("{programs} programs that were running were stopped."),
        0 => "Nothing was running. Plenipo is running again.".to_owned(),
        1 => "1 task was stopped. Open Plenipo to run it again or leave it stopped.".to_owned(),
        n => {
            format!("{n} tasks were stopped. Open Plenipo to run them again or leave them stopped.")
        }
    };
    Notice::new(NoticeKind::Plenipo, title, body)
}

/// 80% of a spending cap is used this month.
fn spending_warning(p: &Value) -> Notice {
    let label = text(p, "label").unwrap_or("A spending cap");
    let spent = crate::spending::dollars(p["spentMicros"].as_u64().unwrap_or(0));
    let cap = crate::spending::dollars(p["capMicros"].as_u64().unwrap_or(0));
    Notice::new(
        NoticeKind::Spending,
        format!(
            "{}% of a spending cap is used",
            p["percent"].as_u64().unwrap_or(80)
        ),
        format!("{label}: {spent} of {cap} this month. Paid AI work stops at the cap."),
    )
}

/// A spending cap stopped paid work this month.
fn spending_stopped(p: &Value) -> Notice {
    let label = text(p, "label").unwrap_or("A spending cap");
    let why = if text(p, "why") == Some("full") {
        format!(
            "{label} used its whole cap ({}).",
            crate::spending::dollars(p["capMicros"].as_u64().unwrap_or(0))
        )
    } else if p["covers"]["kind"] == "business" {
        "A paid task did not fit under the business's cap.".to_owned()
    } else {
        format!("A paid task for {label} did not fit under its cap.")
    };
    Notice::new(
        NoticeKind::Spending,
        "Paid AI work stopped",
        format!("{why} Raise the cap in Settings → Spending caps, or wait for the new month."),
    )
}

/// "shop.example" from "https://shop.example/cart".
fn host(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().filter(|h| !h.is_empty())
}

impl Ledger {
    /// The owner's choices for pop-up notices (all on, and only while Plenipo is not in front,
    /// until the owner changes them).
    pub fn notice_settings(&self) -> Result<NoticeSettings> {
        Ok(self
            .setting(PREFERENCES)?
            .and_then(|v| serde_json::from_value(v[FIELD].clone()).ok())
            .unwrap_or_default())
    }

    /// Keep the owner's choices for pop-up notices.
    pub fn set_notice_settings(
        &self,
        settings: &NoticeSettings,
        actor: &str,
    ) -> Result<NoticeSettings> {
        self.merge_setting(PREFERENCES, &json!({ FIELD: settings }), actor)?;
        self.notice_settings()
    }

    /// What `event` means for the owner, if anything (Phase 12): a worker waiting for the
    /// owner's OK or with a check to solve, a problem, an objective finished, or a lesson to
    /// keep. Reads the approval, task, or position the event is about. `tool` names an AI tool
    /// from its ID ("claude-code" → "Claude Code").
    pub fn notice_for(
        &self,
        event: &LedgerEvent,
        tool: &dyn Fn(&str) -> String,
    ) -> Result<Option<Notice>> {
        let p = &event.payload;
        Ok(match event.event_type.as_str() {
            "approval.requested" => Some(self.approval_notice(event)?),
            "task.state_changed" => self.objective_notice(event)?,
            "ssh.host_key_changed" if text(p, "by") != Some("terminal") => Some(Notice::new(
                NoticeKind::Problems,
                "A server's ID changed",
                format!(
                    "Plenipo refused to connect to {}. Check its ID in Settings → Servers.",
                    text(p, "server").unwrap_or("a server")
                ),
            )),
            "plenipo.recovered" => Some(recovered_notice(p)),
            // Phase 19 (ADR-059 §2, §8): an AI tool's new version, once; and what an update did.
            "ai_tool.update_available" if p["automatic"] != json!(true) => {
                text(p, "newest").map(|v| {
                    let name = tool(text(p, "runtime").unwrap_or_default());
                    Notice::new(
                        NoticeKind::Plenipo,
                        format!("A new version of {name} is ready ({v})"),
                        "Update it on the AI tools page when you are ready. Plenipo updates it \
                         only when no task is using it.",
                    )
                })
            }
            "ai_tool.updated" => text(p, "to").map(|v| {
                let name = tool(text(p, "runtime").unwrap_or_default());
                Notice::new(
                    NoticeKind::Plenipo,
                    format!("{name} was updated to {v}"),
                    "Plenipo checked its version, its sign-in, and its models again.",
                )
            }),
            // An automatic update the tool could not do by itself (installed another way).
            "ai_tool.update_by_hand" if p["automatic"] == json!(true) => {
                let name = tool(text(p, "runtime").unwrap_or_default());
                Some(Notice::new(
                    NoticeKind::Plenipo,
                    format!("{name} can't update itself"),
                    text(p, "message").map_or_else(
                        || format!("See {name}'s card on the AI tools page."),
                        str::to_owned,
                    ),
                ))
            }
            "ai_tool.update_failed" => {
                let name = tool(text(p, "runtime").unwrap_or_default());
                let body = match (p["oldStillWorks"].as_bool(), text(p, "from")) {
                    (Some(true), Some(from)) => {
                        format!("The old version ({from}) still works.")
                    }
                    _ => format!(
                        "Plenipo is not giving {name} tasks for now. See the AI tools page."
                    ),
                };
                Some(Notice::new(
                    NoticeKind::Plenipo,
                    format!("{name}'s update didn't finish"),
                    body,
                ))
            }
            "plenipo.update_available" => text(p, "version").map(|v| {
                Notice::new(
                    NoticeKind::Plenipo,
                    format!("Plenipo {v} is ready to install"),
                    "Install it from Settings → Updates when you are ready. Nothing changes \
                     until you do.",
                )
            }),
            "agent.result" => {
                let name = tool(event.source.strip_prefix("agent:").unwrap_or(&event.source));
                match text(p, "outcome") {
                    Some("authRequired") => Some(Notice::new(
                        NoticeKind::Problems,
                        format!("{name} needs you to sign in"),
                        format!("Sign in to {name} again, and its workers can carry on."),
                    )),
                    Some("usageLimited") => Some(Notice::new(
                        NoticeKind::Problems,
                        format!("{name} reached its usage limit"),
                        "Plenipo picks its work back up when the limit is over. Open Plenipo \
                         to use a reset, use another AI tool, or leave the work stopped.",
                    )),
                    _ => None,
                }
            }
            "liaison.answer_sent_back" => self.repeat_mismatch_notice(event)?,
            // Phase 16 Wave 3 (ADR-085): once a month per cap, each.
            "spending.warning" => Some(spending_warning(p)),
            "spending.stopped" => Some(spending_stopped(p)),
            "lesson.added" if text(p, "state") == Some("waiting") => Some(
                Notice::new(
                    NoticeKind::Lessons,
                    format!(
                        "{} learned something",
                        text(p, "worker").unwrap_or("A worker")
                    ),
                    format!(
                        "{}\nKeep it or discard it on the Approvals page.",
                        line(p["text"].as_str().unwrap_or(""))
                    ),
                )
                .about(text(p, "lessonId").map(|id| NoticeItem::Lesson { id: id.to_owned() })),
            ),
            _ => None,
        })
    }

    /// A worker whose answers keep not matching Plenipo's record (Phase 25, item 4.8): on its
    /// third answer sent back in a week, and again on its tenth.
    fn repeat_mismatch_notice(&self, event: &LedgerEvent) -> Result<Option<Notice>> {
        let Some(task) = event
            .task_id
            .as_deref()
            .map(|id| self.task(id))
            .transpose()?
            .flatten()
        else {
            return Ok(None);
        };
        let Some(position_id) = task.metadata["workforce"]["positionId"].as_str() else {
            return Ok(None);
        };
        let n =
            self.answers_sent_back_since(position_id, event.created_at.saturating_sub(WEEK_MS))?;
        if !REPEAT_MISMATCHES.contains(&n) {
            return Ok(None);
        }
        let who = self
            .position(position_id)?
            .map_or_else(|| "A worker".to_owned(), |p| p.title);
        Ok(Some(Notice::new(
            NoticeKind::Problems,
            format!("{who}'s answers keep not matching the record"),
            format!(
                "{n} of its answers this week didn't match what Plenipo saw it do, and were sent \
                 back. Check its work, or give it another AI model."
            ),
        )))
    }

    fn approval_notice(&self, event: &LedgerEvent) -> Result<Notice> {
        // Guard's requests carry what the approval card shows; older ones are read back.
        let request = if event.payload["request"].is_object() {
            event.payload["request"].clone()
        } else {
            let id = event.payload["approvalId"].as_str().unwrap_or_default();
            match &event.task_id {
                Some(task) => self
                    .approvals_for_task(task)?
                    .into_iter()
                    .find(|a| a.id == id)
                    .map(|a| a.request_payload),
                None => None,
            }
            .unwrap_or(Value::Null)
        };
        let worker = text(&request, "worker").unwrap_or("A worker");
        if text(&request, "tool") == Some("browser_person_check") {
            let site = text(&request, "url").and_then(host).unwrap_or("a website");
            return Ok(Notice::new(
                NoticeKind::Checks,
                "A check for you to solve",
                format!("{worker} needs you to show {site} that a person is using it."),
            ));
        }
        let summary = text(&request, "summary").map_or_else(
            || "Open Plenipo to see what it asks.".to_owned(),
            |s| capitalized(&line(s)),
        );
        let production = text(&request, "environment") == Some("production");
        let item =
            text(&event.payload, "approvalId").map(|id| NoticeItem::Approval { id: id.to_owned() });
        Ok(Notice::new(
            NoticeKind::Approvals,
            format!("{worker} is waiting for your OK"),
            if production {
                format!("{summary} (PRODUCTION)")
            } else {
                summary
            },
        )
        .about(item))
    }

    /// An objective (a task nobody handed on) that failed or finished. Diagnostics' test tasks
    /// never make a notice.
    fn objective_notice(&self, event: &LedgerEvent) -> Result<Option<Notice>> {
        let to = event.payload["to"].as_str().unwrap_or_default();
        if to != "failed" && to != "succeeded" {
            return Ok(None);
        }
        let Some(task) = event
            .task_id
            .as_deref()
            .map(|id| self.task(id))
            .transpose()?
            .flatten()
        else {
            return Ok(None);
        };
        if task.parent_task_id.is_some() || task.metadata["synthetic"] == true {
            return Ok(None);
        }
        let who = match task.metadata["workforce"]["positionId"].as_str() {
            Some(id) => self.position(id)?.map(|p| p.title),
            None => None,
        };
        let objective = line(&task.objective);
        Ok(Some(if to == "failed" {
            let why = text(&event.payload, "reason").map(line);
            Notice::new(
                NoticeKind::Problems,
                "An objective failed",
                match (who, why) {
                    (Some(who), Some(why)) => format!("{who}: {objective}\n{why}"),
                    (Some(who), None) => format!("{who}: {objective}"),
                    (None, Some(why)) => format!("{objective}\n{why}"),
                    (None, None) => objective,
                },
            )
        } else {
            Notice::new(
                NoticeKind::Finished,
                "An objective is finished",
                match who {
                    Some(who) => format!("{who}: {objective}\nOpen Plenipo to see the result."),
                    None => format!("{objective}\nOpen Plenipo to see the result."),
                },
            )
        }))
    }
}

/// Shows notices the way the owner can take them in: those that arrive together become one,
/// and the same notice is not shown again within a minute ([`NOTICE_REPEAT_MS`]).
#[derive(Debug, Default)]
pub struct NoticeGate {
    shown: HashMap<(NoticeKind, String, String), u64>,
}

impl NoticeGate {
    /// The one notice to show for `notices` (arrived together) at `now`, if any is new.
    pub fn pass(&mut self, notices: Vec<Notice>, now: u64) -> Option<Notice> {
        self.shown
            .retain(|_, at| now.saturating_sub(*at) < NOTICE_REPEAT_MS);
        let mut fresh: Vec<Notice> = Vec::new();
        for n in notices {
            let key = (n.kind, n.title.clone(), n.body.clone());
            if self.shown.contains_key(&key) || fresh.contains(&n) {
                continue;
            }
            self.shown.insert(key, now);
            fresh.push(n);
        }
        combine(fresh)
    }
}

/// Several notices as one: counted by kind, with the first few listed.
pub fn combine(mut notices: Vec<Notice>) -> Option<Notice> {
    match notices.len() {
        0 => return None,
        1 => return notices.pop(),
        _ => {}
    }
    let n = notices.len();
    let kind = notices[0].kind;
    let same = notices.iter().all(|x| x.kind == kind);
    let title = if same {
        match kind {
            NoticeKind::Approvals => format!("{n} requests are waiting for your OK"),
            NoticeKind::Checks => format!("{n} checks for you to solve"),
            NoticeKind::Problems => format!("{n} problems need you"),
            NoticeKind::Finished => format!("{n} objectives are finished"),
            NoticeKind::Lessons => format!("Workers learned {n} things"),
            NoticeKind::Plenipo => format!("{n} things about Plenipo itself"),
            NoticeKind::Spending => format!("{n} things about paid AI spending"),
        }
    } else {
        format!("{n} things need you")
    };
    // For one kind the titles repeat ("… is waiting for your OK"): list what each is about.
    let mut lines: Vec<String> = notices
        .iter()
        .take(MAX_LISTED)
        .map(|x| {
            if same
                && !matches!(
                    kind,
                    NoticeKind::Problems | NoticeKind::Plenipo | NoticeKind::Spending
                )
            {
                line(x.body.lines().next().unwrap_or(""))
            } else {
                line(&x.title)
            }
        })
        .collect();
    if n > MAX_LISTED {
        lines.push(format!("and {} more", n - MAX_LISTED));
    }
    Some(Notice::new(
        if same { kind } else { NoticeKind::Problems },
        title,
        lines.join("\n"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{NewTask, TaskState};
    use crate::test_support::{ledger, task};

    fn new_task(l: &Ledger, objective: &str, parent: Option<&str>, metadata: Value) -> String {
        l.create_task(
            NewTask {
                parent_task_id: parent.map(str::to_owned),
                requested_by: "owner".into(),
                objective: objective.into(),
                priority: 2,
                metadata,
                ..NewTask::default()
            },
            "owner",
        )
        .unwrap()
        .id
    }

    fn tool(id: &str) -> String {
        match id {
            "claude-code" => "Claude Code".into(),
            other => other.into(),
        }
    }

    fn note(kind: NoticeKind, title: &str, body: &str) -> Notice {
        Notice::new(kind, title, body)
    }

    #[test]
    fn the_owners_choices_are_kept_with_the_other_preferences() {
        let l = ledger();
        assert_eq!(l.notice_settings().unwrap(), NoticeSettings::default());
        l.merge_setting(
            PREFERENCES,
            &json!({ "terminalShell": "commandPrompt" }),
            "owner",
        )
        .unwrap();
        let chosen = NoticeSettings {
            finished: false,
            only_when_away: false,
            ..NoticeSettings::default()
        };
        assert_eq!(l.set_notice_settings(&chosen, "owner").unwrap(), chosen);
        let kept = l.setting(PREFERENCES).unwrap().unwrap();
        assert_eq!(kept["terminalShell"], "commandPrompt");
        assert!(!chosen.wants(NoticeKind::Finished) && chosen.wants(NoticeKind::Approvals));
        // A choice added later reads as its default.
        l.merge_setting(
            PREFERENCES,
            &json!({ "notices": { "finished": false } }),
            "owner",
        )
        .unwrap();
        assert!(l.notice_settings().unwrap().only_when_away);
    }

    /// Phase 25, item 4.8: the owner hears when a worker's answers keep not matching Plenipo's
    /// record: on the third sent back in a week, and again on the tenth.
    #[test]
    fn a_worker_whose_answers_keep_not_matching_the_record_makes_a_notice() {
        let l = ledger();
        assert!(may_notify("liaison.answer_sent_back"));
        let send_back = |position: &str| {
            let t = new_task(
                &l,
                "Fix the login bug",
                None,
                json!({ "workforce": { "positionId": position } }),
            );
            l.append_event(crate::dto::NewEvent {
                task_id: Some(t),
                source: "liaison".into(),
                event_type: "liaison.answer_sent_back".into(),
                payload: json!({ "mismatches": ["says tests passed, but no test ran"] }),
                ..crate::dto::NewEvent::default()
            })
            .unwrap()
        };
        let mut notices = Vec::new();
        for _ in 0..10 {
            notices.push(l.notice_for(&send_back("p1"), &tool).unwrap());
        }
        // Another worker's answers count for it alone.
        assert_eq!(l.notice_for(&send_back("p2"), &tool).unwrap(), None);
        let at: Vec<usize> = notices
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.as_ref().map(|_| i + 1))
            .collect();
        assert_eq!(at, [3, 10]);
        assert_eq!(
            notices[2],
            Some(note(
                NoticeKind::Problems,
                "A worker's answers keep not matching the record",
                "3 of its answers this week didn't match what Plenipo saw it do, and were sent \
                 back. Check its work, or give it another AI model."
            ))
        );
        let counts = l.experience_counts().unwrap();
        assert_eq!(counts["p1"].answers_sent_back, 10);
        assert_eq!(counts["p2"].answers_sent_back, 1);
    }

    #[test]
    fn approvals_and_person_checks_name_the_worker_and_what_it_asks() {
        let l = ledger();
        let t = task(&l, "Ship the release");
        l.transition_task(&t.id, TaskState::Running, "worker", None)
            .unwrap();
        let ask = |payload: Value| {
            l.request_action_approval(&t.id, "git.write", &payload, u64::MAX, "agent:codex")
                .unwrap();
            l.events_for_task(&t.id)
                .unwrap()
                .into_iter()
                .rev()
                .find(|e| e.event_type == "approval.requested")
                .unwrap()
        };
        let push = ask(json!({ "worker": "Backend Developer", "summary": "git push origin" }));
        // It names the approval, so a phone's notice can answer it (Phase 14).
        let id = push.payload["approvalId"].as_str().unwrap().to_owned();
        assert_eq!(
            l.notice_for(&push, &tool).unwrap(),
            Some(
                note(
                    NoticeKind::Approvals,
                    "Backend Developer is waiting for your OK",
                    "Git push origin"
                )
                .about(Some(NoticeItem::Approval { id }))
            )
        );
        let restart = ask(json!({
            "worker": "Operations Engineer",
            "summary": "run systemctl restart nginx on Shop",
            "environment": "production",
        }));
        assert_eq!(
            l.notice_for(&restart, &tool).unwrap().unwrap().body,
            "Run systemctl restart nginx on Shop (PRODUCTION)"
        );
        let check = ask(json!({
            "worker": "Web Assistant",
            "tool": "browser_person_check",
            "url": "https://shop.example/checkout?step=2",
        }));
        assert_eq!(
            l.notice_for(&check, &tool).unwrap(),
            Some(note(
                NoticeKind::Checks,
                "A check for you to solve",
                "Web Assistant needs you to show shop.example that a person is using it."
            ))
        );
    }

    #[test]
    fn objectives_that_fail_or_finish_make_a_notice_but_tasks_handed_on_do_not() {
        let l = ledger();
        let root = new_task(
            &l,
            "Relaunch the website\nBefore the campaign.",
            None,
            Value::Null,
        );
        let child = new_task(&l, "Build the pricing page", Some(&root), Value::Null);
        let test = new_task(
            &l,
            "Synthetic diagnostic task #1",
            None,
            json!({ "synthetic": true }),
        );
        let changed = |task: &str, to: &str, reason: Option<&str>| LedgerEvent {
            seq: 1,
            id: "e".into(),
            task_id: Some(task.into()),
            execution_id: None,
            source: "plenipo".into(),
            destination: None,
            event_type: "task.state_changed".into(),
            payload: json!({ "from": "running", "to": to, "reason": reason }),
            created_at: 0,
        };
        assert_eq!(
            l.notice_for(&changed(&root, "failed", Some("the tests failed")), &tool)
                .unwrap(),
            Some(note(
                NoticeKind::Problems,
                "An objective failed",
                "Relaunch the website\nthe tests failed"
            ))
        );
        assert_eq!(
            l.notice_for(&changed(&root, "succeeded", None), &tool)
                .unwrap()
                .unwrap()
                .kind,
            NoticeKind::Finished
        );
        assert_eq!(
            l.notice_for(&changed(&child, "failed", None), &tool)
                .unwrap(),
            None
        );
        assert_eq!(
            l.notice_for(&changed(&test, "failed", None), &tool)
                .unwrap(),
            None
        );
        assert_eq!(
            l.notice_for(&changed(&root, "running", None), &tool)
                .unwrap(),
            None
        );
    }

    #[test]
    fn sign_in_usage_limits_server_ids_and_lessons_make_notices() {
        let l = ledger();
        let event = |source: &str, event_type: &str, payload: Value| LedgerEvent {
            seq: 1,
            id: "e".into(),
            task_id: None,
            execution_id: None,
            source: source.into(),
            destination: None,
            event_type: event_type.into(),
            payload,
            created_at: 0,
        };
        let signed_out = event(
            "agent:claude-code",
            "agent.result",
            json!({ "outcome": "authRequired" }),
        );
        assert_eq!(
            l.notice_for(&signed_out, &tool).unwrap().unwrap().title,
            "Claude Code needs you to sign in"
        );
        let limited = event(
            "agent:codex",
            "agent.result",
            json!({ "outcome": "usageLimited" }),
        );
        assert_eq!(
            l.notice_for(&limited, &tool).unwrap().unwrap().title,
            "codex reached its usage limit"
        );
        let done = event(
            "agent:codex",
            "agent.result",
            json!({ "outcome": "completed" }),
        );
        assert_eq!(l.notice_for(&done, &tool).unwrap(), None);
        let changed = event(
            "guard",
            "ssh.host_key_changed",
            json!({ "server": "Shop", "serverId": "s1" }),
        );
        assert_eq!(
            l.notice_for(&changed, &tool).unwrap().unwrap().body,
            "Plenipo refused to connect to Shop. Check its ID in Settings → Servers."
        );
        // The owner's own terminal already said so.
        let seen = event(
            "owner",
            "ssh.host_key_changed",
            json!({ "server": "Shop", "by": "terminal" }),
        );
        assert_eq!(l.notice_for(&seen, &tool).unwrap(), None);
        let waiting = event(
            "agent:codex",
            "lesson.added",
            json!({ "lessonId": "l1", "worker": "Senior Developer", "text": "Run the tests first.\nThey are quick.", "state": "waiting" }),
        );
        assert_eq!(
            l.notice_for(&waiting, &tool).unwrap(),
            Some(
                note(
                    NoticeKind::Lessons,
                    "Senior Developer learned something",
                    "Run the tests first.\nKeep it or discard it on the Approvals page."
                )
                .about(Some(NoticeItem::Lesson { id: "l1".into() }))
            )
        );
        let kept = event("agent:codex", "lesson.added", json!({ "state": "kept" }));
        assert_eq!(l.notice_for(&kept, &tool).unwrap(), None);
        assert!(may_notify("lesson.added") && !may_notify("ssh.output"));
    }

    #[test]
    fn notices_that_arrive_together_become_one_and_none_repeats_within_a_minute() {
        let mut gate = NoticeGate::default();
        let a = note(
            NoticeKind::Approvals,
            "Backend Developer is waiting for your OK",
            "Git push origin",
        );
        let b = note(
            NoticeKind::Approvals,
            "Web Assistant is waiting for your OK",
            "Send the form",
        );
        let c = note(
            NoticeKind::Finished,
            "An objective is finished",
            "Relaunch the website",
        );
        assert_eq!(gate.pass(vec![a.clone()], 0), Some(a.clone()));
        // The same one again within a minute: nothing new.
        assert_eq!(gate.pass(vec![a.clone()], 30_000), None);
        let both = gate.pass(vec![a.clone(), b.clone()], 40_000).unwrap();
        assert_eq!(both, b, "only the new one is left");
        let two = gate
            .pass(vec![a.clone(), b.clone()], 40_000 + NOTICE_REPEAT_MS)
            .unwrap();
        assert_eq!(two.title, "2 requests are waiting for your OK");
        // Joined, it is about no one thing: a phone's notice opens Plenipo instead of answering.
        assert_eq!(two.item, None);
        assert_eq!(two.body, "Git push origin\nSend the form");
        let mixed = combine(vec![a.clone(), b.clone(), c.clone(), c.clone(), a]).unwrap();
        assert_eq!(mixed.title, "5 things need you");
        assert_eq!(
            mixed.body,
            "Backend Developer is waiting for your OK\nWeb Assistant is waiting for your OK\nAn objective is finished\nand 2 more"
        );
        assert_eq!(combine(Vec::new()), None);
    }

    #[test]
    fn long_lines_are_cut_and_first_lines_kept() {
        assert_eq!(line("  one\ntwo"), "one");
        let long = "x".repeat(300);
        assert_eq!(line(&long).chars().count(), MAX_LINE);
        assert_eq!(host("https://a.example:8443/x?y"), Some("a.example:8443"));
        assert_eq!(capitalized("git push"), "Git push");
    }

    #[test]
    fn an_ai_tools_new_version_and_its_update_make_notices() {
        let l = ledger();
        let tool = |id: &str| {
            if id == "grok" {
                "Grok".to_owned()
            } else {
                id.to_owned()
            }
        };
        let event = |kind: &str, payload: Value| {
            l.append_event(crate::NewEvent {
                source: "plenipo".into(),
                event_type: kind.into(),
                payload,
                ..crate::NewEvent::default()
            })
            .unwrap()
        };
        let ready = event(
            "ai_tool.update_available",
            json!({ "runtime": "grok", "installed": "1.0.41", "newest": "1.0.43" }),
        );
        assert!(may_notify(&ready.event_type));
        let n = l.notice_for(&ready, &tool).unwrap().unwrap();
        assert_eq!(n.kind, NoticeKind::Plenipo);
        assert_eq!(n.title, "A new version of Grok is ready (1.0.43)");
        // With automatic updates on, the update itself says what happened.
        let automatic = event(
            "ai_tool.update_available",
            json!({ "runtime": "grok", "newest": "1.0.43", "automatic": true }),
        );
        assert_eq!(l.notice_for(&automatic, &tool).unwrap(), None);
        let updated = event(
            "ai_tool.updated",
            json!({ "runtime": "grok", "from": "1.0.41", "to": "1.0.43" }),
        );
        assert_eq!(
            l.notice_for(&updated, &tool).unwrap().unwrap().title,
            "Grok was updated to 1.0.43"
        );
        let failed = event(
            "ai_tool.update_failed",
            json!({ "runtime": "grok", "from": "1.0.41", "oldStillWorks": true }),
        );
        let n = l.notice_for(&failed, &tool).unwrap().unwrap();
        assert_eq!(n.title, "Grok's update didn't finish");
        assert_eq!(n.body, "The old version (1.0.41) still works.");
        // An automatic update the tool could not do by itself says what to type.
        let by_hand = event(
            "ai_tool.update_by_hand",
            json!({ "runtime": "grok", "newest": "1.0.43", "automatic": true,
                    "message": "Grok installed with npm updates with npm." }),
        );
        assert!(may_notify(&by_hand.event_type));
        let n = l.notice_for(&by_hand, &tool).unwrap().unwrap();
        assert_eq!(n.title, "Grok can't update itself");
        assert_eq!(n.body, "Grok installed with npm updates with npm.");
        // The owner pressed Update and sees the card: no notice.
        let pressed = event(
            "ai_tool.update_by_hand",
            json!({ "runtime": "grok", "newest": "1.0.43", "automatic": false }),
        );
        assert_eq!(l.notice_for(&pressed, &tool).unwrap(), None);
        let stopped = event(
            "ai_tool.update_failed",
            json!({ "runtime": "grok", "from": "1.0.41", "oldStillWorks": false }),
        );
        assert!(l
            .notice_for(&stopped, &tool)
            .unwrap()
            .unwrap()
            .body
            .contains("not giving Grok tasks"));
    }

    #[test]
    fn plenipo_says_when_it_recovered_and_when_a_new_version_is_ready() {
        let l = ledger();
        let tool = |id: &str| id.to_owned();
        let recovered = l
            .append_event(crate::NewEvent {
                source: "plenipo".into(),
                event_type: "plenipo.recovered".into(),
                payload: json!({ "cause": "windowsRestart", "stoppedTasks": ["a", "b"] }),
                ..crate::NewEvent::default()
            })
            .unwrap();
        assert!(may_notify(&recovered.event_type));
        let n = l.notice_for(&recovered, &tool).unwrap().unwrap();
        assert_eq!(n.kind, NoticeKind::Plenipo);
        assert_eq!(
            n.title,
            format!(
                "{} closed Plenipo while it was running",
                plenipo_core::words::sentence_start(plenipo_core::WORDS.the_system)
            )
        );
        if cfg!(windows) {
            assert_eq!(n.title, "Windows closed Plenipo while it was running");
        }
        assert!(n.body.starts_with("2 tasks were stopped."), "{}", n.body);
        // No task, but a program was running: it says so, not "nothing".
        let program = l
            .append_event(crate::NewEvent {
                source: "plenipo".into(),
                event_type: "plenipo.recovered".into(),
                payload: json!({ "cause": "crash", "stoppedTasks": [], "stoppedPrograms": 1 }),
                ..crate::NewEvent::default()
            })
            .unwrap();
        let n = l.notice_for(&program, &tool).unwrap().unwrap();
        assert_eq!(n.body, "1 program that was running was stopped.");
        let ready = l
            .append_event(crate::NewEvent {
                source: "plenipo".into(),
                event_type: "plenipo.update_available".into(),
                payload: json!({ "version": "1.10.0" }),
                ..crate::NewEvent::default()
            })
            .unwrap();
        let n = l.notice_for(&ready, &tool).unwrap().unwrap();
        assert_eq!(n.title, "Plenipo 1.10.0 is ready to install");
        // The owner can turn these off like any other kind.
        let mut settings = NoticeSettings::default();
        assert!(settings.wants(NoticeKind::Plenipo));
        settings.plenipo = false;
        assert!(!settings.wants(NoticeKind::Plenipo));
        // Settings kept by 1.8.0 (without the new choice) read with it on.
        let old: NoticeSettings = serde_json::from_value(json!({ "approvals": false })).unwrap();
        assert!(old.plenipo && !old.approvals);
    }

    #[test]
    fn a_spending_cap_warns_at_80_percent_and_says_when_paid_work_stopped() {
        use crate::spending::{Bill, CapCovers, PaidTask, PricedBy, MICROS_PER_DOLLAR as D};
        let l = ledger();
        let now = crate::spending::month_start_ms(2026, 10) + 1;
        l.set_spending_cap(&CapCovers::Business, 10 * D, "owner", now)
            .unwrap();
        let paid = |most: u64| PaidTask {
            runtime: "openrouter".into(),
            model: "moonshotai/kimi-k3".into(),
            most_micros: most,
            ..PaidTask::default()
        };
        let set = l.set_aside_spending(&paid(8 * D), now).unwrap().unwrap();
        l.settle_spending(
            &set.record_id,
            &Bill::Spent {
                micros: 8 * D,
                priced_by: PricedBy::Service,
            },
            now,
        )
        .unwrap();
        let find = |kind: &str| {
            l.recent_events(100)
                .unwrap()
                .into_iter()
                .find(|e| e.event_type == kind)
                .unwrap()
        };
        let warning = find("spending.warning");
        assert!(may_notify(&warning.event_type));
        let n = l.notice_for(&warning, &tool).unwrap().unwrap();
        assert_eq!(
            n,
            note(
                NoticeKind::Spending,
                "80% of a spending cap is used",
                "The business: $8.00 of $10.00 this month. Paid AI work stops at the cap."
            )
        );
        assert!(l.set_aside_spending(&paid(3 * D), now).unwrap().is_err());
        let n = l
            .notice_for(&find("spending.stopped"), &tool)
            .unwrap()
            .unwrap();
        assert_eq!(n.kind, NoticeKind::Spending);
        assert_eq!(n.title, "Paid AI work stopped");
        assert!(
            n.body
                .starts_with("A paid task did not fit under the business's cap."),
            "{}",
            n.body
        );
        // The owner can turn these off like any other kind.
        let off = NoticeSettings {
            spending: false,
            ..NoticeSettings::default()
        };
        assert!(!off.wants(NoticeKind::Spending));
        assert!(NoticeSettings::default().wants(NoticeKind::Spending));
    }
}
