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
}

impl Notice {
    fn new(kind: NoticeKind, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            body: body.into(),
        }
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
    )
}

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
                        "Its work waits or moves to a backup, as you chose in Settings → AI \
                         models.",
                    )),
                    _ => None,
                }
            }
            "lesson.added" if text(p, "state") == Some("waiting") => Some(Notice::new(
                NoticeKind::Lessons,
                format!(
                    "{} learned something",
                    text(p, "worker").unwrap_or("A worker")
                ),
                format!(
                    "{}\nKeep it or discard it on the Approvals page.",
                    line(p["text"].as_str().unwrap_or(""))
                ),
            )),
            _ => None,
        })
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
        Ok(Notice::new(
            NoticeKind::Approvals,
            format!("{worker} is waiting for your OK"),
            if production {
                format!("{summary} (PRODUCTION)")
            } else {
                summary
            },
        ))
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
        }
    } else {
        format!("{n} things need you")
    };
    // For one kind the titles repeat ("… is waiting for your OK"): list what each is about.
    let mut lines: Vec<String> = notices
        .iter()
        .take(MAX_LISTED)
        .map(|x| {
            if same && kind != NoticeKind::Problems {
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
        assert_eq!(
            l.notice_for(&push, &tool).unwrap(),
            Some(note(
                NoticeKind::Approvals,
                "Backend Developer is waiting for your OK",
                "Git push origin"
            ))
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
            json!({ "worker": "Senior Developer", "text": "Run the tests first.\nThey are quick.", "state": "waiting" }),
        );
        assert_eq!(
            l.notice_for(&waiting, &tool).unwrap(),
            Some(note(
                NoticeKind::Lessons,
                "Senior Developer learned something",
                "Run the tests first.\nKeep it or discard it on the Approvals page."
            ))
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
}
