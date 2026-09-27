//! The result of an objective (Phase 8, ADR-016): what Plenipo's own records say happened for an
//! objective and everything handed on from it — not what an agent says. It answers the plan's
//! list: tasks performed, workers and AI models used, files changed, tests run, branch, commits
//! and pull request, review findings still open, and approvals still needed.
//!
//! [`build`] is a pure function of Ledger records, so the result is the same however often it is
//! read, and live while the objective runs.

use std::collections::{BTreeMap, HashMap};

use plenipo_ledger::{
    Approval, ApprovalState, CommitInfo, LedgerEvent, Task, TaskState, Workspace, WorkspaceState,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::view::OrgView;

/// Longest answer kept in the result (bytes).
const MAX_ANSWER_BYTES: usize = 16 * 1024;
/// Most findings read from one review.
const MAX_FINDINGS: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ObjectiveReport {
    pub root_task_id: String,
    pub correlation_id: Option<String>,
    pub objective: String,
    /// Who was given the objective.
    pub position_id: Option<String>,
    pub position_title: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub state: TaskState,
    #[ts(type = "number")]
    pub created_at: u64,
    #[ts(type = "number | null")]
    pub completed_at: Option<u64>,
    /// The final answer of whoever was given the objective (a VP's report to the owner).
    pub answer: Option<String>,
    /// Every task: the objective's own, then those handed on, in order.
    pub tasks: Vec<ReportTask>,
    /// The workers and AI models that did them.
    pub workers: Vec<ReportWorker>,
    pub files: Vec<ReportFile>,
    /// Programs run (tests among them).
    pub checks: Vec<ReportCheck>,
    pub branches: Vec<ReportBranch>,
    pub pull_requests: Vec<ReportPullRequest>,
    /// Every verdict given, oldest first.
    pub reviews: Vec<ReportReview>,
    /// The findings of each reviewer's latest verdict.
    pub findings: Vec<ReportFinding>,
    pub approvals: Vec<ReportApproval>,
    pub blocked: Vec<ReportBlocked>,
    /// Things the owner should know: failed tasks, refused requests, work left uncommitted,
    /// branches to merge.
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportTask {
    pub task_id: String,
    pub parent_task_id: Option<String>,
    /// 0 for the objective's own task.
    pub depth: u32,
    /// The position that did it ("You" never does; a worker outside the organization is named
    /// by its AI tool).
    pub who: String,
    pub role: Option<String>,
    pub objective: String,
    pub state: TaskState,
    pub runtime_label: Option<String>,
    pub model: Option<String>,
    #[ts(type = "number | null")]
    pub started_at: Option<u64>,
    #[ts(type = "number | null")]
    pub completed_at: Option<u64>,
    /// Its result, in one line.
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportWorker {
    pub who: String,
    pub role: Option<String>,
    pub runtime_label: Option<String>,
    pub model: Option<String>,
    pub tasks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportFile {
    pub path: String,
    pub added: Option<u32>,
    pub removed: Option<u32>,
    /// In a commit on its branch; otherwise changed but not committed.
    pub committed: bool,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportCheck {
    pub task_id: String,
    pub who: String,
    /// The command line (or "a PowerShell script (…)").
    pub command: String,
    /// It ended successfully.
    pub ok: bool,
    /// It looks like a test or check (`cargo test`, `npm test`, `pytest`, `./check.sh`, …).
    pub test: bool,
    #[ts(type = "number")]
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportBranch {
    pub workspace_id: String,
    pub branch: String,
    pub base_ref: Option<String>,
    /// Newest first.
    pub commits: Vec<CommitInfo>,
    pub uncommitted: u32,
    pub pushed: bool,
    /// The working copy's folder (gone once removed; the branch stays).
    pub path: String,
    pub removed: bool,
    /// A second working copy's branch is to be merged into this one.
    pub merge_into: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportPullRequest {
    pub url: String,
    #[ts(type = "number")]
    pub number: u64,
    pub task_id: String,
    pub who: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ReviewVerdict {
    Approve,
    RequestChanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportReview {
    pub task_id: String,
    pub who: String,
    pub verdict: ReviewVerdict,
    pub findings: u32,
    #[ts(type = "number")]
    pub at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Severity {
    Blocker,
    Major,
    Minor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportFinding {
    pub who: String,
    pub severity: Severity,
    pub file: Option<String>,
    pub summary: String,
    /// The reviewer asked for changes and it is a blocker or major.
    pub blocking: bool,
    pub task_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReportApprovalState {
    Waiting,
    Approved,
    Denied,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportApproval {
    pub approval_id: String,
    pub task_id: String,
    pub who: String,
    pub summary: String,
    pub state: ReportApprovalState,
    #[ts(type = "number")]
    pub requested_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportBlocked {
    pub task_id: String,
    pub who: String,
    pub summary: String,
    pub reason: String,
    #[ts(type = "number")]
    pub at: u64,
}

/// One review read from an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub verdict: ReviewVerdict,
    pub findings: Vec<(Severity, Option<String>, String)>,
}

/// Everything [`build`] reads.
pub struct Inputs<'a> {
    pub root: &'a Task,
    /// The rest of the tree, with depth (children 1).
    pub descendants: &'a [(Task, u32)],
    /// Each task's events, by task ID.
    pub events: &'a HashMap<String, Vec<LedgerEvent>>,
    /// Each task's approvals, by task ID.
    pub approvals: &'a HashMap<String, Vec<Approval>>,
    pub workspaces: &'a [Workspace],
    pub view: &'a OrgView<'a>,
    /// Runtime ID → label ("Claude Code").
    pub runtime_labels: &'a HashMap<String, String>,
}

fn first_line(text: &str, max: usize) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.chars().count() <= max {
        line.to_owned()
    } else {
        let mut out: String = line.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn cap_bytes(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max.saturating_sub(3);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Every `plenipo-review` block in an answer (unreadable ones are skipped).
pub fn reviews_in(text: &str) -> Vec<Review> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix("```") else {
            continue;
        };
        if rest.trim() != "plenipo-review" {
            continue;
        }
        let body: Vec<&str> = lines
            .by_ref()
            .take_while(|l| l.trim_start() != "```")
            .collect();
        if let Some(r) = read_review(&body.join("\n")) {
            out.push(r);
        }
    }
    out
}

fn read_review(json: &str) -> Option<Review> {
    let v: Value = serde_json::from_str(json).ok()?;
    let verdict = match v["verdict"]
        .as_str()?
        .trim()
        .to_lowercase()
        .replace([' ', '_'], "-")
        .as_str()
    {
        "approve" | "approved" | "pass" | "passed" => ReviewVerdict::Approve,
        "request-changes" | "changes-requested" | "reject" | "fail" | "failed" => {
            ReviewVerdict::RequestChanges
        }
        _ => return None,
    };
    let findings = v["findings"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .take(MAX_FINDINGS)
                .filter_map(|f| {
                    let summary = first_line(f["summary"].as_str()?, 300);
                    if summary.is_empty() {
                        return None;
                    }
                    let severity = match f["severity"].as_str().unwrap_or("minor") {
                        "blocker" | "critical" => Severity::Blocker,
                        "major" | "high" => Severity::Major,
                        _ => Severity::Minor,
                    };
                    let file = f["file"]
                        .as_str()
                        .map(|p| first_line(p, 300))
                        .filter(|p| !p.is_empty());
                    Some((severity, file, summary))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Review { verdict, findings })
}

/// A command line that looks like a test or check.
pub fn is_test(command: &str) -> bool {
    const WORDS: [&str; 12] = [
        "test", "tests", "pytest", "jest", "vitest", "unittest", "spec", "check", "verify",
        "ctest", "nextest", "e2e",
    ];
    command
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || matches!(c, '/' | '\\' | '.' | ':' | '-' | '_'))
        .any(|w| WORDS.contains(&w))
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

/// The final result recorded for a task (its last step's).
fn last_result(events: &[LedgerEvent]) -> Option<&LedgerEvent> {
    events.iter().rev().find(|e| e.event_type == "agent.result")
}

pub fn build(i: &Inputs<'_>) -> ObjectiveReport {
    let no_events = Vec::new();
    let events_of = |id: &str| i.events.get(id).unwrap_or(&no_events);
    let all: Vec<(&Task, u32)> = std::iter::once((i.root, 0))
        .chain(i.descendants.iter().map(|(t, d)| (t, *d)))
        .collect();
    // Who did each task, with their role.
    let who = |t: &Task| -> (String, Option<String>) {
        let position = t.metadata["workforce"]["positionId"]
            .as_str()
            .and_then(|p| i.view.position(p));
        match position {
            Some(p) => (p.title.clone(), i.view.role(p).map(|r| r.name.clone())),
            None => {
                let runtime = t.metadata["runtimeId"].as_str().unwrap_or("");
                let label = i
                    .runtime_labels
                    .get(runtime)
                    .cloned()
                    .unwrap_or_else(|| runtime.to_owned());
                (format!("Worker on {label}"), None)
            }
        }
    };
    let mut tasks = Vec::new();
    let mut workers: Vec<ReportWorker> = Vec::new();
    let mut checks = Vec::new();
    let mut pull_requests = Vec::new();
    let mut reviews = Vec::new();
    // Reviewer → its latest review (task, at, review).
    let mut latest: BTreeMap<String, (String, u64, Review)> = BTreeMap::new();
    let mut blocked = Vec::new();
    let mut approvals = Vec::new();
    let mut problems = Vec::new();
    for (t, depth) in &all {
        let events = events_of(&t.id);
        let (name, role) = who(t);
        let result = last_result(events);
        let runtime_label = t.metadata["runtimeId"].as_str().map(|r| {
            i.runtime_labels
                .get(r)
                .cloned()
                .unwrap_or_else(|| r.to_owned())
        });
        let model = t.metadata["model"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| result.and_then(|e| str_of(&e.payload["model"])));
        tasks.push(ReportTask {
            task_id: t.id.clone(),
            parent_task_id: t.parent_task_id.clone(),
            depth: *depth,
            who: name.clone(),
            role: role.clone(),
            objective: first_line(&t.objective, 200),
            state: t.state,
            runtime_label: runtime_label.clone(),
            model: model.clone(),
            started_at: t.started_at,
            completed_at: t.completed_at,
            summary: result.and_then(|e| str_of(&e.payload["summary"])),
        });
        if t.started_at.is_some() {
            match workers
                .iter_mut()
                .find(|w| w.who == name && w.runtime_label == runtime_label && w.model == model)
            {
                Some(w) => w.tasks += 1,
                None => workers.push(ReportWorker {
                    who: name.clone(),
                    role: role.clone(),
                    runtime_label: runtime_label.clone(),
                    model: model.clone(),
                    tasks: 1,
                }),
            }
        }
        if matches!(t.state, TaskState::Failed | TaskState::Cancelled) {
            let why = result
                .and_then(|e| str_of(&e.payload["summary"]))
                .or_else(|| {
                    events
                        .iter()
                        .rev()
                        .find(|e| e.event_type == "liaison.dispatch_failed")
                        .and_then(|e| str_of(&e.payload["reason"]))
                })
                .unwrap_or_default();
            problems.push(format!(
                "{name}'s task {} ({}){}",
                if t.state == TaskState::Failed {
                    "failed"
                } else {
                    "was cancelled"
                },
                first_line(&t.objective, 80),
                if why.is_empty() {
                    String::new()
                } else {
                    format!(": {}", first_line(&why, 200))
                }
            ));
        }
        for e in events {
            let p = &e.payload;
            match e.event_type.as_str() {
                "capability.used" => {
                    let capability = p["capability"].as_str().unwrap_or("");
                    if matches!(capability, "shell.exec" | "powershell.exec") {
                        let summary = p["summary"].as_str().unwrap_or("");
                        let command = summary.strip_prefix("run ").unwrap_or(summary).to_owned();
                        checks.push(ReportCheck {
                            task_id: t.id.clone(),
                            who: name.clone(),
                            test: is_test(&command),
                            command,
                            ok: p["ok"].as_bool().unwrap_or(false),
                            at: e.created_at,
                        });
                    }
                    if let (Some(url), Some(number)) = (
                        p["pullRequest"]["url"].as_str(),
                        p["pullRequest"]["number"].as_u64(),
                    ) {
                        pull_requests.push(ReportPullRequest {
                            url: url.to_owned(),
                            number,
                            task_id: t.id.clone(),
                            who: name.clone(),
                        });
                    }
                }
                "guard.denied" => blocked.push(ReportBlocked {
                    task_id: t.id.clone(),
                    who: name.clone(),
                    summary: p["summary"].as_str().unwrap_or("").to_owned(),
                    reason: p["reason"].as_str().unwrap_or("").to_owned(),
                    at: e.created_at,
                }),
                "liaison.handoff_rejected" => problems.push(format!(
                    "A request from {name} was refused: {}",
                    first_line(p["reason"].as_str().unwrap_or(""), 200)
                )),
                "agent.result" => {
                    let text = p["text"].as_str().unwrap_or("");
                    for review in reviews_in(text) {
                        reviews.push(ReportReview {
                            task_id: t.id.clone(),
                            who: name.clone(),
                            verdict: review.verdict,
                            findings: u32::try_from(review.findings.len()).unwrap_or(u32::MAX),
                            at: e.created_at,
                        });
                        let newer = latest
                            .get(&name)
                            .is_none_or(|(_, at, _)| e.created_at >= *at);
                        if newer {
                            latest.insert(name.clone(), (t.id.clone(), e.created_at, review));
                        }
                    }
                }
                _ => {}
            }
        }
        for a in i.approvals.get(&t.id).into_iter().flatten() {
            let p = &a.request_payload;
            approvals.push(ReportApproval {
                approval_id: a.id.clone(),
                task_id: t.id.clone(),
                who: p["worker"]
                    .as_str()
                    .map_or_else(|| name.clone(), str::to_owned),
                summary: p["summary"].as_str().unwrap_or(&a.action_type).to_owned(),
                state: match a.state {
                    ApprovalState::Pending => ReportApprovalState::Waiting,
                    ApprovalState::Approved => ReportApprovalState::Approved,
                    ApprovalState::Rejected => ReportApprovalState::Denied,
                    ApprovalState::Expired => ReportApprovalState::Expired,
                },
                requested_at: a.requested_at,
            });
        }
    }
    approvals.sort_by_key(|a| a.requested_at);
    let mut findings = Vec::new();
    for (reviewer, (task_id, _, review)) in &latest {
        for (severity, file, summary) in &review.findings {
            findings.push(ReportFinding {
                who: reviewer.clone(),
                severity: *severity,
                file: file.clone(),
                summary: summary.clone(),
                blocking: review.verdict == ReviewVerdict::RequestChanges
                    && *severity != Severity::Minor,
                task_id: task_id.clone(),
            });
        }
        if review.verdict == ReviewVerdict::RequestChanges {
            problems.push(format!("{reviewer}'s latest review requests changes."));
        }
    }
    findings.sort_by_key(|f| (!f.blocking, f.severity));
    // Branches, and the files on them.
    let mut branches = Vec::new();
    let mut files = Vec::new();
    for w in i.workspaces {
        let merge_into = w.parent_id.as_ref().and_then(|p| {
            i.workspaces
                .iter()
                .find(|m| &m.id == p)
                .map(|m| m.branch.clone())
        });
        if let Some(into) = &merge_into {
            problems.push(format!(
                "{} was worked on at the same time as {into}: merge it into {into}.",
                w.branch
            ));
        }
        if w.facts.uncommitted > 0 && w.state == WorkspaceState::Active {
            problems.push(format!(
                "{} changed file(s) on {} are not committed yet.",
                w.facts.uncommitted, w.branch
            ));
        }
        for f in &w.facts.files {
            files.push(ReportFile {
                path: f.path.clone(),
                added: f.added,
                removed: f.removed,
                committed: f.committed,
                branch: w.branch.clone(),
            });
        }
        branches.push(ReportBranch {
            workspace_id: w.id.clone(),
            branch: w.branch.clone(),
            base_ref: w.base_ref.clone(),
            commits: w.facts.commits.clone(),
            uncommitted: w.facts.uncommitted,
            pushed: w.facts.pushed,
            path: w.path.clone(),
            removed: w.state == WorkspaceState::Removed,
            merge_into,
        });
    }
    // Who was given the objective, and the project.
    let position_id = i.root.metadata["workforce"]["positionId"]
        .as_str()
        .map(str::to_owned);
    let position_title = position_id
        .as_deref()
        .and_then(|p| i.view.position(p))
        .map(|p| p.title.clone());
    let project_id = i
        .root
        .project_id
        .clone()
        .or_else(|| i.descendants.iter().find_map(|(t, _)| t.project_id.clone()));
    let project_name = project_id.as_deref().and_then(|p| {
        i.view
            .records
            .projects
            .iter()
            .find(|x| x.id == p)
            .map(|x| x.name.clone())
    });
    let answer = last_result(events_of(&i.root.id))
        .and_then(|e| e.payload["text"].as_str())
        .map(|t| cap_bytes(t.trim(), MAX_ANSWER_BYTES))
        .filter(|t| !t.is_empty());
    ObjectiveReport {
        root_task_id: i.root.id.clone(),
        correlation_id: str_of(&i.root.metadata["liaison"]["correlationId"]),
        objective: i.root.objective.clone(),
        position_id,
        position_title,
        project_id,
        project_name,
        state: i.root.state,
        created_at: i.root.created_at,
        completed_at: i.root.completed_at,
        answer,
        tasks,
        workers,
        files,
        checks,
        branches,
        pull_requests,
        reviews,
        findings,
        approvals,
        blocked,
        problems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviews_are_read_from_their_blocks() {
        let text = "Looks mostly fine.\n\n```plenipo-review\n{\"verdict\": \"request-changes\", \
                    \"findings\": [{\"severity\": \"major\", \"file\": \"src/a.rs\", \"summary\": \
                    \"Handles no errors\"}, {\"severity\": \"nit\", \"summary\": \"Rename x\"}, \
                    {\"summary\": \"\"}]}\n```\nAnd a second:\n```plenipo-review\n{\"verdict\": \
                    \"Approved\"}\n```\n```plenipo-review\n{\"verdict\": \"maybe\"}\n```\n\
                    ```json\n{\"verdict\": \"approve\"}\n```";
        let r = reviews_in(text);
        assert_eq!(r.len(), 2, "{r:?}");
        assert_eq!(r[0].verdict, ReviewVerdict::RequestChanges);
        assert_eq!(
            r[0].findings,
            [
                (
                    Severity::Major,
                    Some("src/a.rs".into()),
                    "Handles no errors".into()
                ),
                (Severity::Minor, None, "Rename x".into()),
            ]
        );
        assert_eq!(r[1].verdict, ReviewVerdict::Approve);
        assert!(r[1].findings.is_empty());
        assert!(reviews_in("no block").is_empty());
    }

    #[test]
    fn tests_and_checks_are_recognized() {
        for c in [
            "cargo test --workspace",
            "npm test",
            "pnpm run test:unit",
            "python -m pytest -q",
            "./check.sh",
            "go test ./...",
            "make check",
            "npx vitest run",
        ] {
            assert!(is_test(c), "{c}");
        }
        for c in [
            "cargo build",
            "npm run build",
            "cargo clippy",
            "go vet",
            "latest",
        ] {
            assert!(!is_test(c), "{c}");
        }
    }
}
