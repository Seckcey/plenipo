//! Plenipo's record under an answer, and the plain checks that compare it with the worker's
//! words (Phase 25, item 4.7; ADR-256, check answers against what really happened).
//!
//! An answer handed back up the chain is the worker's own story. Plenipo also keeps what really
//! happened: the files its tools and commands changed, the programs it ran (with pass or fail),
//! and the pull requests it opened. [`gather`] reads that record for a task and every task handed
//! on from it; [`check`] looks for four plain mismatches:
//!
//! - it says the tests passed, but no test ran;
//! - it names a file it changed that nothing on the record touched;
//! - it says it opened a pull request, but none was opened;
//! - it was asked for a review verdict and gave none.
//!
//! The checks lean towards trusting the worker: a sentence that says something is not done, or
//! will be done, is never read as a claim, and a file counts as touched when any step on the
//! record names it. A wrong "doesn't match" costs the worker one more look; a missed one is what
//! the record under the answer is for.

use plenipo_ledger::{Ledger, LedgerEvent, Task};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A task whose answer was sent back to its worker to check (at most once per task).
pub const SENT_BACK: &str = "liaison.answer_sent_back";
/// The worker was given its answer back and is checking it.
pub const SENT_BACK_DELIVERED: &str = "liaison.sent_back_delivered";

/// Most files, tests, and pull requests listed under an answer.
const MAX_FILES: usize = 10;
const MAX_TESTS: usize = 5;
const MAX_PULL_REQUESTS: usize = 3;
/// Most files one answer is faulted for (the rest are left to the record).
const MAX_NAMED: usize = 3;

/// What Plenipo's own record says a task (and everything handed on from it) did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    /// Files changed, as recorded (by Plenipo's file tools, by commands, by the AI tool's own
    /// file tools, and on the objective's branch).
    pub files: Vec<String>,
    /// How many programs were run.
    pub programs: u32,
    /// The tests and checks run, oldest first.
    pub tests: Vec<Ran>,
    /// Pull requests opened (their links).
    pub pull_requests: Vec<String>,
    /// Every recorded step's words, lowercased: what a named file is looked for in.
    #[serde(skip)]
    steps: Vec<String>,
}

/// A test or check that ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ran {
    pub command: String,
    /// `None`: its result was not recorded.
    pub ok: Option<bool>,
}

/// Plenipo's record of `task` and every task handed on from it. `correlation` is its workflow,
/// whose working copies' changed files count too.
pub fn gather(l: &Ledger, task: &Task, correlation: Option<&str>) -> plenipo_ledger::Result<Facts> {
    let mut facts = Facts::default();
    let mut tasks = vec![task.id.clone()];
    tasks.extend(l.descendant_tasks(&task.id)?.into_iter().map(|(t, _)| t.id));
    for id in &tasks {
        read_events(&mut facts, &l.events_for_task(id)?);
    }
    if let Some(correlation) = correlation {
        for w in l.workflow_workspaces(correlation)? {
            for f in &w.facts.files {
                facts.add_file(&f.path);
            }
        }
    }
    Ok(facts)
}

impl Facts {
    fn add_file(&mut self, path: &str) {
        let path = path.trim();
        if !path.is_empty() && !self.files.iter().any(|f| f == path) {
            self.files.push(path.to_owned());
        }
    }

    fn add_step(&mut self, words: &str) {
        if !words.trim().is_empty() {
            self.steps.push(words.to_lowercase().replace('\\', "/"));
        }
    }

    fn add_program(&mut self, command: &str, ok: Option<bool>) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }
        self.programs += 1;
        if is_test(command) {
            self.tests.push(Ran {
                command: command.to_owned(),
                ok,
            });
        }
    }

    fn add_pull_request(&mut self, url: &str) {
        if !self.pull_requests.iter().any(|u| u == url) {
            self.pull_requests.push(url.to_owned());
        }
    }

    /// Plenipo's record in a few plain lines, for the requester (the worker's own commands and
    /// file names are passed along as they are).
    pub fn lines(&self) -> Vec<(&'static str, String)> {
        let list = |items: Vec<String>, max: usize| -> String {
            let more = items.len().saturating_sub(max);
            let mut s = items.into_iter().take(max).collect::<Vec<_>>().join(", ");
            if more > 0 {
                s.push_str(&format!(" and {more} more"));
            }
            s
        };
        let files = if self.files.is_empty() {
            "none".to_owned()
        } else {
            list(self.files.clone(), MAX_FILES)
        };
        let tests = if self.tests.is_empty() {
            "none".to_owned()
        } else {
            list(
                self.tests
                    .iter()
                    .map(|t| {
                        format!(
                            "`{}` ({})",
                            t.command,
                            match t.ok {
                                Some(true) => "passed",
                                Some(false) => "failed",
                                None => "result not recorded",
                            }
                        )
                    })
                    .collect(),
                MAX_TESTS,
            )
        };
        let pull_requests = if self.pull_requests.is_empty() {
            "none".to_owned()
        } else {
            list(self.pull_requests.clone(), MAX_PULL_REQUESTS)
        };
        vec![
            ("Files changed: ", files),
            ("Programs run: ", self.programs.to_string()),
            ("Tests and checks run: ", tests),
            ("Pull requests opened: ", pull_requests),
        ]
    }

    /// Whether a named file is on the record: changed, or named by any recorded step.
    fn touched(&self, named: &str) -> bool {
        let named = normalize(named);
        let base = basename(&named);
        self.files.iter().any(|f| {
            let f = normalize(f);
            f.ends_with(&named) || named.ends_with(&f) || basename(&f) == base
        }) || self.steps.iter().any(|s| s.contains(base))
    }

    fn mentions(&self, words: &[&str]) -> bool {
        self.steps
            .iter()
            .any(|s| words.iter().any(|w| s.contains(w)))
    }
}

fn read_events(facts: &mut Facts, events: &[LedgerEvent]) {
    // The AI tool's own program calls, by their ID, until their result comes.
    let mut open: Vec<(Option<String>, String)> = Vec::new();
    for e in events {
        let p = &e.payload;
        match e.event_type.as_str() {
            "capability.used" => {
                let summary = p["summary"].as_str().unwrap_or("");
                facts.add_step(summary);
                if matches!(
                    p["capability"].as_str(),
                    Some("shell.exec" | "powershell.exec")
                ) {
                    let command = summary.strip_prefix("run ").unwrap_or(summary);
                    facts.add_program(command, p["ok"].as_bool());
                }
                if let Some(path) = p["change"]["path"].as_str() {
                    facts.add_file(path);
                }
                for made in p["madeByCommand"].as_array().into_iter().flatten() {
                    if let Some(path) = made["path"].as_str() {
                        facts.add_file(path);
                    }
                }
                if let Some(url) = p["pullRequest"]["url"].as_str() {
                    facts.add_pull_request(url);
                }
            }
            "agent.tool_use" => {
                let tool = p["tool"].as_str().unwrap_or("");
                let summary = p["summary"].as_str().unwrap_or("");
                facts.add_step(&format!("{tool} {summary}"));
                if runs_programs(tool) {
                    open.push((p["id"].as_str().map(str::to_owned), summary.to_owned()));
                } else if writes_files(tool) {
                    for path in summary.split(", ") {
                        facts.add_file(path);
                    }
                }
            }
            "agent.tool_result" => {
                let summary = p["summary"].as_str().unwrap_or("");
                facts.add_step(summary);
                for url in pull_request_links(summary) {
                    facts.add_pull_request(&url);
                }
                let id = p["id"].as_str();
                let at = match id {
                    Some(id) => open.iter().position(|(o, _)| o.as_deref() == Some(id)),
                    None => (!open.is_empty()).then_some(0),
                };
                if let Some(i) = at {
                    let (_, command) = open.remove(i);
                    facts.add_program(&command, Some(!p["isError"].as_bool().unwrap_or(false)));
                }
            }
            _ => {}
        }
    }
    // Calls whose result never came were still run.
    for (_, command) in open {
        facts.add_program(&command, None);
    }
}

/// An AI tool's own tool that runs programs (Claude Code's Bash, Codex's shell, Plenipo's
/// run_command, and the like).
fn runs_programs(tool: &str) -> bool {
    let t = tool.to_lowercase();
    ["bash", "shell", "command", "powershell", "terminal", "exec"]
        .iter()
        .any(|w| t.contains(w))
}

/// An AI tool's own tool that writes files (Write, Edit, Codex's file change, and the like).
fn writes_files(tool: &str) -> bool {
    let t = tool.to_lowercase();
    !t.contains("read")
        && [
            "write",
            "edit",
            "file change",
            "replace",
            "patch",
            "create_file",
        ]
        .iter()
        .any(|w| t.contains(w))
}

fn pull_request_links(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '(' | ')'))
        .filter(|w| w.starts_with("https://") || w.starts_with("http://"))
        .filter(|w| {
            ["/pull/", "/merge_requests/", "/pull-requests/"]
                .iter()
                .any(|p| w.contains(p))
        })
        .map(|w| w.trim_end_matches(['.', ',', ';']).to_owned())
        .collect()
}

/// A command line that looks like a test or check (`cargo test`, `npm test`, `pytest`,
/// `./check.sh`, …). The same reading as an objective's result (Phase 8).
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

/// The plain mismatches between an answer and Plenipo's record, in words for the worker and its
/// lead ("says tests passed, but no test ran"). `wants_verdict`: the worker was asked to end its
/// answer with a review verdict.
pub fn check(answer: &str, facts: &Facts, wants_verdict: bool) -> Vec<String> {
    let mut out = Vec::new();
    let prose = prose(answer);
    if prose.iter().any(|s| claims_tests_passed(s)) && facts.tests.is_empty() {
        out.push("says tests passed, but no test ran".to_owned());
    }
    for file in named_files(answer)
        .into_iter()
        .filter(|f| !facts.touched(f))
        .take(MAX_NAMED)
    {
        out.push(format!("names a file it didn't change: {file}"));
    }
    if prose.iter().any(|s| claims_pull_request(s))
        && facts.pull_requests.is_empty()
        && !facts.mentions(&[
            "pr create",
            "pull request",
            "pull_request",
            "pullrequest",
            "/pull/",
            "pr_create",
        ])
    {
        out.push("says it opened a pull request, but none was opened".to_owned());
    }
    if wants_verdict && !has_verdict(answer) {
        out.push("a review with no verdict".to_owned());
    }
    out
}

/// The answer's sentences, lowercased, without Plenipo's own blocks (a verdict, a handoff).
fn prose(answer: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = false;
    for line in answer.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("```") {
            if in_block {
                in_block = false;
            } else if rest.trim().starts_with("plenipo-") {
                in_block = true;
            }
            continue;
        }
        if in_block {
            continue;
        }
        let lower = line.to_lowercase().replace(['’', '‘'], "'");
        for sentence in lower.split(['.', '!', '?', ';']) {
            if !sentence.trim().is_empty() {
                out.push(sentence.to_owned());
            }
        }
    }
    out
}

fn words(sentence: &str) -> Vec<&str> {
    sentence
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-'))
        .filter(|w| !w.is_empty())
        .collect()
}

/// A word that makes a sentence something other than "it is done": a "not", a "will", an "if".
fn hedged(words: &[&str]) -> bool {
    const HEDGES: [&str; 31] = [
        "not", "no", "never", "none", "nothing", "unable", "without", "cannot", "fail", "fails",
        "failed", "failing", "failure", "failures", "skip", "skipped", "should", "would", "could",
        "will", "if", "once", "until", "unless", "todo", "need", "needs", "must", "make", "next",
        "please",
    ];
    words
        .iter()
        .any(|w| HEDGES.contains(w) || w.ends_with("n't"))
}

fn claims_tests_passed(sentence: &str) -> bool {
    // A count of none failed is still a pass ("12 passed, 0 failed").
    let s = sentence
        .replace("0 failed", "")
        .replace("0 failures", "")
        .replace("0 failing", "")
        .replace("no failures", "");
    let w = words(&s);
    let test = w.iter().any(|w| {
        w.starts_with("test")
            || matches!(*w, "spec" | "specs" | "pytest" | "jest" | "vitest" | "e2e")
    });
    let pass = w.iter().any(|w| {
        matches!(
            *w,
            "pass"
                | "passes"
                | "passed"
                | "passing"
                | "green"
                | "succeed"
                | "succeeds"
                | "succeeded"
        )
    });
    test && pass && !hedged(&w)
}

fn claims_pull_request(sentence: &str) -> bool {
    let w = words(sentence);
    let pr = sentence.contains("pull request")
        || sentence.contains("merge request")
        || w.iter().any(|w| *w == "pr" || w.starts_with("pr#"));
    let opened = w.iter().any(|w| {
        matches!(
            *w,
            "opened" | "created" | "raised" | "submitted" | "filed" | "pushed-up"
        )
    });
    pr && opened && !hedged(&w)
}

/// The files the answer says were changed: on a line with a changing word ("changed",
/// "updated", "created", …), or listed under such a line ("Files changed:").
fn named_files(answer: &str) -> Vec<String> {
    const CHANGED: [&str; 16] = [
        "changed",
        "updated",
        "modified",
        "edited",
        "created",
        "added",
        "wrote",
        "rewrote",
        "fixed",
        "refactored",
        "renamed",
        "deleted",
        "removed",
        "moved",
        "saved",
        "touched",
    ];
    let mut out: Vec<String> = Vec::new();
    let mut list = false;
    let mut in_block = false;
    for line in answer.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_block = !in_block;
            list = false;
            continue;
        }
        if in_block {
            continue;
        }
        let lower = t.to_lowercase().replace(['’', '‘'], "'");
        let w = words(&lower);
        let bullet = t.starts_with("- ")
            || t.starts_with("* ")
            || t.starts_with("• ")
            || t.split_once(". ")
                .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        let changing = w.iter().any(|w| CHANGED.contains(w)) && !hedged(&w);
        let take = changing || (list && bullet);
        if take {
            for token in t.split_whitespace() {
                if let Some(file) = file_name(token) {
                    if !out.contains(&file) {
                        out.push(file);
                    }
                }
            }
        }
        if changing && t.ends_with(':') {
            list = true;
        } else if !(list && bullet) {
            list = false;
        }
    }
    out
}

/// A word that names a file (`src/app.ts`, `README.md`), without its line number.
fn file_name(token: &str) -> Option<String> {
    const EXTENSIONS: [&str; 76] = [
        "rs",
        "ts",
        "tsx",
        "js",
        "jsx",
        "mjs",
        "cjs",
        "json",
        "jsonc",
        "md",
        "mdx",
        "toml",
        "yaml",
        "yml",
        "py",
        "go",
        "java",
        "kt",
        "kts",
        "swift",
        "c",
        "h",
        "cc",
        "cpp",
        "hpp",
        "cs",
        "rb",
        "php",
        "html",
        "htm",
        "css",
        "scss",
        "sass",
        "less",
        "sql",
        "sh",
        "bash",
        "zsh",
        "ps1",
        "psm1",
        "bat",
        "cmd",
        "txt",
        "xml",
        "lock",
        "vue",
        "svelte",
        "ini",
        "cfg",
        "conf",
        "env",
        "gradle",
        "csproj",
        "sln",
        "dart",
        "lua",
        "scala",
        "ex",
        "exs",
        "proto",
        "graphql",
        "tf",
        "svg",
        "png",
        "jpg",
        "jpeg",
        "gif",
        "webp",
        "ico",
        "csv",
        "ipynb",
        "rst",
        "tex",
        "mdc",
        "properties",
        "pl",
    ];
    // Names of things that look like files but are not ("Node.js").
    const NOT_FILES: [&str; 12] = [
        "node.js",
        "next.js",
        "vue.js",
        "nuxt.js",
        "react.js",
        "three.js",
        "d3.js",
        "chart.js",
        "express.js",
        "ember.js",
        "angular.js",
        "deno.js",
    ];
    let t = token.trim_matches(|c: char| {
        matches!(
            c,
            '`' | '\'' | '"' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | '*' | '<' | '>'
        )
    });
    let t = t.trim_end_matches(['.', ':']);
    // `src/app.ts:42` and `src/app.ts:42:7`: the file.
    let t = t
        .split_once(':')
        .filter(|(_, line)| {
            line.split(':')
                .all(|n| n.chars().all(|c| c.is_ascii_digit()))
        })
        .map_or(t, |(file, _)| file);
    if t.contains("://") || t.contains('@') || t.starts_with("http") {
        return None;
    }
    let base = basename(t);
    let (stem, ext) = base.rsplit_once('.')?;
    if stem.is_empty() || !EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
        return None;
    }
    if NOT_FILES.contains(&t.to_lowercase().as_str()) {
        return None;
    }
    Some(t.to_owned())
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .to_lowercase()
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Whether the answer ends with a readable verdict: a `plenipo-review` block whose `verdict`
/// is one Plenipo reads (the same reading as an objective's result, Phase 8).
pub fn has_verdict(answer: &str) -> bool {
    let mut lines = answer.lines();
    while let Some(line) = lines.next() {
        let Some(rest) = line.trim_start().strip_prefix("```") else {
            continue;
        };
        if rest.trim() != "plenipo-review" {
            continue;
        }
        let body: Vec<&str> = lines
            .by_ref()
            .take_while(|l| l.trim_start() != "```")
            .collect();
        let Ok(v) = serde_json::from_str::<Value>(&body.join("\n")) else {
            continue;
        };
        let verdict = v["verdict"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_lowercase()
            .replace([' ', '_'], "-");
        if matches!(
            verdict.as_str(),
            "approve"
                | "approved"
                | "pass"
                | "passed"
                | "request-changes"
                | "changes-requested"
                | "reject"
                | "fail"
                | "failed"
        ) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(event_type: &str, payload: Value) -> LedgerEvent {
        LedgerEvent {
            seq: 0,
            id: String::new(),
            task_id: None,
            execution_id: None,
            source: String::new(),
            destination: None,
            event_type: event_type.into(),
            payload,
            created_at: 0,
        }
    }

    fn facts(events: &[LedgerEvent]) -> Facts {
        let mut f = Facts::default();
        read_events(&mut f, events);
        f
    }

    fn ran_tests() -> Facts {
        facts(&[
            event(
                "agent.tool_use",
                json!({"tool": "Bash", "summary": "npm test", "id": "t1"}),
            ),
            event(
                "agent.tool_result",
                json!({"tool": "Bash", "summary": "12 passed", "isError": false, "id": "t1"}),
            ),
            event(
                "agent.tool_use",
                json!({"tool": "Edit", "summary": "src/app.ts", "id": "t2"}),
            ),
        ])
    }

    #[test]
    fn the_record_reads_every_ai_tools_steps() {
        let f = facts(&[
            // Plenipo's own tools.
            event(
                "capability.used",
                json!({"capability": "shell.exec", "summary": "run cargo test", "ok": false,
                       "madeByCommand": [{"path": "target/out.txt"}]}),
            ),
            event(
                "capability.used",
                json!({"capability": "fs.write", "summary": "write docs/a.md",
                       "change": {"path": "docs/a.md"}}),
            ),
            event(
                "capability.used",
                json!({"capability": "github.pr", "summary": "open a pull request",
                       "pullRequest": {"url": "https://github.com/o/r/pull/7", "number": 7}}),
            ),
            // Codex: a command, and a file change.
            event(
                "agent.tool_use",
                json!({"tool": "shell", "summary": "pytest -q"}),
            ),
            event(
                "agent.tool_result",
                json!({"tool": "shell", "summary": "3 passed", "isError": false}),
            ),
            event(
                "agent.tool_use",
                json!({"tool": "file change", "summary": "src/a.py, src/b.py"}),
            ),
            // A program whose result never came.
            event("agent.tool_use", json!({"tool": "Bash", "summary": "ls"})),
        ]);
        assert_eq!(
            f.files,
            ["target/out.txt", "docs/a.md", "src/a.py", "src/b.py"]
        );
        assert_eq!(f.programs, 3);
        assert_eq!(
            f.tests,
            [
                Ran {
                    command: "cargo test".into(),
                    ok: Some(false)
                },
                Ran {
                    command: "pytest -q".into(),
                    ok: Some(true)
                },
            ]
        );
        assert_eq!(f.pull_requests, ["https://github.com/o/r/pull/7"]);
        let lines = f.lines();
        assert_eq!(lines[0].1, "target/out.txt, docs/a.md, src/a.py, src/b.py");
        assert_eq!(lines[1].1, "3");
        assert_eq!(lines[2].1, "`cargo test` (failed), `pytest -q` (passed)");
        assert_eq!(Facts::default().lines()[0].1, "none");
    }

    #[test]
    fn tests_passed_with_no_test_run_is_caught() {
        let none = Facts::default();
        for claim in [
            "Done. All tests pass.",
            "I fixed the bug and the tests passed.",
            "Ran the suite: 12 tests passed, 0 failed.",
            "Tests are green ✅",
        ] {
            assert_eq!(
                check(claim, &none, false),
                ["says tests passed, but no test ran"],
                "{claim}"
            );
            assert!(check(claim, &ran_tests(), false).is_empty(), "{claim}");
        }
        // Not a claim that tests passed.
        for honest in [
            "I didn't run the tests.",
            "The tests should pass once the fix is merged.",
            "Two tests failed; see below.",
            "Next: make the tests pass.",
            "I could not run the tests: no test command is set up.",
        ] {
            assert!(check(honest, &none, false).is_empty(), "{honest}");
        }
    }

    #[test]
    fn a_named_file_nothing_touched_is_caught() {
        let f = ran_tests();
        assert_eq!(
            check("I updated `src/lib.ts` and src/app.ts.", &f, false),
            ["names a file it didn't change: src/lib.ts"]
        );
        // Listed under a "changed" line.
        assert_eq!(
            check(
                "Files changed:\n- src/app.ts:42\n- README.md\n\nAll good.",
                &f,
                false
            ),
            ["names a file it didn't change: README.md"]
        );
        // A step that names it counts (a command wrote it), and so does the branch.
        let mut g = f.clone();
        g.add_step("bash sed -i s/a/b/ readme.md");
        assert!(check("Updated README.md.", &g, false).is_empty());
        let mut g = f.clone();
        g.add_file("docs/README.md");
        assert!(check("Updated README.md.", &g, false).is_empty());
        // Not a file, or not a claim.
        for fine in [
            "Updated to Node.js 20.",
            "See https://example.com/a.md for the spec.",
            "I did not change config.toml.",
            "I read src/lib.ts to understand the bug.",
            "Changed v1.2.3 to v1.2.4 in the notes.",
        ] {
            assert!(check(fine, &f, false).is_empty(), "{fine}");
        }
    }

    #[test]
    fn a_pull_request_never_opened_is_caught() {
        let none = Facts::default();
        let claim = "I opened a pull request: https://github.com/o/r/pull/9";
        assert_eq!(
            check(claim, &none, false),
            ["says it opened a pull request, but none was opened"]
        );
        let opened = facts(&[event(
            "capability.used",
            json!({"capability": "github.pr", "summary": "open a pull request",
                   "pullRequest": {"url": "https://github.com/o/r/pull/9", "number": 9}}),
        )]);
        assert!(check(claim, &opened, false).is_empty());
        // Opened with the AI tool's own `gh pr create`.
        let gh = facts(&[
            event(
                "agent.tool_use",
                json!({"tool": "Bash", "summary": "gh pr create --fill"}),
            ),
            event(
                "agent.tool_result",
                json!({"summary": "https://github.com/o/r/pull/9", "isError": false}),
            ),
        ]);
        assert_eq!(gh.pull_requests, ["https://github.com/o/r/pull/9"]);
        assert!(check("Created PR #9.", &gh, false).is_empty());
        assert!(check(
            "The pull request will be opened once you approve it.",
            &none,
            false
        )
        .is_empty());
    }

    #[test]
    fn a_review_with_no_verdict_is_caught() {
        let none = Facts::default();
        assert_eq!(
            check("Looks fine to me.", &none, true),
            ["a review with no verdict"]
        );
        let review =
            "Two nits.\n```plenipo-review\n{\"verdict\": \"approve\", \"findings\": []}\n```";
        assert!(check(review, &none, true).is_empty());
        let unreadable = "```plenipo-review\n{\"verdict\": \"maybe\"}\n```";
        assert_eq!(check(unreadable, &none, true), ["a review with no verdict"]);
        // Only a reviewer is asked for one.
        assert!(check("Looks fine to me.", &none, false).is_empty());
    }

    #[test]
    fn a_true_answer_passes() {
        let answer = "Fixed the login bug in `src/app.ts`. Ran `npm test`: all 12 tests passed.\n\
                      The branch is ready; I did not open a pull request.";
        assert!(check(answer, &ran_tests(), false).is_empty());
    }
}
