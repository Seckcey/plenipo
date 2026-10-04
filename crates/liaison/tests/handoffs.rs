//! Phase 4 Liaison tests: the real Liaison, agent runtime, supervisor, adapters, and a
//! file-backed Ledger, driving `plenipo-fake-agent` installed as `claude` and `codex`. Each plan
//! test is covered, in both directions where it applies. No network, no accounts.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use plenipo_ledger::{
    Ledger, LedgerEvent, LiaisonMessage, MessageKind, MessageState, NewReply, Task, TaskState,
    DB_FILE_NAME,
};
use plenipo_liaison::store::{LedgerExecutionStore, LedgerSessionStore};
use plenipo_liaison::{HandoffOutcome, HandoffState, Liaison, LiaisonConfig, ReplyState};
use plenipo_runtime::agent::{
    builtin_adapters, AgentConfig, AgentRuntime, AgentSink, AgentUpdate, HostEnv, SessionState,
    TurnOutcome, TurnResult,
};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ProfileRegistry, RuntimeError, RuntimeEvent, Supervisor,
    SupervisorConfig,
};

const WAIT: Duration = Duration::from_secs(60);
const HOME_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

fn scratch() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

/// Every AI tool the fake CLI stands in for (`plenipo-fake-agent --personas`).
fn personas() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_plenipo-fake-agent-liaison"))
            .arg("--personas")
            .output()
            .unwrap();
        String::from_utf8(out.stdout)
            .unwrap()
            .leak()
            .lines()
            .collect()
    })
}

/// One copy of the fake CLIs per test process, ready to execute (see the runtime's tests for
/// why: copying while other tests fork can fail with ETXTBSY; hard links never open a write
/// handle).
fn fake_clis() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("liaison-fake-agents-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for stem in personas() {
            let path = dir.join(exe_name(stem));
            std::fs::copy(env!("CARGO_BIN_EXE_plenipo-fake-agent-liaison"), &path).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while let Err(e) = std::process::Command::new(&path).arg("--version").output() {
                assert!(Instant::now() < deadline, "fake CLI never runnable: {e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        dir
    })
}

fn install_fake(bin: &Path, stem: &str) {
    let source = fake_clis().join(exe_name(stem));
    let target = bin.join(exe_name(stem));
    if std::fs::hard_link(&source, &target).is_err() {
        std::fs::copy(&source, &target).unwrap();
    }
}

struct NoOutput;

impl EventSink for NoOutput {
    fn emit(&self, _: RuntimeEvent) {}
}

struct NoUpdates;

impl AgentSink for NoUpdates {
    fn emit(&self, _: AgentUpdate) {}
}

struct Setup {
    installed: &'static [&'static str],
    auth: Option<&'static str>,
    max_active_turns: usize,
    liaison: LiaisonConfig,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            installed: personas(),
            auth: None,
            max_active_turns: 4,
            liaison: LiaisonConfig {
                tick: Duration::from_millis(200),
                ..LiaisonConfig::default()
            },
        }
    }
}

struct H {
    ledger: Arc<Ledger>,
    rt: AgentRuntime,
    sup: Supervisor,
    liaison: Liaison,
    run: tokio::task::JoinHandle<()>,
    dir: tempfile::TempDir,
}

fn runtime(
    dir: &Path,
    ledger: &Arc<Ledger>,
    max_active_turns: usize,
) -> (AgentRuntime, Supervisor) {
    let bin = dir.join("bin");
    let home = dir.join("home");
    let sup = Supervisor::new(
        SupervisorConfig::default(),
        ExecutablePolicy::default(),
        ProfileRegistry::default(),
        Arc::new(LedgerExecutionStore(Arc::clone(ledger))),
        Arc::new(NoOutput),
        vec![],
    );
    let mut config = AgentConfig::new(dir.join("workspaces"));
    config.extra_env = vec![(HOME_VAR.into(), home.display().to_string())];
    config.turn_timeout = Duration::from_secs(120);
    config.max_active_turns = max_active_turns;
    let rt = AgentRuntime::new(
        config,
        builtin_adapters(),
        sup.clone(),
        Arc::new(LedgerSessionStore(Arc::clone(ledger))),
        Arc::new(NoUpdates),
        HostEnv::new(Some(bin.into_os_string()), Some(home), None),
    );
    (rt, sup)
}

async fn harness_with(setup: Setup) -> H {
    let dir = scratch();
    let bin = dir.path().join("bin");
    let home = dir.path().join("home");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(home.join(".plenipo-fake-agent")).unwrap();
    for stem in setup.installed {
        install_fake(&bin, stem);
    }
    if let Some(auth) = setup.auth {
        std::fs::write(home.join(".plenipo-fake-agent").join("auth"), auth).unwrap();
    }
    let ledger = Arc::new(Ledger::open(&dir.path().join("ledger").join(DB_FILE_NAME)).unwrap());
    let (rt, sup) = runtime(dir.path(), &ledger, setup.max_active_turns);
    rt.refresh().await;
    let liaison = Liaison::new(Arc::clone(&ledger), rt.clone(), setup.liaison);
    let run = tokio::spawn(liaison.clone().run());
    H {
        ledger,
        rt,
        sup,
        liaison,
        run,
        dir,
    }
}

async fn harness() -> H {
    harness_with(Setup::default()).await
}

impl H {
    /// Start an owner task that may hand off; returns (session ID, task ID).
    async fn start(&self, runtime: &str, objective: &str) -> (String, String) {
        let d = self
            .liaison
            .start_session(runtime, objective, None, true)
            .await
            .unwrap();
        (d.session.id.clone(), d.turns[0].task_id.clone())
    }

    fn task(&self, id: &str) -> Task {
        self.ledger.task(id).unwrap().unwrap()
    }

    async fn until_task(&self, id: &str, what: &str, pred: impl Fn(&Task) -> bool) -> Task {
        let deadline = Instant::now() + WAIT;
        loop {
            let task = self.task(id);
            if pred(&task) {
                return task;
            }
            assert!(
                Instant::now() < deadline,
                "task {id} never {what}: {task:#?}\n{:#?}",
                self.types(id)
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// Wait until task `id` has finished and its session no longer holds it. A turn's task is
    /// recorded as finished a moment before the runtime releases the session; a follow-up sent
    /// in that moment is refused as "already running".
    async fn finished(&self, id: &str) -> Task {
        let task = self
            .until_task(id, "finished", |t| t.state.is_terminal())
            .await;
        if let Some(session) = task.metadata["sessionId"].as_str() {
            let deadline = Instant::now() + WAIT;
            while let Ok(detail) = self.rt.session(session).await {
                if detail.session.active_task_id.as_deref() != Some(id) {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "task {id} never released its session"
                );
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }
        task
    }

    async fn until(&self, what: &str, pred: impl Fn(&H) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !pred(self) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    fn trail(&self, id: &str) -> Vec<LedgerEvent> {
        self.ledger.events_for_task(id).unwrap()
    }

    fn types(&self, id: &str) -> Vec<String> {
        self.trail(id).into_iter().map(|e| e.event_type).collect()
    }

    fn children(&self, id: &str) -> Vec<Task> {
        self.ledger.child_tasks(id).unwrap()
    }

    fn only_child(&self, id: &str) -> Task {
        let mut children = self.children(id);
        assert_eq!(children.len(), 1, "{children:#?}");
        children.remove(0)
    }

    /// The task's final result.
    fn result(&self, id: &str) -> TurnResult {
        let e = self
            .ledger
            .last_task_event(id, "agent.result")
            .unwrap()
            .expect("a result");
        serde_json::from_value(e.payload).unwrap()
    }

    fn text(&self, id: &str) -> String {
        self.result(id).text.unwrap_or_default()
    }

    fn messages(&self, task_id: &str) -> Vec<LiaisonMessage> {
        self.ledger.liaison_messages_for_task(task_id).unwrap()
    }

    fn requests(&self, task_id: &str) -> Vec<LiaisonMessage> {
        self.messages(task_id)
            .into_iter()
            .filter(|m| m.kind == MessageKind::Request)
            .collect()
    }

    fn fake_state(&self) -> PathBuf {
        self.dir.path().join("home").join(".plenipo-fake-agent")
    }

    fn last_args(&self) -> Vec<String> {
        serde_json::from_str(
            &std::fs::read_to_string(self.fake_state().join("last-args.json")).unwrap(),
        )
        .unwrap()
    }
}

/// `expected` occurs in `types` in this order (other events may come between).
fn assert_in_order(types: &[String], expected: &[&str]) {
    let mut rest = types.iter();
    for want in expected {
        assert!(
            rest.any(|t| t == want),
            "{want} missing or out of order in {types:#?}"
        );
    }
}

fn correlation(task: &Task) -> String {
    task.metadata["liaison"]["correlationId"]
        .as_str()
        .expect("a correlation ID")
        .to_owned()
}

fn depth(task: &Task) -> u64 {
    task.metadata["liaison"]["depth"].as_u64().unwrap()
}

// ---- Plan tests: handoffs in both directions ---------------------------------------------

/// A worker on `requester` asks a worker on `reviewer` for a review; the review comes back
/// into the requester's workflow, and everything is on the Ledger trail.
async fn review_round_trip(requester: &str, reviewer: &str, reviewer_label: &str) {
    let h = harness().await;
    let (session, root) = h
        .start(requester, &format!("Write a parser [handoff:{reviewer}]"))
        .await;
    let done = h.finished(&root).await;
    assert_eq!(done.state, TaskState::Succeeded, "{:#?}", h.types(&root));

    // The child task: created under the requester for the reviewer, in the same workflow.
    let child = h.only_child(&root);
    assert_eq!(child.parent_task_id.as_deref(), Some(root.as_str()));
    assert_eq!(child.assigned_to.as_deref(), Some(reviewer));
    assert_eq!(child.requested_by, format!("agent:{requester}"));
    assert_eq!(child.objective, "Review the answer above");
    assert_eq!(child.state, TaskState::Succeeded);
    assert_eq!(correlation(&child), correlation(&done));
    assert_eq!((depth(&done), depth(&child)), (0, 1));

    // The reviewer received the requester's answer as context and answered.
    let review = h.text(&child.id);
    assert!(
        review.starts_with("Turn 1: you asked \"Review the answer above\"; context: \"Turn 1: you said \\\"Write a parser"),
        "{review}"
    );
    assert!(review.contains("capabilities none granted"), "{review}");

    // The review appears in the originating workflow: the requester continued with it.
    let final_text = h.text(&root);
    assert_eq!(
        final_text,
        format!(
            "Turn 2: received 1 reply: {reviewer_label}: completed: Turn 1: you asked \"Review \
             the answer above\"; context: \"Turn 1: you said \\\"Write a parser \
             [handoff:{reviewer}]\\\". Previous: None.\"; capabilities none granted.."
        )
    );
    let detail = h.rt.session(&session).await.unwrap();
    let turn = &detail.turns[0];
    assert_eq!(turn.steps.len(), 2, "{turn:#?}");
    assert!(turn.steps[0]
        .result
        .as_ref()
        .unwrap()
        .text
        .as_deref()
        .unwrap()
        .contains("```plenipo-handoff"));
    assert_eq!(detail.session.turn_count, 1);
    // Same provider session, resumed for the reply.
    let args = h.last_args();
    let flag = if requester == "claude-code" {
        "--resume"
    } else {
        "resume"
    };
    assert!(args.iter().any(|a| a == flag), "{args:?}");

    // Messages: one request, answered; one reply, delivered — same correlation, linked.
    let messages = h.messages(&root);
    assert_eq!(messages.len(), 2, "{messages:#?}");
    let (request, reply) = (&messages[0], &messages[1]);
    assert_eq!(request.kind, MessageKind::Request);
    assert_eq!(request.state, MessageState::Answered);
    assert_eq!(request.child_task_id.as_deref(), Some(child.id.as_str()));
    assert_eq!(request.source, format!("session:{session}"));
    assert_eq!(request.destination, format!("runtime:{reviewer}"));
    assert_eq!(reply.kind, MessageKind::Reply);
    assert_eq!(reply.state, MessageState::Delivered);
    assert_eq!(reply.in_reply_to.as_deref(), Some(request.id.as_str()));
    assert_eq!(reply.correlation_id, request.correlation_id);
    assert_eq!(request.correlation_id, correlation(&done));
    assert_eq!(reply.destination, request.source);

    // A complete, ordered Ledger trail on both tasks.
    assert_in_order(
        &h.types(&root),
        &[
            "task.created",
            "task.state_changed",
            "execution.running",
            "agent.message",
            "agent.result",
            "liaison.handoff_requested",
            "task.child_created",
            "task.state_changed",
            "liaison.reply_received",
            "liaison.replies_delivered",
            "task.state_changed",
            "execution.running",
            "agent.result",
            "task.state_changed",
        ],
    );
    assert_in_order(
        &h.types(&child.id),
        &[
            "task.created",
            "liaison.handoff_received",
            "liaison.dispatched",
            "task.state_changed",
            "execution.running",
            "agent.message",
            "agent.result",
            "task.state_changed",
            "liaison.reply_sent",
        ],
    );
    let states: Vec<String> = h
        .trail(&root)
        .into_iter()
        .filter(|e| e.event_type == "task.state_changed")
        .map(|e| e.payload["to"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(states, ["running", "blocked", "running", "succeeded"]);
    let executions = h.ledger.executions_for_task(&root).unwrap();
    assert_eq!(executions.len(), 2);
    assert!(executions.iter().all(|e| e.runtime == requester));
    assert_eq!(
        h.ledger.executions_for_task(&child.id).unwrap()[0].runtime,
        reviewer
    );

    // The handoff worker was ephemeral: its session is closed once done.
    let worker = child.metadata["sessionId"].as_str().unwrap().to_owned();
    h.until("the handoff worker to retire", |h| {
        h.ledger
            .runtime_session(&worker)
            .unwrap()
            .is_some_and(|s| s.state == plenipo_ledger::RuntimeSessionState::Closed)
    })
    .await;
    let worker_session = h.rt.session(&worker).await.unwrap().session;
    assert_eq!(worker_session.runtime_id, reviewer);
    assert_eq!(worker_session.state, SessionState::Closed);
    assert_eq!(worker_session.metadata["liaison"]["origin"], "handoff");

    // The views the UI shows.
    let handoffs = h.liaison.task_handoffs(&root).unwrap();
    assert_eq!(handoffs.sent.len(), 1);
    let view = &handoffs.sent[0];
    assert_eq!(view.state, HandoffState::Answered);
    assert_eq!(view.destination_label, reviewer_label);
    assert_eq!(view.step, Some(1));
    assert_eq!(view.context[0].kind, "answer");
    let reply = view.reply.as_ref().unwrap();
    assert_eq!(
        (reply.state, reply.outcome),
        (ReplyState::Delivered, HandoffOutcome::Completed)
    );
    let received = h
        .liaison
        .task_handoffs(&child.id)
        .unwrap()
        .received
        .unwrap();
    assert_eq!(received.message_id, view.message_id);
    let tree = h.liaison.task_tree(&child.id).unwrap();
    assert_eq!(tree.root_id, root);
    assert_eq!(tree.nodes.len(), 2);
    assert_eq!(tree.nodes[1].depth, 1);
    assert_eq!(tree.nodes[1].runtime_label.as_deref(), Some(reviewer_label));
    assert_eq!(
        tree.nodes[1].handoff.as_ref().unwrap().reply_outcome,
        Some(HandoffOutcome::Completed)
    );
    assert!(h.ledger.integrity_check().unwrap().ok);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn codex_task_gets_a_claude_code_review_and_continues_with_it() {
    review_round_trip("codex", "claude-code", "Claude Code").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn claude_code_task_gets_a_codex_review_and_continues_with_it() {
    review_round_trip("claude-code", "codex", "Codex").await;
}

// ---- Plan tests: limits, destinations, failures, cancellation ----------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nested_handoffs_stop_at_the_depth_limit() {
    let h = harness().await;
    let (_, root) = h
        .start(
            "codex",
            "Build it [handoff:claude-code>codex>claude-code>codex]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    // root (0) → claude (1) → codex (2) → claude (3); the fourth level was refused.
    let mut chain = vec![h.task(&root)];
    loop {
        let children = h.children(&chain.last().unwrap().id);
        match children.as_slice() {
            [] => break,
            [only] => chain.push(only.clone()),
            many => panic!("one child per level: {many:#?}"),
        }
    }
    let runtimes: Vec<_> = chain
        .iter()
        .map(|t| t.assigned_to.clone().unwrap())
        .collect();
    assert_eq!(runtimes, ["codex", "claude-code", "codex", "claude-code"]);
    assert_eq!(chain.iter().map(depth).collect::<Vec<_>>(), [0, 1, 2, 3]);
    assert!(chain.iter().all(|t| t.state == TaskState::Succeeded));
    let workflow = correlation(&chain[0]);
    assert!(chain.iter().all(|t| correlation(t) == workflow));

    let deepest = chain.last().unwrap();
    let refused = h.requests(&deepest.id);
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].state, MessageState::Rejected);
    let reason = refused[0].envelope["rejection"].as_str().unwrap();
    assert!(reason.contains("depth limit (3)"), "{reason}");
    // The deepest worker was told and finished the work itself.
    assert!(h
        .text(&deepest.id)
        .contains("Plenipo: rejected: Reason: the handoff depth limit"));
    // Each reply went to its own requester, all the way up.
    for pair in chain.windows(2) {
        let request = &h.requests(&pair[0].id)[0];
        assert_eq!(request.child_task_id.as_deref(), Some(pair[1].id.as_str()));
        let reply = h.ledger.liaison_reply_to(&request.id).unwrap().unwrap();
        assert_eq!(reply.task_id, pair[0].id);
        assert_eq!(reply.state, MessageState::Delivered);
    }
    let tree = h.liaison.task_tree(&root).unwrap();
    assert_eq!(
        tree.nodes.iter().map(|n| n.depth).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn missing_destinations_are_refused_and_the_requester_is_told() {
    let h = harness().await;
    let (session, root) = h
        .start(
            "claude-code",
            "Ask around [handoff:gemini] [handoff:role:Code Reviewer] [handoff:session:0f8fad5b-d9cb-469f-a165-70867728950e]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert!(h.children(&root).is_empty(), "nothing was created");
    let requests = h.requests(&root);
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|r| r.state == MessageState::Rejected));
    let reasons: Vec<&str> = requests
        .iter()
        .map(|r| r.envelope["rejection"].as_str().unwrap())
        .collect();
    assert!(
        reasons[0].contains("no AI tool named \"gemini\""),
        "{reasons:?}"
    );
    assert!(reasons[1].contains("role \"Code Reviewer\""), "{reasons:?}");
    assert!(
        reasons[2].contains("cannot address another worker's session"),
        "{reasons:?}"
    );
    // The requester got all three refusals in one continuation and finished.
    let text = h.text(&root);
    assert!(
        text.starts_with("Turn 2: received 3 replies: Plenipo: rejected"),
        "{text}"
    );
    assert_eq!(
        h.rt.session(&session).await.unwrap().turns[0].steps.len(),
        2
    );
    assert_eq!(
        h.types(&root)
            .iter()
            .filter(|t| *t == "liaison.handoff_rejected")
            .count(),
        3
    );
    // No worker session was started anywhere.
    assert_eq!(h.rt.overview().await.unwrap().sessions.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_worker_replies_with_its_failure_and_the_requester_carries_on() {
    for (requester, reviewer, label) in [
        ("codex", "claude-code", "Claude Code"),
        ("claude-code", "codex", "Codex"),
    ] {
        let h = harness().await;
        let (_, root) = h
            .start(requester, &format!("Build it [handoff:{reviewer}+crash]"))
            .await;
        assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
        let child = h.only_child(&root);
        assert_eq!(child.state, TaskState::Failed);
        assert_eq!(h.result(&child.id).outcome, TurnOutcome::Crashed);
        let text = h.text(&root);
        assert!(text.contains(&format!("{label}: crashed")), "{text}");
        let view = &h.liaison.task_handoffs(&root).unwrap().sent[0];
        assert_eq!(
            view.reply.as_ref().unwrap().outcome,
            HandoffOutcome::Crashed
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unavailable_destination_fails_the_handoff_without_switching_provider() {
    // Only Codex is installed: a request for Claude Code is accepted (it is a known runtime),
    // but its worker cannot start. It is never sent to Codex instead.
    let h = harness_with(Setup {
        installed: &["codex"],
        ..Setup::default()
    })
    .await;
    let (_, root) = h.start("codex", "Build it [handoff:claude-code]").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.only_child(&root);
    assert_eq!(child.state, TaskState::Failed);
    assert_eq!(child.assigned_to.as_deref(), Some("claude-code"));
    assert_in_order(
        &h.types(&child.id),
        &[
            "task.created",
            "liaison.handoff_received",
            "liaison.dispatch_failed",
            "task.state_changed",
            "liaison.reply_sent",
        ],
    );
    let reply = h.liaison.task_handoffs(&root).unwrap().sent[0]
        .reply
        .clone()
        .unwrap();
    assert_eq!(reply.outcome, HandoffOutcome::ProviderUnavailable);
    assert!(
        reply.summary.contains("Claude Code is not available"),
        "{}",
        reply.summary
    );
    assert!(h.text(&root).contains("Claude Code: providerUnavailable"));
    // Every execution ran on Codex, for the requester only.
    let sessions = h.rt.overview().await.unwrap().sessions;
    assert_eq!(
        sessions.len(),
        1,
        "no worker session was opened: {sessions:#?}"
    );
    assert!(h.ledger.executions_for_task(&child.id).unwrap().is_empty());
}

/// Side chats (Phase 25, item 3.5): a side chat during a running task leaves the task alone, has
/// no tools (it is not a member's conversation, which is what Plenipo gives tools to), and its
/// hand-off blocks are not read.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_side_chat_leaves_the_work_alone_has_no_tools_and_hands_nothing_on() {
    let h = harness().await;
    let (_, root) = h
        .start("codex", "Build it [handoff:claude-code+slow]")
        .await;
    h.until("the child to run", |h| {
        h.children(&root)
            .first()
            .is_some_and(|c| c.state == TaskState::Running)
    })
    .await;
    let child = h.only_child(&root);

    let side = h
        .liaison
        .start_side_chat(
            plenipo_runtime::agent::SessionStart {
                runtime_id: "codex".into(),
                title: Some("Side chat with Builder".into()),
                metadata: serde_json::json!({ "sideChat": { "positionId": "p-1", "title": "Builder" } }),
                ..plenipo_runtime::agent::SessionStart::default()
            },
            "How far along is it? [handoff:claude-code]",
        )
        .await
        .unwrap();
    assert_eq!(side.session.title, "Side chat with Builder");
    let meta = &side.session.metadata;
    assert!(
        meta.get("workforce").is_none() && meta.get("liaison").is_none(),
        "{meta}"
    );
    let asked = side.turns[0].task_id.clone();
    assert_eq!(h.finished(&asked).await.state, TaskState::Succeeded);
    // Its hand-off block was not read: nothing was handed on.
    assert!(h.children(&asked).is_empty());
    assert!(h.requests(&asked).is_empty());
    // The work goes on, untouched.
    assert_eq!(h.task(&child.id).state, TaskState::Running);
    assert_eq!(h.task(&root).state, TaskState::Blocked);
    assert_eq!(h.children(&root).len(), 1);
}

/// Stop on a worker (Phase 25, item 3.3): only its task ends; the lead that asked for it is told
/// it was stopped, and carries on with its own task.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stopping_a_worker_tells_its_lead_and_the_lead_carries_on() {
    let h = harness().await;
    let (_, root) = h
        .start("codex", "Build it [handoff:claude-code+slow]")
        .await;
    h.until("the child to run", |h| {
        h.children(&root)
            .first()
            .is_some_and(|c| c.state == TaskState::Running && c.metadata["sessionId"].is_string())
    })
    .await;
    let child = h.only_child(&root);
    let worker = child.metadata["sessionId"].as_str().unwrap().to_owned();

    // The owner stops the worker, as Stop does from any page (in the moment its turn is still
    // starting, Plenipo says to try again).
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match h.rt.cancel_turn(&worker).await {
            Ok(_) => break,
            Err(e) if e.to_string().contains("still starting") => {
                assert!(std::time::Instant::now() < deadline, "{e}");
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(e) => panic!("{e}"),
        }
    }
    assert_eq!(h.finished(&child.id).await.state, TaskState::Cancelled);

    // The lead hears that it was stopped, and finishes its own task.
    let request = h.requests(&root)[0].id.clone();
    h.until("the lead to be told", |h| {
        h.ledger.liaison_reply_to(&request).unwrap().is_some()
    })
    .await;
    let reply = h.ledger.liaison_reply_to(&request).unwrap().unwrap();
    assert_eq!(reply.envelope["result"]["outcome"], "cancelled");
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelling_a_waiting_parent_cancels_its_handoffs_down_the_tree() {
    let h = harness().await;
    let (session, root) = h
        .start("codex", "Build it [handoff:claude-code>codex+slow]")
        .await;
    // Wait until the grandchild (Codex, slow) is running.
    h.until("the child to exist", |h| h.children(&root).len() == 1)
        .await;
    let child = h.only_child(&root);
    h.until("the grandchild to run", |h| {
        h.children(&child.id)
            .first()
            .is_some_and(|g| g.state == TaskState::Running)
    })
    .await;
    let grandchild = h.only_child(&child.id);
    assert_eq!(h.task(&root).state, TaskState::Blocked);
    assert_eq!(h.task(&child.id).state, TaskState::Blocked);

    // The owner cancels the waiting task.
    let detail = h.rt.cancel_turn(&session).await.unwrap();
    assert_eq!(
        detail.turns[0].result.as_ref().unwrap().outcome,
        TurnOutcome::Cancelled
    );
    assert_eq!(h.task(&root).state, TaskState::Cancelled);
    // Liaison stops the child (waiting) and the grandchild (running) in turn.
    assert_eq!(h.finished(&child.id).await.state, TaskState::Cancelled);
    assert_eq!(h.finished(&grandchild.id).await.state, TaskState::Cancelled);
    for task in [&root, &child.id] {
        let request = &h.requests(task)[0];
        let request = h.ledger.liaison_message(&request.id).unwrap().unwrap();
        assert_eq!(request.state, MessageState::Cancelled, "{task}");
        assert!(h.ledger.liaison_reply_to(&request.id).unwrap().is_none());
    }
    assert!(h
        .types(&child.id)
        .contains(&"liaison.handoff_cancelled".to_owned()));
    let exec = h
        .ledger
        .executions_for_task(&grandchild.id)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(exec.state, "cancelled");
    // Nothing was delivered to the cancelled requester, and the workers are retired.
    assert_eq!(
        h.rt.session(&session).await.unwrap().turns[0].steps.len(),
        1
    );
    h.until("the workers to retire", |h| {
        h.ledger.open_handoff_sessions().unwrap().is_empty()
    })
    .await;
    h.until("no open handoffs", |h| {
        h.ledger.liaison_open_requests().unwrap().is_empty()
    })
    .await;
}

// ---- Plan tests: duplicates and correlation ------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn duplicate_requests_create_one_child_and_reconciling_again_changes_nothing() {
    let h = harness().await;
    let (_, root) = h.start("codex", "Build it [handoff-dup:claude-code]").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(
        h.children(&root).len(),
        1,
        "the same request twice made one child"
    );
    assert_eq!(h.requests(&root).len(), 1);
    let ignored: Vec<LedgerEvent> = h
        .trail(&root)
        .into_iter()
        .filter(|e| e.event_type == "liaison.duplicate_ignored")
        .collect();
    assert_eq!(ignored.len(), 1);
    assert_eq!(ignored[0].payload["block"], 2);

    // Reconciling again (as after a missed or repeated event) records nothing new.
    h.until("the worker to retire", |h| {
        h.ledger.open_handoff_sessions().unwrap().is_empty()
    })
    .await;
    let events = h.ledger.status().unwrap().event_count;
    for _ in 0..3 {
        h.liaison.reconcile().await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(h.ledger.status().unwrap().event_count, events);

    // A second reply to the answered request is not recorded.
    let request = &h.requests(&root)[0];
    let again = h
        .ledger
        .answer_request(
            NewReply {
                message_id: "late-duplicate".into(),
                correlation_id: request.correlation_id.clone(),
                in_reply_to: request.id.clone(),
                child_task_id: request.child_task_id.clone(),
                source: "session:x".into(),
                envelope: serde_json::json!({ "result": { "outcome": "completed" } }),
                summary: serde_json::json!({}),
            },
            "test",
        )
        .unwrap();
    assert!(matches!(
        again,
        plenipo_ledger::ReplyOutcome::AlreadyAnswered(_)
    ));
    assert!(h
        .ledger
        .liaison_message("late-duplicate")
        .unwrap()
        .is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn correlation_ids_tie_each_workflow_together_and_forgeries_are_refused() {
    let h = harness().await;
    let (session, root) = h
        .start(
            "claude-code",
            "Plan it [handoff:codex>claude-code] [handoff-forge]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let workflow = correlation(&h.task(&root));

    // Every task, message, and Liaison event of the workflow carries its correlation ID.
    let tree = h.liaison.task_tree(&root).unwrap();
    assert_eq!(tree.correlation_id.as_deref(), Some(workflow.as_str()));
    assert_eq!(tree.nodes.len(), 3);
    for node in &tree.nodes {
        assert_eq!(correlation(&node.task), workflow);
        for e in h.trail(&node.task.id) {
            if e.event_type.starts_with("liaison.") {
                assert_eq!(e.payload["correlationId"], workflow.as_str(), "{e:#?}");
            }
        }
    }
    let messages = h
        .ledger
        .liaison_messages_for_correlation(&workflow)
        .unwrap();
    assert_eq!(messages.len(), 6, "2 accepted + 1 refused, each answered");
    for reply in messages.iter().filter(|m| m.kind == MessageKind::Reply) {
        let request = h
            .ledger
            .liaison_message(reply.in_reply_to.as_deref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(reply.correlation_id, request.correlation_id);
        assert_eq!(
            reply.task_id, request.task_id,
            "replies go to their requester"
        );
        assert_eq!(reply.child_task_id, request.child_task_id);
    }

    // A block that tries to set its own identity is refused.
    let forged = h
        .requests(&root)
        .into_iter()
        .find(|r| r.state == MessageState::Rejected)
        .unwrap();
    let reason = forged.envelope["rejection"].as_str().unwrap();
    assert!(
        reason.contains("\"correlationId\" cannot be set"),
        "{reason}"
    );
    assert_eq!(
        forged.correlation_id, workflow,
        "the real workflow, not the claimed one"
    );

    // A reply claiming another workflow is refused and recorded.
    let request = h
        .requests(&root)
        .into_iter()
        .find(|r| r.child_task_id.is_some())
        .unwrap();
    let child = request.child_task_id.clone().unwrap();
    let forged_reply = NewReply {
        message_id: "forged".into(),
        correlation_id: "some-other-workflow".into(),
        in_reply_to: request.id.clone(),
        child_task_id: Some(child),
        source: "session:intruder".into(),
        envelope: serde_json::json!({}),
        summary: serde_json::json!({}),
    };
    assert!(h.ledger.answer_request(forged_reply, "test").is_err());
    assert!(h.types(&root).contains(&"liaison.reply_refused".to_owned()));

    // The next owner task in the session starts a new workflow.
    let next = h
        .liaison
        .resume_session(&session, "Next [handoff:codex]")
        .await
        .unwrap();
    let next_root = next.turns[1].task_id.clone();
    assert_eq!(h.finished(&next_root).await.state, TaskState::Succeeded);
    let next_workflow = correlation(&h.task(&next_root));
    assert_ne!(next_workflow, workflow);
    assert_eq!(correlation(&h.only_child(&next_root)), next_workflow);
}

// ---- Beyond the plan list ----------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn handoffs_wait_for_a_free_worker_slot() {
    let h = harness_with(Setup {
        max_active_turns: 1,
        ..Setup::default()
    })
    .await;
    let (_, root) = h
        .start("codex", "Split it [handoff:claude-code] [handoff:codex]")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let children = h.children(&root);
    assert_eq!(children.len(), 2);
    assert!(children.iter().all(|c| c.state == TaskState::Succeeded));
    // With one slot, the second worker started only after the first had finished.
    let started = |t: &Task| {
        h.trail(&t.id)
            .into_iter()
            .find(|e| e.event_type == "liaison.dispatched")
            .unwrap()
            .seq
    };
    let ended = |t: &Task| {
        h.trail(&t.id)
            .into_iter()
            .rfind(|e| e.event_type == "task.state_changed")
            .unwrap()
            .seq
    };
    let (a, b) = (&children[0], &children[1]);
    assert!(
        started(b) > ended(a) || started(a) > ended(b),
        "the two workers never overlapped"
    );
    assert!(h.text(&root).starts_with("Turn 2: received 2 replies"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn limits_bound_one_answer_one_workflow_and_one_task() {
    // At most three requests per answer.
    let h = harness().await;
    let (_, root) = h.start("codex", "Fan out [handoff-many:4:codex]").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.children(&root).len(), 3);
    let refused: Vec<_> = h
        .requests(&root)
        .into_iter()
        .filter(|r| r.state == MessageState::Rejected)
        .collect();
    assert_eq!(refused.len(), 1);
    assert!(refused[0].envelope["rejection"]
        .as_str()
        .unwrap()
        .contains("at most 3"));

    // A workflow's handoff budget.
    let h = harness_with(Setup {
        liaison: LiaisonConfig {
            max_workflow_handoffs: 2,
            tick: Duration::from_millis(200),
            ..LiaisonConfig::default()
        },
        ..Setup::default()
    })
    .await;
    let (_, root) = h.start("codex", "Fan out [handoff-many:3:codex]").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.children(&root).len(), 2);
    let rejected = h
        .requests(&root)
        .into_iter()
        .find(|r| r.state == MessageState::Rejected)
        .unwrap();
    assert!(rejected.envelope["rejection"]
        .as_str()
        .unwrap()
        .contains("already used its 2 handoffs"));

    // A task's reply rounds: past the limit it finishes instead of waiting again.
    let h = harness_with(Setup {
        liaison: LiaisonConfig {
            max_rounds: 1,
            tick: Duration::from_millis(200),
            ..LiaisonConfig::default()
        },
        ..Setup::default()
    })
    .await;
    let (session, root) = h
        .start("claude-code", "Keep asking [handoff-always:codex]")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.children(&root).len(), 1);
    assert_eq!(
        h.rt.session(&session).await.unwrap().turns[0].steps.len(),
        2
    );
    let last = h
        .trail(&root)
        .into_iter()
        .rfind(|e| e.event_type == "liaison.handoff_rejected")
        .unwrap();
    assert!(last.payload["reason"]
        .as_str()
        .unwrap()
        .contains("reply rounds"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn long_replies_are_cut_before_they_reach_the_requester() {
    // ADR-012 (brief messages between agents): a reply is at most 8 KiB; the worker's full
    // answer stays with its own task.
    let h = harness().await;
    let (_, root) = h
        .start("codex", "Write a parser [handoff:claude-code+big]")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.only_child(&root);
    assert!(
        h.text(&child.id).len() > 32 * 1024,
        "the worker answered at length"
    );
    let reply = h
        .messages(&root)
        .into_iter()
        .find(|m| m.kind == MessageKind::Reply)
        .unwrap();
    let text = reply.envelope["result"]["text"].as_str().unwrap();
    assert!(text.len() <= 8 * 1024, "{}", text.len());
    assert!(
        text.len() > 7 * 1024 && text.ends_with('…'),
        "{}",
        text.len()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn capability_requests_are_recorded_but_never_granted() {
    let h = harness().await;
    let (_, root) = h.start("codex", "Read it [handoff-caps:claude-code]").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.only_child(&root);
    let received = h
        .trail(&child.id)
        .into_iter()
        .find(|e| e.event_type == "liaison.handoff_received")
        .unwrap();
    assert_eq!(
        received.payload["capabilities"],
        serde_json::json!({ "requested": ["filesystem.read"], "granted": [] })
    );
    assert!(h.text(&child.id).contains("capabilities none granted"));
    let view = &h.liaison.task_handoffs(&root).unwrap().sent[0];
    assert_eq!(view.capabilities_requested, ["filesystem.read"]);
    // The worker still ran with Claude Code's no-tools posture.
    let args = h.ledger.executions_for_task(&child.id).unwrap()[0]
        .args
        .clone();
    let tools = args.iter().position(|a| a == "--tools").unwrap();
    assert_eq!(args[tools + 1], "");
}

/// Every prompt the fake AI tools received, in every conversation.
fn all_prompts(h: &H) -> Vec<String> {
    let dir = h.fake_state().join("sessions");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        let session: serde_json::Value = serde_json::from_str(&text).unwrap();
        for p in session["prompts"].as_array().into_iter().flatten() {
            out.push(p.as_str().unwrap_or_default().to_owned());
        }
    }
    out
}

/// Phase 25, item 4.7 (ADR-256): an answer that doesn't match Plenipo's record goes back to its
/// worker once, with the reasons and the record; its next answer goes up as it is, marked, with
/// the record under it. A true answer goes straight up.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_answer_that_doesnt_match_the_record_is_sent_back_once() {
    let h = harness().await;
    // The worker says the tests pass, and runs none.
    let (_, root) = h
        .start(
            "codex",
            "Fix it {{handoff:claude-code|Say: all tests pass}}",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.only_child(&root);
    assert_eq!(child.state, TaskState::Succeeded);
    let types = h.types(&child.id);
    assert_in_order(
        &types,
        &[
            "agent.result",
            "liaison.answer_sent_back",
            "liaison.sent_back_delivered",
            "agent.result",
        ],
    );
    assert_eq!(
        types
            .iter()
            .filter(|t| *t == "liaison.answer_sent_back")
            .count(),
        1,
        "sent back once only"
    );
    let sent = h
        .ledger
        .last_task_event(&child.id, "liaison.answer_sent_back")
        .unwrap()
        .unwrap();
    assert_eq!(
        sent.payload["mismatches"],
        serde_json::json!(["says tests passed, but no test ran"])
    );
    let prompts = all_prompts(&h);
    let back = prompts
        .iter()
        .find(|p| p.contains("[Plenipo Liaison — your answer, sent back to check]"))
        .expect("the worker was given its answer back");
    assert!(
        back.contains("- Says tests passed, but no test ran.")
            && back.contains("- Tests and checks run: none"),
        "{back}"
    );
    // Its second answer still says so: it goes up marked, with the record under it.
    let reply = h.liaison.task_handoffs(&root).unwrap().sent[0]
        .reply
        .clone()
        .unwrap();
    assert_eq!(reply.mismatches, ["says tests passed, but no test ran"]);
    assert!(reply.sent_back);
    // The requester's answer came after the reply (the words it was given are tested with
    // `context::replies_message`).
    assert!(
        h.text(&root).contains("received 1 reply"),
        "{}",
        h.text(&root)
    );

    // A true answer goes straight up, with the record under it.
    let h = harness().await;
    let (_, root) = h
        .start("codex", "Fix it {{handoff:claude-code|Look at the parser}}")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.only_child(&root);
    assert!(!h
        .types(&child.id)
        .contains(&"liaison.answer_sent_back".to_owned()));
    let reply = h.liaison.task_handoffs(&root).unwrap().sent[0]
        .reply
        .clone()
        .unwrap();
    assert!(reply.mismatches.is_empty() && !reply.sent_back);
    let recorded = h
        .messages(&root)
        .into_iter()
        .find(|m| m.kind == MessageKind::Reply)
        .unwrap();
    assert_eq!(recorded.envelope["result"]["record"]["programs"], 0);
}

/// Phase 25, item 4.8: a link named in an answer that doesn't exist is caught like any other
/// mismatch: the answer goes back once, then up marked.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_fake_link_in_an_answer_is_caught() {
    struct Links;
    impl plenipo_liaison::LinkChecker for Links {
        fn check(&self, _task_id: &str, links: &[String]) -> Vec<Option<bool>> {
            links
                .iter()
                .map(|url| Some(!url.contains("missing")))
                .collect()
        }
    }
    let h = harness().await;
    h.liaison.set_link_checker(Arc::new(Links));
    let (_, root) = h
        .start(
            "codex",
            "Fix it {{handoff:claude-code|Say: the guide is at https://example.com/missing}}",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let child = h.only_child(&root);
    let sent = h
        .ledger
        .last_task_event(&child.id, "liaison.answer_sent_back")
        .unwrap()
        .expect("sent back");
    assert_eq!(
        sent.payload["mismatches"],
        serde_json::json!(["names a link that doesn't exist: https://example.com/missing"])
    );
    let reply = h.liaison.task_handoffs(&root).unwrap().sent[0]
        .reply
        .clone()
        .unwrap();
    assert!(reply.sent_back);
    assert!(
        reply
            .mismatches
            .contains(&"names a link that doesn't exist: https://example.com/missing".to_owned()),
        "{:?}",
        reply.mismatches
    );
}

/// Phase 25, item 4.8: a lead sends a finished task back to the worker who did it, with what
/// to fix. Liaison gives it back as a new request, recorded on both tasks; the lead never
/// controls the worker itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lead_sends_work_back_to_the_worker_who_did_it() {
    let h = harness().await;
    let (_, root) = h
        .start(
            "codex",
            "Fix it [handoff:claude-code] [handoff-sendback:claude-code]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let mut children = h.children(&root);
    children.sort_by_key(|t| t.created_at);
    assert_eq!(children.len(), 2, "{children:#?}");
    let (first, again) = (&children[0], &children[1]);
    let requests = h.requests(&root);
    let request = requests
        .iter()
        .find(|r| r.child_task_id.as_deref() == Some(again.id.as_str()))
        .unwrap();
    assert_eq!(request.envelope["sendBack"], first.id.as_str());
    assert_eq!(
        request.envelope["objective"],
        "Run the tests you said passed, and say what they show"
    );
    // The worker gets its earlier task as context.
    assert_eq!(
        request.envelope["packet"]["references"][0]["taskId"],
        first.id.as_str()
    );
    let sent = h
        .ledger
        .last_task_event(&first.id, "liaison.work_sent_back")
        .unwrap()
        .expect("recorded on the work sent back");
    assert_eq!(sent.payload["byTaskId"], root.as_str());
    assert_eq!(sent.payload["messageId"], request.id.as_str());
    let received = h
        .ledger
        .last_task_event(&again.id, "liaison.handoff_received")
        .unwrap()
        .unwrap();
    assert_eq!(received.payload["sentBack"], first.id.as_str());

    // Only to the worker who did it.
    let h = harness().await;
    let (_, root) = h
        .start(
            "codex",
            "Fix it [handoff:claude-code] [handoff-sendback:codex]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let refused = h
        .requests(&root)
        .into_iter()
        .find(|r| r.state == MessageState::Rejected)
        .expect("refused");
    assert!(
        refused.envelope["rejection"]
            .as_str()
            .unwrap()
            .contains("work is sent back to the worker who did it: use \"to\": \"claude-code\""),
        "{}",
        refused.envelope["rejection"]
    );
    assert_eq!(h.children(&root).len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn invalid_blocks_are_explained_to_the_requester() {
    let h = harness().await;
    let (_, root) = h.start("codex", "Try [handoff-invalid]").await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let request = &h.requests(&root)[0];
    assert_eq!(request.state, MessageState::Rejected);
    assert!(request.envelope["rejection"]
        .as_str()
        .unwrap()
        .contains("not valid JSON"));
    assert_eq!(request.envelope["raw"], "{not json");
    assert!(h
        .text(&root)
        .contains("Plenipo: rejected: Reason: the handoff request is not valid JSON"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sessions_without_handoffs_ignore_handoff_blocks() {
    let h = harness().await;
    let d = h
        .liaison
        .start_session("codex", "Example [handoff:claude-code]", None, false)
        .await
        .unwrap();
    let root = d.turns[0].task_id.clone();
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert!(h.children(&root).is_empty());
    assert!(h.messages(&root).is_empty());
    assert!(!h.types(&root).iter().any(|t| t.starts_with("liaison.")));
    // The prompt was the objective alone: no Liaison instructions were added.
    assert_eq!(
        h.text(&root),
        "Turn 1: you said \"Example [handoff:claude-code]\". Previous: None."
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn handoff_workers_take_no_owner_follow_ups() {
    let h = harness().await;
    let (_, root) = h.start("codex", "Build it [handoff:claude-code]").await;
    h.finished(&root).await;
    let worker = h.only_child(&root).metadata["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();
    let err = h
        .liaison
        .resume_session(&worker, "Do more")
        .await
        .unwrap_err();
    assert!(matches!(err, RuntimeError::NotReady(_)), "{err}");
    assert!(err.to_string().contains("only through Liaison"), "{err}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_restart_interrupts_workflows_in_flight_and_resumes_nothing() {
    let h = harness().await;
    let (_, root) = h
        .start("codex", "Build it [handoff:claude-code+slow]")
        .await;
    h.until("the child to run", |h| {
        h.children(&root)
            .first()
            .is_some_and(|c| c.state == TaskState::Running)
    })
    .await;
    let child = h.only_child(&root);

    // "Restart": Liaison stops, and a new runtime opens the same Ledger, as after a crash.
    h.liaison.shutdown();
    h.run.abort();
    let ledger = Arc::new(Ledger::open(&h.dir.path().join("ledger").join(DB_FILE_NAME)).unwrap());
    let (rt, sup) = runtime(h.dir.path(), &ledger, 4);
    let liaison = Liaison::new(Arc::clone(&ledger), rt.clone(), LiaisonConfig::default());
    let notices = rt.overview().await.unwrap().notices;
    assert!(notices[0].contains("2 agent turn(s)"), "{notices:?}");
    for id in [&root, &child.id] {
        let task = ledger.task(id).unwrap().unwrap();
        assert_eq!(task.state, TaskState::Failed, "{id}");
        let e = ledger.last_task_event(id, "agent.result").unwrap().unwrap();
        assert_eq!(e.payload["outcome"], "interrupted");
    }
    // Reconciling after the restart records what happened and starts nothing.
    let executions = ledger.status().unwrap().execution_count;
    liaison.reconcile().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    liaison.reconcile().await.unwrap();
    let request = &ledger.liaison_messages_for_task(&root).unwrap()[0];
    assert_eq!(request.state, MessageState::Answered);
    let reply = ledger.liaison_reply_to(&request.id).unwrap().unwrap();
    assert_eq!(reply.state, MessageState::Discarded);
    assert_eq!(reply.envelope["result"]["outcome"], "interrupted");
    assert_eq!(sup.active_count(), 0, "nothing was started");
    assert_eq!(ledger.status().unwrap().execution_count, executions);
    let states: Vec<TaskState> = [&root, &child.id]
        .iter()
        .map(|id| ledger.task(id).unwrap().unwrap().state)
        .collect();
    assert_eq!(states, [TaskState::Failed, TaskState::Failed]);
    // Stop the first runtime's still-running worker.
    h.sup.shutdown(Duration::from_secs(10)).await;
    drop(liaison);
    drop(rt);
}

/// Stop all work lasts across a restart (Phase 25, item 3.4; the security review of #156): the
/// Ledger's record turns the hold back on as Plenipo starts (the app does it for the first
/// organization, before Liaison runs), so work started after the restart waits and no AI tool
/// runs, until Allow again.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn after_a_restart_stop_all_work_still_holds_what_starts() {
    let h = harness().await;
    // The owner pressed Stop all, recorded as the app records it.
    h.rt.hold_all_work();
    h.ledger
        .append_event(plenipo_ledger::NewEvent {
            source: "owner".into(),
            event_type: "work.stopped_all".into(),
            payload: json!({ "stopped": 0 }),
            ..plenipo_ledger::NewEvent::default()
        })
        .unwrap();

    // "Restart": Liaison stops, and a new runtime opens the same Ledger.
    h.liaison.shutdown();
    h.run.abort();
    let ledger = Arc::new(Ledger::open(&h.dir.path().join("ledger").join(DB_FILE_NAME)).unwrap());
    let (rt, sup) = runtime(h.dir.path(), &ledger, 4);
    rt.refresh().await;
    assert!(ledger.work_stopped_on_record().unwrap());
    rt.hold_all_work();
    let liaison = Liaison::new(
        Arc::clone(&ledger),
        rt.clone(),
        LiaisonConfig {
            tick: Duration::from_millis(200),
            ..LiaisonConfig::default()
        },
    );
    let run = tokio::spawn(liaison.clone().run());

    // Work started now waits, and no AI tool runs.
    let starting = {
        let liaison = liaison.clone();
        tokio::spawn(async move {
            liaison
                .start_session("codex", "Say hello", None, true)
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(!starting.is_finished(), "it waits for Allow again");
    assert_eq!(
        sup.active_count(),
        0,
        "no AI tool runs while Stop all holds"
    );

    // Allow again: it runs and finishes.
    rt.allow_work();
    let started = tokio::time::timeout(WAIT, starting)
        .await
        .expect("it starts once allowed")
        .unwrap()
        .unwrap();
    let task = started.turns[0].task_id.clone();
    let deadline = Instant::now() + WAIT;
    while !ledger.task(&task).unwrap().unwrap().state.is_terminal() {
        assert!(Instant::now() < deadline, "the task never finished");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        ledger.task(&task).unwrap().unwrap().state,
        TaskState::Succeeded
    );
    liaison.shutdown();
    run.abort();
}

// ---- Leads stop their team mid-task (ADR-259) ---------------------------------------------

/// A harness whose check-ins come after `debounce` ms of quiet after an answer, after `every`
/// ms of waiting, at most `max` a round.
async fn check_in_harness(debounce: u64, every: u64, max: u32) -> H {
    harness_with(Setup {
        liaison: LiaisonConfig {
            tick: Duration::from_millis(100),
            check_in_debounce: Duration::from_millis(debounce),
            check_in_every: Duration::from_millis(every),
            max_check_ins: max,
            ..LiaisonConfig::default()
        },
        ..Setup::default()
    })
    .await
}

const HOUR: u64 = 3_600_000;

/// The quick worker's `[wait-for:…]` file: it answers only once the test has made it, after
/// what the test checks is in place. A check-in comes a moment after an answer, so without it a
/// slow machine could see the check-in before the test is ready for it.
const QUICK: &str = "quick-may-answer";
/// A second worker's, for a test that needs its answer later still.
const SECOND: &str = "second-may-answer";
/// A third worker's, later than the second's.
const THIRD: &str = "third-may-answer";

impl H {
    /// Let the worker waiting for `name` (`[wait-for:NAME]`) answer.
    fn let_answer(&self, name: &str) {
        std::fs::write(self.fake_state().join(name), "").unwrap();
    }

    /// `task`'s child whose objective holds `text`.
    fn child_with(&self, task: &str, text: &str) -> Task {
        self.children(task)
            .into_iter()
            .find(|t| t.objective.contains(text))
            .unwrap_or_else(|| panic!("no child of {task} asked for {text:?}"))
    }

    fn count(&self, task: &str, event_type: &str) -> u32 {
        self.ledger.count_task_events(task, event_type).unwrap()
    }

    fn reply_for(&self, child: &Task) -> serde_json::Value {
        let request = self
            .ledger
            .liaison_request_for_child(&child.id)
            .unwrap()
            .unwrap();
        self.ledger
            .liaison_reply_to(&request.id)
            .unwrap()
            .expect("a reply")
            .envelope["result"]
            .clone()
    }
}

/// When one answer comes back while other work still goes, the lead is checked in on and stops
/// the work that no longer makes sense. The stopped task's reply says so, and the lead gets every
/// answer once, together. A check-in is no round, and the next real delivery is a delivery.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lead_stops_a_worker_when_another_answers_first() {
    let h = check_in_harness(300, HOUR, 4).await;
    let (_, root) = h
        .start(
            "codex",
            "Build it [handoff:claude-code] [handoff:claude-code+slow] [check-in-stop:slow]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let quick = h
        .children(&root)
        .into_iter()
        .find(|t| !t.objective.contains("[slow]"))
        .unwrap();
    let slow = h.child_with(&root, "[slow]");
    assert_eq!(h.task(&quick.id).state, TaskState::Succeeded);
    assert_eq!(h.finished(&slow.id).await.state, TaskState::Cancelled);

    let stopped = h
        .ledger
        .last_task_event(&slow.id, "liaison.work_stopped")
        .unwrap()
        .expect("recorded on the stopped task");
    assert_eq!(stopped.payload["byTaskId"], root.as_str());
    assert_eq!(
        stopped.payload["reason"],
        "The other answers make this unnecessary"
    );
    assert_eq!(stopped.payload["checkIn"], 1);
    assert_eq!(stopped.payload["triggeredBy"], quick.id.as_str());
    let asked = h
        .ledger
        .last_task_event(&root, "liaison.stop_asked")
        .unwrap()
        .expect("recorded on the lead's task");
    assert_eq!(asked.payload["taskId"], slow.id.as_str());
    assert_eq!(asked.payload["checkIn"], 1);
    assert_eq!(asked.payload["triggeredBy"], quick.id.as_str());

    let reply = h.reply_for(&slow);
    assert_eq!(reply["outcome"], "cancelled");
    assert_eq!(
        reply["summary"],
        "Stopped by you: The other answers make this unnecessary"
    );
    assert_eq!(reply["stopped"]["finishedFirst"], false);
    assert_eq!(h.reply_for(&quick)["outcome"], "completed");

    // One check-in, one delivery: the delivery after the check-in was not read as one.
    assert_eq!(h.count(&root, "liaison.check_in_started"), 1);
    assert_eq!(h.count(&root, "liaison.checked_in"), 1);
    assert_eq!(h.count(&root, "liaison.replies_delivered"), 1);
    assert!(
        h.text(&root).contains("received 2 replies"),
        "{}",
        h.text(&root)
    );
}

/// A check-in step never ends the lead, is never checked as an answer, and never hands work on,
/// whatever it says: here a lead that is itself a handed-on task says "All tests passed." and
/// adds a handoff block. Its team goes on, and it gets every answer when the last is back.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_check_in_never_ends_the_lead_hands_work_on_or_is_checked_as_an_answer() {
    let h = check_in_harness(300, HOUR, 4).await;
    let (_, root) = h
        .start(
            "codex",
            "Run it {{handoff:claude-code|Lead it [handoff:codex] \
             [handoff:codex+wait-for:second-may-answer] \
             [check-in-say:All tests passed.] [check-in-handoff:codex]}}",
        )
        .await;
    // The first answer brings the check-in while the second worker still works: it answers
    // only once the check-in is over.
    h.until("the lead", |h| h.children(&root).len() == 1).await;
    let lead = h.only_child(&root);
    h.until("a check-in", |h| {
        h.count(&lead.id, "liaison.checked_in") > 0
    })
    .await;
    h.let_answer(SECOND);
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.task(&lead.id).state, TaskState::Succeeded);
    let checked = h
        .ledger
        .last_task_event(&lead.id, "liaison.checked_in")
        .unwrap()
        .expect("a check-in");
    assert_eq!(checked.payload["outcome"], "completed");
    assert_eq!(checked.payload["stops"], 0);
    assert_eq!(
        checked.payload["ignored"], 1,
        "its handoff block was left alone"
    );
    // No new work mid-round, no answer checked, no early end.
    assert_eq!(h.children(&lead.id).len(), 2);
    assert_eq!(h.count(&lead.id, "liaison.answer_sent_back"), 0);
    let second = h.child_with(&lead.id, SECOND);
    assert_eq!(h.task(&second.id).state, TaskState::Succeeded);
    assert!(
        h.text(&lead.id).contains("received 2 replies"),
        "{}",
        h.text(&lead.id)
    );
}

/// A check-in that fails (here its AI tool crashes) leaves the lead waiting; its team goes on,
/// and no other check-in comes that round.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_check_in_that_fails_leaves_the_lead_waiting() {
    let h = check_in_harness(300, HOUR, 4).await;
    std::fs::write(h.fake_state().join("check-in-markers.txt"), "[crash]").unwrap();
    let (_, root) = h
        .start(
            "codex",
            "Build it [handoff:claude-code] [handoff:claude-code+wait-for:second-may-answer] \
             [handoff:claude-code+wait-for:third-may-answer]",
        )
        .await;
    // The first answer brings the check-in, which fails; the others answer only after it.
    h.until("the check-in to end", |h| {
        h.count(&root, "liaison.checked_in") > 0
    })
    .await;
    h.let_answer(SECOND);
    h.until("the second answer", |h| {
        h.messages(&root)
            .iter()
            .filter(|m| m.kind == MessageKind::Reply)
            .count()
            == 2
    })
    .await;
    // Longer than a check-in waits after an answer: still the one check-in.
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert_eq!(h.count(&root, "liaison.check_in_started"), 1);
    h.let_answer(THIRD);
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let checked = h
        .ledger
        .last_task_event(&root, "liaison.checked_in")
        .unwrap()
        .expect("a check-in");
    assert_ne!(checked.payload["outcome"], "completed");
    assert_eq!(
        h.count(&root, "liaison.check_in_started"),
        1,
        "no other that round"
    );
    for child in h.children(&root) {
        assert_eq!(child.state, TaskState::Succeeded, "{}", child.objective);
    }
    assert!(
        h.text(&root).contains("received 3 replies"),
        "{}",
        h.text(&root)
    );
}

/// A lead can stop only its own team's work, once, and only while it runs. The owner's task, a
/// task further down (stopped only through its own request), a finished task, and a second stop
/// of the same task are refused with the reason. Stopping a task that waits on its own team stops
/// that team too.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lead_can_stop_only_its_own_team() {
    let h = check_in_harness(300, HOUR, 4).await;
    let (_, root) = h
        .start(
            "codex",
            "Run it {{handoff:claude-code|Lead it [handoff:codex+wait-for:quick-may-answer] \
             [handoff:codex>claude-code+slow]}}",
        )
        .await;
    h.until("the lead", |h| h.children(&root).len() == 1).await;
    let lead = h.only_child(&root);
    h.until("the grandchild to run", |h| {
        h.children(&lead.id).iter().any(|m| {
            h.children(&m.id)
                .first()
                .is_some_and(|g| g.state == TaskState::Running)
        })
    })
    .await;
    let middle = h.child_with(&lead.id, "[handoff:claude-code+slow]");
    let quick = h.child_with(&lead.id, QUICK);
    let grandchild = h.only_child(&middle.id);
    let stop = |id: &str| serde_json::json!({ "stop": id, "reason": "Not needed now" });
    std::fs::write(
        h.fake_state().join("check-in.json"),
        serde_json::json!([
            stop(&root),
            stop(&grandchild.id),
            stop(&quick.id),
            stop(&middle.id),
            stop(&middle.id),
            { "stop": middle.id, "reason": "x", "to": "codex" },
        ])
        .to_string(),
    )
    .unwrap();
    // The check-in's stops are written: now the quick answer may bring it.
    h.let_answer(QUICK);

    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.finished(&middle.id).await.state, TaskState::Cancelled);
    assert_eq!(h.finished(&grandchild.id).await.state, TaskState::Cancelled);
    assert_eq!(h.task(&quick.id).state, TaskState::Succeeded);
    let why = |id: &str| -> String {
        h.trail(&lead.id)
            .into_iter()
            .filter(|e| e.event_type == "liaison.stop_refused")
            .find(|e| e.payload["taskId"] == id)
            .map(|e| e.payload["why"].as_str().unwrap_or_default().to_owned())
            .unwrap_or_else(|| panic!("no refusal for {id}"))
    };
    assert!(
        why(&root).contains("not one you handed on"),
        "{}",
        why(&root)
    );
    assert!(
        why(&grandchild.id).contains("not one you handed on"),
        "{}",
        why(&grandchild.id)
    );
    assert!(
        why(&quick.id).contains("already finished"),
        "{}",
        why(&quick.id)
    );
    let refusals: Vec<String> = h
        .trail(&lead.id)
        .into_iter()
        .filter(|e| e.event_type == "liaison.stop_refused")
        .map(|e| e.payload["why"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(
        refusals.iter().any(|w| w.contains("already stopped")),
        "{refusals:?}"
    );
    assert!(
        refusals
            .iter()
            .any(|w| w.contains("only \"stop\" and \"reason\"")),
        "{refusals:?}"
    );
    assert_eq!(h.count(&lead.id, "liaison.stop_asked"), 1);
    assert_eq!(h.count(&root, "liaison.work_stopped"), 0);
    assert_eq!(h.count(&grandchild.id, "liaison.work_stopped"), 0);
    assert_eq!(
        h.reply_for(&middle)["summary"],
        "Stopped by you: Not needed now"
    );
}

/// A check-in that can't start (the lead's AI tool is signed out) leaves no mark: it is recorded
/// as skipped, the lead still waits, and its next real delivery is a delivery, not a check-in.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_check_in_that_cannot_start_leaves_no_mark() {
    let h = check_in_harness(200, HOUR, 4).await;
    let (_, root) = h
        .start(
            "codex",
            "Build it [handoff:claude-code+wait-for:quick-may-answer] \
             [handoff:claude-code+wait-for:second-may-answer]",
        )
        .await;
    h.until("both workers to run", |h| {
        let children = h.children(&root);
        children.len() == 2 && children.iter().all(|t| t.state == TaskState::Running)
    })
    .await;
    let auth = h.fake_state().join("auth");
    std::fs::write(&auth, "signed-out").unwrap();
    // Signed out: now the first answer may bring a check-in.
    h.let_answer(QUICK);
    h.until("a check-in to be tried", |h| {
        h.count(&root, "liaison.check_in_skipped") > 0
    })
    .await;
    assert_eq!(h.count(&root, "liaison.check_in_started"), 0);
    assert_eq!(h.task(&root).state, TaskState::Blocked, "it still waits");
    std::fs::remove_file(&auth).unwrap();
    h.let_answer(SECOND);
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.count(&root, "liaison.replies_delivered"), 1);
    assert!(
        h.text(&root).contains("received 2 replies"),
        "{}",
        h.text(&root)
    );
}

/// A signed-out lead is tried once for an answer, not over and over, and a try that can't start
/// uses up none of the round's check-ins: once signed in, the next answer brings a real one.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_signed_out_lead_is_tried_once_and_checked_in_on_after_sign_in() {
    let h = check_in_harness(1000, HOUR, 1).await;
    let (_, root) = h
        .start(
            "codex",
            "Build it [handoff:claude-code+wait-for:quick-may-answer] \
             [handoff:claude-code+wait-for:second-may-answer] \
             [handoff:claude-code+slow] [check-in-stop:slow]",
        )
        .await;
    h.until("the team to be at work", |h| {
        let children = h.children(&root);
        children.len() == 3 && children.iter().all(|t| t.state == TaskState::Running)
    })
    .await;
    let auth = h.fake_state().join("auth");
    std::fs::write(&auth, "signed-out").unwrap();
    // Signed out: now the first answer may bring a check-in.
    h.let_answer(QUICK);
    h.until("a check-in to be tried", |h| {
        h.count(&root, "liaison.check_in_skipped") > 0
    })
    .await;
    // Two more debounce windows and many passes: still the one try.
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(h.count(&root, "liaison.check_in_skipped"), 1);
    assert_eq!(h.count(&root, "liaison.check_in_started"), 0);
    assert_eq!(h.task(&root).state, TaskState::Blocked, "it still waits");
    std::fs::remove_file(&auth).unwrap();
    // Signed in again: the cap is 1 and the skip didn't use it, so the second answer brings the
    // check-in.
    h.let_answer(SECOND);
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.count(&root, "liaison.check_in_started"), 1);
    assert_eq!(h.count(&root, "liaison.check_in_skipped"), 1);
    let delayed = h.child_with(&root, SECOND);
    let slow = h.child_with(&root, "[slow]");
    assert_eq!(h.task(&slow.id).state, TaskState::Cancelled);
    let asked = h
        .ledger
        .last_task_event(&root, "liaison.stop_asked")
        .unwrap()
        .unwrap();
    assert_eq!(asked.payload["triggeredBy"], delayed.id.as_str());
}

/// A stop in an ordinary answer (not a check-in) is refused and makes no request: the answer's
/// real handoff goes ahead.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stop_outside_a_check_in_is_refused_and_makes_no_request() {
    let h = harness().await;
    let (_, root) = h
        .start("codex", "Build it [handoff:claude-code] [stop-block:t-123]")
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.requests(&root).len(), 1, "only the real handoff");
    let refused = h
        .trail(&root)
        .into_iter()
        .find(|e| e.event_type == "liaison.handoff_rejected")
        .expect("the stop was refused");
    assert!(
        refused.payload["reason"]
            .as_str()
            .unwrap()
            .contains("only in a check-in"),
        "{}",
        refused.payload
    );
}

/// A long wait brings a check-in too, with no answer back yet.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_long_wait_brings_a_check_in() {
    let h = check_in_harness(HOUR, 500, 4).await;
    let (_, root) = h
        .start(
            "codex",
            "Build it [handoff:claude-code+slow] [check-in-stop:slow]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    let slow = h.only_child(&root);
    assert_eq!(h.task(&slow.id).state, TaskState::Cancelled);
    let asked = h
        .ledger
        .last_task_event(&root, "liaison.stop_asked")
        .unwrap()
        .unwrap();
    assert!(asked.payload["triggeredBy"].is_null(), "{}", asked.payload);
}

/// Check-ins are capped per round, so a slow team can't use up its lead's plan.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn check_ins_are_capped_per_round() {
    let h = check_in_harness(HOUR, 200, 2).await;
    let (_, root) = h
        .start("codex", "Build it [handoff:claude-code+slow]")
        .await;
    h.until("two check-ins", |h| {
        h.count(&root, "liaison.checked_in") == 2
    })
    .await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(h.count(&root, "liaison.check_in_started"), 2, "no third");
    // The owner stops the slow worker; the lead finishes with its answer.
    let slow = h.only_child(&root);
    let session = slow.metadata["sessionId"].as_str().unwrap().to_owned();
    stop_now(&h, &session).await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.count(&root, "liaison.check_in_started"), 2);
}

/// Answers that come together bring one check-in, not one each.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn answers_that_come_together_bring_one_check_in() {
    let h = check_in_harness(2000, HOUR, 4).await;
    let (_, root) = h
        .start(
            "codex",
            "Build it [handoff:claude-code+wait-for:quick-may-answer] \
             [handoff:codex+wait-for:quick-may-answer] [handoff:claude-code+slow] \
             [check-in-stop:slow]",
        )
        .await;
    // Both quick workers are at work, then answer together (however slow the machine).
    h.until("the team to be at work", |h| {
        let children = h.children(&root);
        children.len() == 3 && children.iter().all(|t| t.state == TaskState::Running)
    })
    .await;
    h.let_answer(QUICK);
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.count(&root, "liaison.check_in_started"), 1);
    assert!(
        h.text(&root).contains("received 3 replies"),
        "{}",
        h.text(&root)
    );
}

/// No check-in starts while Stop all holds the work; once allowed again, it comes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_check_in_starts_while_stop_all_holds_the_work() {
    let h = check_in_harness(800, HOUR, 4).await;
    let (_, root) = h
        .start(
            "codex",
            "Build it [handoff:claude-code+wait-for:quick-may-answer] [handoff:claude-code+slow] \
             [check-in-stop:slow]",
        )
        .await;
    h.until("both workers to run", |h| {
        let children = h.children(&root);
        children.len() == 2 && children.iter().all(|t| t.state == TaskState::Running)
    })
    .await;
    // Stop all holds the work first; a step already running still ends, so the answer comes in
    // while it holds.
    h.rt.hold_all_work();
    h.let_answer(QUICK);
    h.until("the first answer", |h| {
        h.messages(&root)
            .iter()
            .any(|m| m.kind == MessageKind::Reply)
    })
    .await;
    // Longer than the 800 ms a check-in waits after an answer.
    tokio::time::sleep(Duration::from_millis(2000)).await;
    assert_eq!(h.count(&root, "liaison.check_in_started"), 0);
    h.rt.allow_work();
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert_eq!(h.count(&root, "liaison.check_in_started"), 1);
    let slow = h.child_with(&root, "[slow]");
    assert_eq!(h.task(&slow.id).state, TaskState::Cancelled);
}

/// Stop a session's turn the way the owner's Stop does (in the moment its turn is still
/// starting, Plenipo says to try again).
async fn stop_now(h: &H, session: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match h.rt.cancel_turn(session).await {
            Ok(_) => return,
            Err(e) if e.to_string().contains("still starting") => {
                assert!(Instant::now() < deadline, "{e}");
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(e) => panic!("{e}"),
        }
    }
}

// ---- Members of an organization (Phase 5, ADR-009) ------------------------------------------

use plenipo_ledger::{NewPosition, NewWorker, Position, RoleTemplate, RoleType};
use plenipo_liaison::context::Destination;
use plenipo_liaison::{Directory, Placement, Team};
use plenipo_runtime::agent::{Effort, SessionStart, WorkDoneBy};
use serde_json::{json, Value};

/// A directory over real Ledger positions: a lead with a team whose members are placed by
/// title. (The Workforce engine provides the real one.)
struct TeamDirectory {
    lead: Position,
    members: Vec<Position>,
    /// The work each placed request was about (AI tools and models), in order.
    reviewed: std::sync::Mutex<Vec<Vec<WorkDoneBy>>>,
}

fn runtime_of(p: &Position) -> String {
    p.runtime_id.clone().unwrap_or_default()
}

impl Directory for TeamDirectory {
    fn team(&self, workforce: &Value) -> Option<Team> {
        let position = workforce["positionId"].as_str()?;
        let me = std::iter::once(&self.lead)
            .chain(&self.members)
            .find(|p| p.id == position)?;
        Some(Team {
            identity: format!("You are {}, supervised by Plenipo.", me.title),
            reminder: None,
            members: self
                .members
                .iter()
                .map(|m| Destination {
                    address: format!("role:{}", m.title),
                    label: format!("{} on {}", m.title, runtime_of(m)),
                    ready: true,
                })
                .collect(),
        })
    }

    fn place(
        &self,
        _: &Value,
        requester: &Task,
        name: &str,
        reviewed: &[WorkDoneBy],
    ) -> Result<Placement, String> {
        self.reviewed.lock().unwrap().push(reviewed.to_vec());
        let m = self
            .members
            .iter()
            .find(|m| m.title.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("\"{name}\" is not on your team"))?;
        let agent_id = uuid::Uuid::new_v4().to_string();
        Ok(Placement {
            address: format!("role:{}", m.title),
            label: format!("{} ({})", m.title, runtime_of(m)),
            runtime_id: runtime_of(m),
            model: m.model.clone(),
            // A position with a model also sets an effort level (as the Router may).
            effort: m.model.as_ref().map(|_| Effort::High),
            worker: Some(NewWorker {
                agent_id: agent_id.clone(),
                position_id: m.id.clone(),
                role_id: m.role_id.clone(),
                runtime_id: runtime_of(m),
                runtime_provider: None,
                model: m.model.clone(),
                project_id: requester.project_id.clone(),
                routing: json!({ "reason": "test" }),
            }),
            conversation: None,
            workforce: json!({ "positionId": m.id, "agentId": agent_id }),
            identity: format!("You are working as {} for {}.", m.title, self.lead.title),
            project_id: requester.project_id.clone(),
        })
    }
}

/// An organization with a lead (staffed, on Codex) and two on-demand members: a Reviewer on
/// Claude Code (with a model) and a Builder on Codex. Returns (lead workforce record, members,
/// the directory).
fn organization(h: &H) -> (Value, Vec<Position>, Arc<TeamDirectory>) {
    let l = &h.ledger;
    let roles = l
        .ensure_roles(
            &[
                RoleTemplate {
                    name: "Department Manager",
                    description: "",
                    role_type: RoleType::DepartmentManager,
                    persistent: true,
                    metadata: Value::Null,
                    formerly: &[],
                },
                RoleTemplate {
                    name: "Specialist",
                    description: "",
                    role_type: RoleType::Worker,
                    persistent: false,
                    metadata: Value::Null,
                    formerly: &[],
                },
            ],
            "plenipo",
        )
        .unwrap();
    let role = |n: &str| roles.iter().find(|r| r.name == n).unwrap().id.clone();
    let (_, lead) = l
        .create_department_with_head(
            "Development",
            "",
            &NewPosition {
                title: "Development Manager".into(),
                role_id: role("Department Manager"),
                runtime_id: Some("codex".into()),
                staffed: true,
                ..NewPosition::default()
            },
            "owner",
        )
        .unwrap();
    let member = |title: &str, runtime: &str, model: Option<&str>| {
        l.create_position(
            &NewPosition {
                title: title.into(),
                role_id: role("Specialist"),
                reports_to: Some(lead.id.clone()),
                runtime_id: Some(runtime.into()),
                model: model.map(str::to_owned),
                ..NewPosition::default()
            },
            "owner",
        )
        .unwrap()
        .0
    };
    let members = vec![
        member("Reviewer", "claude-code", Some("fake-model-x")),
        member("Builder", "codex", None),
    ];
    let agent = l.position_incumbent(&lead.id).unwrap().unwrap();
    let workforce = json!({ "positionId": lead.id, "agentId": agent.id });
    let directory = Arc::new(TeamDirectory {
        lead,
        members: members.clone(),
        reviewed: std::sync::Mutex::new(Vec::new()),
    });
    h.liaison
        .set_directory(Arc::clone(&directory) as Arc<dyn Directory>);
    (workforce, members, directory)
}

impl H {
    /// Start the lead's session with `objective`; returns (session ID, task ID).
    async fn start_member(&self, workforce: &Value, objective: &str) -> (String, String) {
        self.start_member_on(workforce, objective, None).await
    }

    /// The same, on `model` (`None`: Codex's default).
    async fn start_member_on(
        &self,
        workforce: &Value,
        objective: &str,
        model: Option<&str>,
    ) -> (String, String) {
        let d = self
            .liaison
            .start_member_session(
                SessionStart {
                    runtime_id: "codex".into(),
                    model: model.map(str::to_owned),
                    ..SessionStart::default()
                },
                objective,
                workforce.clone(),
                None,
            )
            .await
            .unwrap();
        (d.session.id.clone(), d.turns[0].task_id.clone())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_hands_work_to_its_team_and_the_worker_leaves_when_done() {
    let h = harness().await;
    let (lead, members, directory) = organization(&h);
    let reviewer = &members[0];
    let (session, root) = h
        .start_member(&lead, "Plan the release [handoff:role:Reviewer]")
        .await;
    let done = h.finished(&root).await;
    assert_eq!(done.state, TaskState::Succeeded, "{:#?}", h.types(&root));
    assert_eq!(
        done.metadata["workforce"], lead,
        "the turn names its member"
    );

    // The work under review was the requester's own (it referenced no tasks): its AI tool and
    // the model it ran (ADR-081).
    // Its task named no model and the stand-in Codex reported none: Codex's default.
    assert_eq!(
        *directory.reviewed.lock().unwrap(),
        [vec![WorkDoneBy::new("codex", None)]]
    );
    // The child went to the Reviewer position: its runtime and model, recorded under it.
    let child = h.only_child(&root);
    assert_eq!(child.assigned_to.as_deref(), Some("claude-code"));
    assert_eq!(
        child.metadata["workforce"]["positionId"],
        json!(reviewer.id)
    );
    let agent_id = child.metadata["workforce"]["agentId"].as_str().unwrap();
    let request = &h.requests(&root)[0];
    assert_eq!(request.destination, "role:Reviewer");
    assert_eq!(
        request.envelope["destinationLabel"],
        "Reviewer (claude-code)"
    );
    let child_session = child.metadata["sessionId"].as_str().unwrap();
    let worker_session = h.rt.session(child_session).await.unwrap().session;
    assert_eq!(
        worker_session.metadata["workforce"]["agentId"],
        json!(agent_id)
    );
    assert_eq!(worker_session.runtime_id, "claude-code");
    assert_eq!(
        worker_session.model.as_deref(),
        Some("fake-model-x"),
        "the position's model"
    );
    assert_eq!(
        worker_session.effort,
        Some(Effort::High),
        "the placement's effort"
    );

    // The worker was in the workforce while it ran and left when its task ended; its
    // history remains.
    assert_in_order(
        &h.types(&child.id),
        &[
            "task.created",
            "liaison.handoff_received",
            "org.worker_spawned",
            "liaison.dispatched",
            "task.state_changed",
            "org.worker_started",
            "agent.result",
            "task.state_changed",
            "org.worker_retired",
        ],
    );
    let agents = h.ledger.position_agents(&reviewer.id, 10).unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].id, agent_id);
    assert_eq!(
        agents[0].lifecycle_state,
        plenipo_ledger::AgentLifecycle::Retired
    );
    assert!(h
        .ledger
        .org_records()
        .unwrap()
        .agents
        .iter()
        .all(|a| a.id != agent_id));

    // The lead continued with the reply in its own session; a member's reply is named by its
    // position (Phase 8).
    assert!(
        h.text(&root)
            .starts_with("Turn 2: received 1 reply: Reviewer (Claude Code): completed"),
        "{}",
        h.text(&root)
    );
    let view = h.liaison.task_handoffs(&root).unwrap();
    assert_eq!(view.sent[0].destination_label, "Reviewer (claude-code)");
    // A member session takes objectives only through the organization.
    let err = h
        .liaison
        .resume_session(&session, "more")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Organization view"), "{err}");
    let stranger = json!({ "positionId": reviewer.id, "agentId": "someone-else" });
    assert!(h
        .liaison
        .resume_member_session(&session, "more", stranger, None)
        .await
        .is_err());
    let next = h
        .liaison
        .resume_member_session(&session, "One more thing", lead.clone(), None)
        .await
        .unwrap();
    let second = next.turns[1].task_id.clone();
    assert_eq!(h.finished(&second).await.state, TaskState::Succeeded);
    assert!(h
        .text(&second)
        .starts_with("Turn 3: you said \"One more thing\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn members_address_their_team_by_role_and_never_a_raw_runtime() {
    let h = harness().await;
    let (lead, _, _) = organization(&h);
    let (_, root) = h
        .start_member(
            &lead,
            "Ask around [handoff:claude-code] [handoff:role:Stranger]",
        )
        .await;
    assert_eq!(h.finished(&root).await.state, TaskState::Succeeded);
    assert!(h.children(&root).is_empty(), "nothing was created");
    let reasons: Vec<String> = h
        .requests(&root)
        .iter()
        .map(|r| r.envelope["rejection"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        reasons[0].contains(
            "hand work to a member of your team, not to an AI tool: role:Reviewer, role:Builder"
        ),
        "{reasons:?}"
    );
    assert!(
        reasons[1].contains("\"Stranger\" is not on your team"),
        "{reasons:?}"
    );
    // No worker was recorded.
    assert!(
        h.ledger.org_records().unwrap().agents.len() == 1,
        "only the lead"
    );
}

/// Cross-company review counts who made the models (ADR-081 §3): the directory hears which
/// model did the work under review, not only which AI tool.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_directory_hears_the_model_that_did_the_work_under_review() {
    let h = harness().await;
    let (lead, _members, directory) = organization(&h);
    let (_, root) = h
        .start_member_on(
            &lead,
            "Plan the release [handoff:role:Reviewer]",
            Some("gpt-6-sol"),
        )
        .await;
    h.finished(&root).await;
    assert_eq!(
        *directory.reviewed.lock().unwrap(),
        [vec![WorkDoneBy::new("codex", Some("gpt-6-sol"))]]
    );
}

/// Work passed on by its task ID is the work under review (ADR-081 §3): the AI tool and model
/// that task asked for, not the requester's own.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_directory_hears_the_model_of_a_task_passed_on() {
    let h = harness().await;
    let (lead, _members, directory) = organization(&h);
    let (_, root) = h
        .start_member(
            &lead,
            "Plan the release [handoff:role:Reviewer] [handoff-pass:role:Builder]",
        )
        .await;
    h.finished(&root).await;
    let reviewed = directory.reviewed.lock().unwrap().clone();
    assert_eq!(
        reviewed,
        [
            // To the Reviewer: the requester's own work (Codex, its default).
            vec![WorkDoneBy::new("codex", None)],
            // To the Builder: the Reviewer's task it passed on, on the model its position names.
            vec![WorkDoneBy::new("claude-code", Some("fake-model-x"))],
        ]
    );
}
