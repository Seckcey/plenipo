//! The Liaison service (ADR-008).
//!
//! - **At a turn's end** (a [`TurnHook`]): a completed answer in a session that allows handoffs
//!   is searched for `plenipo-handoff` blocks. Each is validated and decided — accepted (a
//!   queued child task) or refused — and recorded with the step's result and the task's move
//!   to `blocked`, in one Ledger transaction.
//! - **Reconciliation** (after every relevant Ledger event, and on a timer): answer requests
//!   whose child finished; cancel requests whose requester ended, stopping the child; dispatch
//!   accepted requests to a new worker session when a slot is free; resume waiting tasks whose
//!   replies are all in; discard replies nobody waits for; retire finished handoff workers.
//!   Every step is guarded by recorded state, so repeating a pass changes nothing.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use plenipo_ledger::{
    CancelOutcome, HandoffDecision, Ledger, LedgerError, LedgerEvent, LiaisonMessage, MessageKind,
    MessageState, NewEvent, NewHandoffRequest, NewReply, NewTask, OpenRequest, Task, TaskState,
};
use plenipo_runtime::agent::{
    unavailable_outcome, AgentRuntime, AgentSessionDetail, InstallState, SessionStart, StepNote,
    TurnDisposition, TurnEnd, TurnHook, TurnInput, TurnOutcome, TurnRef, TurnResult, TurnTask,
    OWNER,
};
use plenipo_runtime::RuntimeError;
use serde_json::{json, Value};
use tokio::sync::Notify;

use crate::address::Address;
use crate::context::{
    self, ContextPacket, DeliveredReply, Destination, PacketArtifact, PacketCapabilities,
    PacketFrom, PacketReference, PacketTask, PromptLimits, CONTEXT_FORMAT,
};
use crate::dto::*;
use crate::error::{LiaisonError, Result};
use crate::protocol::{self, cap_chars, Block, ContextRequest, Directive, PROTOCOL};
use crate::store::result_event;

/// Actor recorded for Liaison's own changes.
pub const ACTOR: &str = "liaison";

#[derive(Debug, Clone)]
pub struct LiaisonConfig {
    /// Deepest a handoff chain may go (the owner's task is depth 0).
    pub max_depth: u32,
    pub max_requests_per_answer: usize,
    /// How often one task may be resumed with replies.
    pub max_rounds: u32,
    /// Accepted handoffs per workflow.
    pub max_workflow_handoffs: u32,
    /// Longest reply text passed back to a requester (bytes).
    pub reply_text_bytes: usize,
    /// Longest requester answer passed as context (bytes).
    pub answer_context_bytes: usize,
    /// Longest task result passed as context (bytes).
    pub task_context_bytes: usize,
    /// Reconciliation also runs this often, to retry work refused for lack of a worker slot.
    pub tick: Duration,
}

impl Default for LiaisonConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_requests_per_answer: 3,
            max_rounds: 5,
            max_workflow_handoffs: 12,
            reply_text_bytes: 16 * 1024,
            answer_context_bytes: 12 * 1024,
            task_context_bytes: 8 * 1024,
            tick: Duration::from_secs(2),
        }
    }
}

/// Liaison settings stored with a session (`metadata.liaison`).
#[derive(Debug, Default)]
struct SessionInfo {
    enabled: bool,
    /// `owner` or `handoff`.
    origin: Option<String>,
}

fn session_info(metadata: &Value) -> SessionInfo {
    let l = &metadata["liaison"];
    SessionInfo {
        enabled: l["enabled"].as_bool().unwrap_or(false),
        origin: l["origin"].as_str().map(str::to_owned),
    }
}

/// Liaison fields of a task (`metadata.liaison`).
#[derive(Debug, Default)]
struct TaskInfo {
    present: bool,
    correlation_id: Option<String>,
    depth: u32,
}

fn task_info(metadata: &Value) -> TaskInfo {
    let l = &metadata["liaison"];
    TaskInfo {
        present: l.is_object(),
        correlation_id: l["correlationId"].as_str().map(str::to_owned),
        depth: l["depth"]
            .as_u64()
            .and_then(|d| u32::try_from(d).ok())
            .unwrap_or(0),
    }
}

/// Metadata for a new owner task in a session that allows handoffs: a new workflow.
fn root_metadata() -> Value {
    json!({ "liaison": {
        "correlationId": uuid::Uuid::new_v4().to_string(),
        "depth": 0,
        "protocol": PROTOCOL,
    }})
}

fn first_line(text: &str, max: usize) -> String {
    cap_chars(
        text.lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim(),
        max,
    )
}

/// At most `max` bytes on a character boundary, marking a cut.
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

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn task_event(task_id: &str, event_type: &str, payload: Value) -> NewEvent {
    NewEvent {
        task_id: Some(task_id.into()),
        source: ACTOR.into(),
        event_type: event_type.into(),
        payload,
        ..NewEvent::default()
    }
}

#[derive(Default)]
struct State {
    /// Spawned actions still running (dispatch, delivery, stop, retire), by key.
    inflight: HashSet<String>,
    shutting_down: bool,
    notices: Vec<String>,
}

struct Inner {
    ledger: Arc<Ledger>,
    runtime: AgentRuntime,
    config: LiaisonConfig,
    wake: Arc<Notify>,
    /// Serializes reading a turn's requests with the counts their limits depend on.
    planning: Mutex<()>,
    state: Mutex<State>,
}

/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct Liaison {
    inner: Arc<Inner>,
}

struct Hook {
    inner: Weak<Inner>,
}

impl TurnHook for Hook {
    fn turn_ended(&self, end: &TurnEnd) -> TurnDisposition {
        match self.inner.upgrade() {
            Some(inner) => Liaison { inner }.turn_ended(end),
            None => TurnDisposition::Finish,
        }
    }

    fn released(&self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.wake.notify_one();
        }
    }
}

/// What one reconciliation pass works from.
struct Snapshot {
    open: Vec<OpenRequest>,
    live_cancelled: Vec<Task>,
    deliveries: Vec<String>,
    stale: Vec<String>,
    retire: Vec<String>,
}

impl Snapshot {
    fn read(l: &Ledger) -> plenipo_ledger::Result<Self> {
        let mut retire = Vec::new();
        for session in l.open_handoff_sessions()? {
            let tasks = l.session_tasks(&session.id)?;
            if !tasks.is_empty() && tasks.iter().all(|t| t.state.is_terminal()) {
                retire.push(session.id);
            }
        }
        Ok(Self {
            open: l.liaison_open_requests()?,
            live_cancelled: l.liaison_cancelled_live_children()?,
            deliveries: l.liaison_ready_deliveries()?,
            stale: l.liaison_stale_replies()?,
            retire,
        })
    }
}

/// A request Liaison accepted, with what the child needs.
struct Accepted {
    runtime_id: String,
    depth: u32,
    references: Vec<PacketReference>,
    artifacts: Vec<PacketArtifact>,
}

impl Liaison {
    /// Create Liaison: install its turn hook on `runtime` and listen to `ledger`. Call
    /// [`Liaison::run`] to start reconciliation.
    pub fn new(ledger: Arc<Ledger>, runtime: AgentRuntime, config: LiaisonConfig) -> Self {
        let wake = Arc::new(Notify::new());
        let inner = Arc::new(Inner {
            ledger: Arc::clone(&ledger),
            runtime: runtime.clone(),
            config,
            wake: Arc::clone(&wake),
            planning: Mutex::new(()),
            state: Mutex::new(State::default()),
        });
        runtime.set_hook(Arc::new(Hook {
            inner: Arc::downgrade(&inner),
        }));
        ledger.add_listener(Arc::new(move |e: &LedgerEvent| {
            if e.event_type == "task.state_changed" || e.event_type.starts_with("liaison.") {
                wake.notify_one();
            }
        }));
        Self { inner }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        lock(&self.inner.state)
    }

    fn notice(&self, notice: String) {
        let mut state = self.lock();
        if !state.notices.contains(&notice) {
            state.notices.push(notice);
        }
    }

    fn limits(&self) -> PromptLimits {
        PromptLimits {
            requests_per_answer: self.inner.config.max_requests_per_answer,
        }
    }

    fn destinations(&self) -> Vec<Destination> {
        self.inner
            .runtime
            .runtimes()
            .into_iter()
            .map(|r| Destination {
                runtime_id: r.id,
                label: r.label,
                ready: r.ready,
            })
            .collect()
    }

    fn runtime_label(&self, runtime_id: &str) -> String {
        self.inner
            .runtime
            .runtimes()
            .into_iter()
            .find(|r| r.id == runtime_id)
            .map_or_else(|| runtime_id.to_owned(), |r| r.label)
    }

    fn destination_label(&self, address: &str) -> String {
        match Address::parse(address) {
            Ok(Address::Runtime(id)) => self.runtime_label(&id),
            Ok(Address::Role(name)) => format!("role {name}"),
            _ => address.to_owned(),
        }
    }

    async fn blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Ledger) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let ledger = Arc::clone(&self.inner.ledger);
        tokio::task::spawn_blocking(move || f(&ledger))
            .await
            .map_err(|e| LiaisonError::Internal(e.to_string()))?
    }

    /// Mark `key` as running; false if it already is.
    fn claim(&self, key: &str) -> bool {
        self.lock().inflight.insert(key.to_owned())
    }

    fn release(&self, key: &str) {
        self.lock().inflight.remove(key);
        self.inner.wake.notify_one();
    }

    // ---- Owner-facing sessions ----------------------------------------------------------

    /// Start a session. With `handoffs`, the worker may ask other workers for help: its first
    /// objective carries Liaison's instructions, and it starts a new workflow.
    pub async fn start_session(
        &self,
        runtime_id: &str,
        objective: &str,
        model: Option<&str>,
        handoffs: bool,
    ) -> std::result::Result<AgentSessionDetail, RuntimeError> {
        let runtime = &self.inner.runtime;
        if !handoffs {
            return runtime.start_session(runtime_id, objective, model).await;
        }
        if runtime
            .runtimes()
            .iter()
            .any(|r| r.installation.state == InstallState::Checking)
        {
            runtime.refresh().await;
        }
        let prompt = context::root_prompt(objective, &self.destinations(), self.limits());
        runtime
            .start_session_with(
                SessionStart {
                    id: None,
                    runtime_id: runtime_id.into(),
                    model: model.map(str::to_owned),
                    title: None,
                    metadata: json!({ "liaison": {
                        "enabled": true,
                        "origin": "owner",
                        "protocol": PROTOCOL,
                    }}),
                },
                TurnInput {
                    objective: objective.into(),
                    prompt: Some(prompt),
                    task: TurnTask::New {
                        requested_by: OWNER.into(),
                        metadata: root_metadata(),
                    },
                },
            )
            .await
    }

    /// Give a session its next objective. In a session that allows handoffs it starts a new
    /// workflow; a handoff worker's session takes work only through Liaison.
    pub async fn resume_session(
        &self,
        session_id: &str,
        objective: &str,
    ) -> std::result::Result<AgentSessionDetail, RuntimeError> {
        let runtime = &self.inner.runtime;
        let detail = runtime.session(session_id).await?;
        let info = session_info(&detail.session.metadata);
        if info.origin.as_deref() == Some("handoff") {
            return Err(RuntimeError::NotReady(
                "This is a handoff worker's session: it takes work only through Liaison.".into(),
            ));
        }
        if !info.enabled {
            return runtime.resume_session(session_id, objective).await;
        }
        runtime
            .resume_session_with(
                session_id,
                TurnInput {
                    objective: objective.into(),
                    prompt: None,
                    task: TurnTask::New {
                        requested_by: OWNER.into(),
                        metadata: root_metadata(),
                    },
                },
            )
            .await
    }

    // ---- Turn ends ----------------------------------------------------------------------

    fn turn_ended(&self, end: &TurnEnd) -> TurnDisposition {
        if !session_info(&end.session.metadata).enabled
            || end.result.outcome != TurnOutcome::Completed
        {
            return TurnDisposition::Finish;
        }
        let Some(text) = end.result.text.as_deref() else {
            return TurnDisposition::Finish;
        };
        let extracted = protocol::extract(text);
        if extracted.blocks.is_empty() {
            return TurnDisposition::Finish;
        }
        let _planning = lock(&self.inner.planning);
        match self.record_requests(end, &extracted.answer, &extracted.blocks) {
            Ok(Some(reason)) => {
                self.inner.wake.notify_one();
                TurnDisposition::Suspended { reason }
            }
            Ok(None) => TurnDisposition::Finish,
            Err(e) => {
                self.notice(format!(
                    "Liaison could not record the handoff requests of task {}: {e}",
                    end.task_id
                ));
                TurnDisposition::Finish
            }
        }
    }

    /// Decide and record a step's requests. `Some(reason)`: the task now waits.
    fn record_requests(
        &self,
        end: &TurnEnd,
        answer: &str,
        blocks: &[Block],
    ) -> Result<Option<String>> {
        let config = &self.inner.config;
        let l = &self.inner.ledger;
        let task = l
            .task(&end.task_id)?
            .ok_or_else(|| LedgerError::NotFound(format!("task {}", end.task_id)))?;
        // Sender identity comes from Plenipo's own records: the running turn of this session.
        if task.metadata["sessionId"].as_str() != Some(end.session.id.as_str())
            || task.state != TaskState::Running
        {
            l.append_event(task_event(
                &task.id,
                "liaison.sender_rejected",
                json!({
                    "sessionId": end.session.id,
                    "reason": "the requests did not come from this task's running turn",
                }),
            ))?;
            return Ok(None);
        }
        let info = task_info(&task.metadata);
        let correlation = info
            .correlation_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let rounds = l.count_task_events(&task.id, "liaison.replies_delivered")?;
        if rounds >= config.max_rounds {
            for block in blocks {
                l.append_event(task_event(
                    &task.id,
                    "liaison.handoff_rejected",
                    json!({
                        "correlationId": correlation,
                        "block": block.index,
                        "step": end.step,
                        "reason": format!(
                            "this task already used its {} reply rounds, so it finishes without \
                             further handoffs",
                            config.max_rounds
                        ),
                    }),
                ))?;
            }
            return Ok(None);
        }
        let destinations = self.destinations();
        let mut budget = config
            .max_workflow_handoffs
            .saturating_sub(l.liaison_handoff_count(&correlation)?);
        let mut seen = HashSet::new();
        let mut duplicates = Vec::new();
        let mut requests = Vec::new();
        for block in blocks {
            let fingerprint = protocol::fingerprint(&block.raw);
            if !seen.insert(fingerprint.clone()) {
                duplicates.push(block.index);
                continue;
            }
            let decision = if requests.len() >= config.max_requests_per_answer {
                Err(format!(
                    "too many handoff requests in one answer (at most {})",
                    config.max_requests_per_answer
                ))
            } else {
                match &block.parsed {
                    Err(reason) => Err(reason.clone()),
                    Ok(d) => self.accept(
                        d,
                        &task,
                        &info,
                        &correlation,
                        &destinations,
                        &mut budget,
                        answer,
                    ),
                }
            };
            requests.push(self.request(
                end,
                &task,
                &info,
                &correlation,
                block,
                &format!("{}:{}:{fingerprint}", task.id, end.step),
                decision,
            )?);
        }
        let accepted = requests
            .iter()
            .filter(|r| matches!(r.decision, HandoffDecision::Accept { .. }))
            .count();
        let reason = if accepted > 0 {
            format!(
                "waiting for {accepted} handoff repl{}",
                if accepted == 1 { "y" } else { "ies" }
            )
        } else {
            "its handoff requests were refused; the worker will be told why".into()
        };
        let actor = format!("agent:{}", end.session.runtime_id);
        let step_result = result_event(
            &TurnRef {
                session_id: &end.session.id,
                task_id: &task.id,
                execution_id: end.execution_id.as_deref(),
                step: Some(end.step),
                actor: &actor,
            },
            &end.result,
        )
        .map_err(LiaisonError::Internal)?;
        let suspended = l.suspend_for_handoffs(&task.id, step_result, requests, &reason, ACTOR)?;
        if !suspended.replayed {
            for index in duplicates {
                l.append_event(task_event(
                    &task.id,
                    "liaison.duplicate_ignored",
                    json!({
                        "correlationId": correlation,
                        "block": index,
                        "step": end.step,
                        "reason": "identical to an earlier request in the same answer",
                    }),
                ))?;
            }
        }
        Ok(Some(reason))
    }

    /// Validate an accepted directive against the workflow; `Err` is the refusal reason.
    #[allow(clippy::too_many_arguments)]
    fn accept(
        &self,
        d: &Directive,
        task: &Task,
        info: &TaskInfo,
        correlation: &str,
        destinations: &[Destination],
        budget: &mut u32,
        answer: &str,
    ) -> std::result::Result<Accepted, String> {
        let config = &self.inner.config;
        let list = destinations
            .iter()
            .map(|d| d.runtime_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let depth = info.depth + 1;
        if depth > config.max_depth {
            return Err(format!(
                "the handoff depth limit ({}) is reached, so this part cannot be handed on; do \
                 it yourself",
                config.max_depth
            ));
        }
        let runtime_id = match Address::parse(&d.to) {
            Ok(Address::Runtime(id)) if destinations.iter().any(|x| x.runtime_id == id) => id,
            Ok(Address::Runtime(id)) => {
                return Err(format!(
                    "missing destination: there is no worker runtime named \"{id}\" (available: \
                     {list})"
                ))
            }
            Ok(Address::Role(name)) => {
                return Err(format!(
                    "missing destination: no worker is assigned to the role \"{name}\" yet \
                     (roles arrive with Plenipo's Workforce engine); address a runtime instead: \
                     {list}"
                ))
            }
            Ok(Address::Session(_)) => {
                return Err(format!(
                    "workers cannot address another worker's session; hand off to a runtime \
                     instead: {list}"
                ))
            }
            Ok(Address::Owner | Address::Liaison) => {
                return Err(format!(
                    "\"{}\" is not a destination for handoffs; address a runtime: {list}",
                    d.to
                ))
            }
            Err(e) => return Err(format!("missing destination: {e} (available: {list})")),
        };
        if *budget == 0 {
            return Err(format!(
                "this workflow already used its {} handoffs",
                config.max_workflow_handoffs
            ));
        }
        let l = &self.inner.ledger;
        let same_workflow =
            |task: &Task| task_info(&task.metadata).correlation_id.as_deref() == Some(correlation);
        let mut references = Vec::new();
        for c in &d.context {
            references.push(match c {
                ContextRequest::Answer => PacketReference {
                    kind: "answer".into(),
                    title: "The requester's answer".into(),
                    text: if answer.trim().is_empty() {
                        "(The requester's answer outside its handoff request was empty.)".into()
                    } else {
                        cap_bytes(answer, config.answer_context_bytes)
                    },
                    task_id: None,
                },
                ContextRequest::Excerpt { title, text } => PacketReference {
                    kind: "excerpt".into(),
                    title: format!("Excerpt: {title}"),
                    text: text.clone(),
                    task_id: None,
                },
                ContextRequest::Task { task_id } => {
                    let found = l.task(task_id).map_err(|e| e.to_string())?;
                    let Some(found) = found.filter(|t| same_workflow(t)) else {
                        return Err(format!(
                            "context task {task_id} is not part of this workflow"
                        ));
                    };
                    let result = l
                        .last_task_event(&found.id, "agent.result")
                        .map_err(|e| e.to_string())?
                        .and_then(|e| serde_json::from_value::<TurnResult>(e.payload).ok());
                    PacketReference {
                        kind: "task".into(),
                        title: format!(
                            "Task {} ({}): {}",
                            found.id,
                            found.state.as_str(),
                            first_line(&found.objective, 120)
                        ),
                        text: result.and_then(|r| r.text.or(Some(r.summary))).map_or_else(
                            || "(No result yet.)".into(),
                            |t| cap_bytes(&t, config.task_context_bytes),
                        ),
                        task_id: Some(found.id),
                    }
                }
            });
        }
        let mut artifacts = Vec::new();
        for id in &d.artifacts {
            let artifact = l.artifact(id).map_err(|e| e.to_string())?;
            let owner = match artifact.as_ref().and_then(|a| a.task_id.clone()) {
                Some(owner) => l.task(&owner).map_err(|e| e.to_string())?,
                None => None,
            };
            match (artifact, owner) {
                (Some(a), Some(owner)) if same_workflow(&owner) || owner.id == task.id => artifacts
                    .push(PacketArtifact {
                        id: a.id,
                        artifact_type: a.artifact_type,
                        path: a.local_path,
                        uri: a.uri,
                        hash: a.hash,
                    }),
                _ => return Err(format!("artifact {id} is not part of this workflow")),
            }
        }
        *budget -= 1;
        Ok(Accepted {
            runtime_id,
            depth,
            references,
            artifacts,
        })
    }

    /// The Ledger record for one block: an accepted request with its child, or a refusal with
    /// the reply that explains it.
    #[allow(clippy::too_many_arguments)]
    fn request(
        &self,
        end: &TurnEnd,
        task: &Task,
        info: &TaskInfo,
        correlation: &str,
        block: &Block,
        dedupe_key: &str,
        decision: std::result::Result<Accepted, String>,
    ) -> Result<NewHandoffRequest> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let source = Address::Session(end.session.id.clone()).to_string();
        let now = plenipo_ledger::now_ms();
        let directive = block.parsed.as_ref().ok();
        let destination = directive
            .map(|d| {
                Address::parse(&d.to).map_or_else(|_| cap_chars(d.to.trim(), 64), |a| a.to_string())
            })
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "unknown".into());
        let context_summary: Vec<Value> = match &decision {
            Ok(a) => a
                .references
                .iter()
                .map(|r| json!({ "kind": r.kind, "title": r.title, "chars": r.text.chars().count() }))
                .collect(),
            Err(_) => Vec::new(),
        };
        let mut envelope = json!({
            "protocol": PROTOCOL,
            "kind": "request",
            "messageId": message_id,
            "correlationId": correlation,
            "parentTaskId": task.id,
            "source": source,
            "sourceRuntime": end.session.runtime_id,
            "destination": destination,
            "objective": directive.map(|d| d.objective.clone()),
            "acceptanceCriteria": directive.map(|d| d.acceptance_criteria.clone()),
            "context": context_summary,
            "artifacts": directive.map_or_else(Vec::new, |d| d.artifacts.clone()),
            "capabilities": {
                "requested": directive.map_or_else(Vec::new, |d| d.capabilities.clone()),
                "granted": [],
            },
            "priority": directive.and_then(|d| d.priority).unwrap_or(task.priority),
            "timestamp": now,
            "step": end.step,
            "block": block.index,
        });
        let summary = json!({
            "objective": directive.map(|d| first_line(&d.objective, 200)),
            "step": end.step,
            "block": block.index,
        });
        let decision = match decision {
            Ok(accepted) => {
                let d = directive.ok_or_else(|| {
                    LiaisonError::Internal("an accepted request has no directive".into())
                })?;
                let priority = d.priority.unwrap_or(task.priority);
                let packet = ContextPacket {
                    format: CONTEXT_FORMAT.into(),
                    message_id: message_id.clone(),
                    correlation_id: correlation.into(),
                    task: PacketTask {
                        objective: d.objective.clone(),
                        acceptance_criteria: d.acceptance_criteria.clone(),
                        priority,
                    },
                    from: PacketFrom {
                        address: source.clone(),
                        runtime_id: end.session.runtime_id.clone(),
                        runtime_label: self.runtime_label(&end.session.runtime_id),
                        task_id: task.id.clone(),
                        objective: first_line(&task.objective, 200),
                    },
                    depth: accepted.depth,
                    max_depth: self.inner.config.max_depth,
                    references: accepted.references,
                    artifacts: accepted.artifacts,
                    capabilities: PacketCapabilities {
                        requested: d.capabilities.clone(),
                        granted: Vec::new(),
                    },
                };
                envelope["depth"] = json!(accepted.depth);
                envelope["destination"] = json!(format!("runtime:{}", accepted.runtime_id));
                envelope["packet"] = serde_json::to_value(&packet)
                    .map_err(|e| LiaisonError::Internal(e.to_string()))?;
                let received = json!({
                    "objective": first_line(&d.objective, 200),
                    "depth": accepted.depth,
                    "runtimeId": accepted.runtime_id,
                    "references": envelope["context"],
                    "artifacts": packet.artifacts.len(),
                    "capabilities": { "requested": d.capabilities, "granted": [] },
                    "contextFormat": CONTEXT_FORMAT,
                });
                HandoffDecision::Accept {
                    child: NewTask {
                        parent_task_id: Some(task.id.clone()),
                        requested_by: format!("agent:{}", end.session.runtime_id),
                        assigned_to: Some(accepted.runtime_id.clone()),
                        project_id: task.project_id.clone(),
                        objective: d.objective.clone(),
                        acceptance_criteria: d.acceptance_criteria.clone(),
                        priority,
                        metadata: json!({
                            "sessionId": uuid::Uuid::new_v4().to_string(),
                            "runtimeId": accepted.runtime_id,
                            "turn": 1,
                            "liaison": {
                                "correlationId": correlation,
                                "depth": accepted.depth,
                                "requestId": message_id,
                                "parentTaskId": task.id,
                                "parentSessionId": end.session.id,
                                "protocol": PROTOCOL,
                            },
                        }),
                    },
                    received,
                }
            }
            Err(reason) => {
                envelope["rejection"] = json!(reason);
                envelope["raw"] = json!(cap_chars(&block.raw, 2_000));
                envelope["depth"] = json!(info.depth + 1);
                let reply_id = uuid::Uuid::new_v4().to_string();
                HandoffDecision::Reject {
                    reply: NewReply {
                        message_id: reply_id.clone(),
                        correlation_id: correlation.into(),
                        in_reply_to: message_id.clone(),
                        child_task_id: None,
                        source: ACTOR.into(),
                        envelope: json!({
                            "protocol": PROTOCOL,
                            "kind": "reply",
                            "messageId": reply_id,
                            "correlationId": correlation,
                            "inReplyTo": message_id,
                            "parentTaskId": task.id,
                            "source": ACTOR,
                            "destination": source,
                            "timestamp": now,
                            "result": { "outcome": "rejected", "summary": reason },
                        }),
                        summary: json!({ "outcome": "rejected", "summary": reason }),
                    },
                    reason,
                }
            }
        };
        Ok(NewHandoffRequest {
            message_id,
            correlation_id: correlation.into(),
            dedupe_key: dedupe_key.into(),
            source,
            destination,
            envelope,
            summary,
            decision,
        })
    }

    // ---- Reconciliation -----------------------------------------------------------------

    /// Reconcile until [`Liaison::shutdown`]: after every relevant Ledger event or released
    /// worker slot, and every `tick`.
    pub async fn run(self) {
        loop {
            if self.lock().shutting_down {
                break;
            }
            if let Err(e) = self.reconcile().await {
                self.notice(format!("Liaison could not finish a pass: {e}"));
            }
            tokio::select! {
                () = self.inner.wake.notified() => {}
                () = tokio::time::sleep(self.inner.config.tick) => {}
            }
        }
    }

    /// Stop dispatching and delivering (running workers are stopped by the runtime).
    pub fn shutdown(&self) {
        self.lock().shutting_down = true;
        self.inner.wake.notify_one();
    }

    /// One reconciliation pass. Long actions (starting or stopping workers) run in the
    /// background; their results trigger the next pass.
    pub async fn reconcile(&self) -> Result<()> {
        if self.lock().shutting_down {
            return Ok(());
        }
        let snapshot = self.blocking(|l| Ok(Snapshot::read(l)?)).await?;
        for open in snapshot.open {
            match &open.child {
                Some(child) if child.state.is_terminal() => {
                    self.answer(&open.message, child).await?;
                }
                _ if open.parent_state.is_terminal() => {
                    self.cancel(&open.message, open.parent_state).await?;
                }
                Some(child)
                    if open.message.state == MessageState::Accepted
                        && child.state == TaskState::Queued =>
                {
                    self.spawn_dispatch(open.message.clone(), child.clone());
                }
                _ => {}
            }
        }
        for child in snapshot.live_cancelled {
            self.spawn_stop(child);
        }
        for task_id in snapshot.deliveries {
            self.spawn_deliver(task_id);
        }
        for task_id in snapshot.stale {
            self.blocking(move |l| {
                Ok(
                    l.discard_replies(
                        &task_id,
                        "the task is no longer waiting for replies",
                        ACTOR,
                    )?,
                )
            })
            .await?;
        }
        for session_id in snapshot.retire {
            self.spawn_retire(session_id);
        }
        Ok(())
    }

    /// Record the reply of a finished child.
    async fn answer(&self, request: &LiaisonMessage, child: &Task) -> Result<()> {
        let (request, child) = (request.clone(), child.clone());
        let limit = self.inner.config.reply_text_bytes;
        self.blocking(move |l| {
            let reply = build_reply(l, &request, &child, limit)?;
            l.answer_request(reply, ACTOR)?;
            Ok(())
        })
        .await
    }

    async fn cancel(&self, request: &LiaisonMessage, parent_state: TaskState) -> Result<()> {
        let id = request.id.clone();
        let reason = format!(
            "the requesting task {}",
            match parent_state {
                TaskState::Cancelled => "was cancelled",
                TaskState::Failed => "failed",
                _ => "finished",
            }
        );
        let outcome = self
            .blocking(move |l| Ok(l.cancel_request(&id, &reason, ACTOR)?))
            .await?;
        if let CancelOutcome::Cancelled { child: Some(child) } = outcome {
            if !child.state.is_terminal() {
                self.spawn_stop(*child);
            }
        }
        Ok(())
    }

    fn spawn_stop(&self, child: Task) {
        let Some(session_id) = child.metadata["sessionId"].as_str().map(str::to_owned) else {
            return;
        };
        let key = format!("stop:{}", child.id);
        if !self.claim(&key) {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            // Ends a running step or a wait; a turn still starting is retried by a later pass.
            let _ = this.inner.runtime.cancel_turn(&session_id).await;
            this.release(&key);
        });
    }

    fn spawn_dispatch(&self, request: LiaisonMessage, child: Task) {
        let key = format!("dispatch:{}", request.id);
        if !self.claim(&key) {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            if let Err(e) = this.dispatch(&request, &child).await {
                this.notice(format!(
                    "Liaison could not dispatch handoff {}: {e}",
                    request.id
                ));
            }
            this.release(&key);
        });
    }

    /// Start the child's worker session. The dispatch is recorded when its turn begins.
    async fn dispatch(&self, request: &LiaisonMessage, child: &Task) -> Result<()> {
        let packet: ContextPacket = serde_json::from_value(request.envelope["packet"].clone())
            .map_err(|e| LiaisonError::Internal(format!("unreadable context packet: {e}")))?;
        let (Some(session_id), Some(runtime_id)) = (
            child.metadata["sessionId"].as_str(),
            child.metadata["runtimeId"].as_str(),
        ) else {
            return Err(LiaisonError::Internal(format!(
                "task {} has no worker session",
                child.id
            )));
        };
        let prompt = context::child_prompt(&packet, &self.destinations(), self.limits());
        let parent_session = request.source.strip_prefix("session:").unwrap_or_default();
        let start = SessionStart {
            id: Some(session_id.into()),
            runtime_id: runtime_id.into(),
            model: None,
            title: Some(child.objective.clone()),
            metadata: json!({ "liaison": {
                "enabled": true,
                "origin": "handoff",
                "protocol": PROTOCOL,
                "requestId": request.id,
                "correlationId": request.correlation_id,
                "parentTaskId": request.task_id,
                "parentSessionId": parent_session,
                "depth": packet.depth,
            }}),
        };
        let input = TurnInput {
            objective: child.objective.clone(),
            prompt: Some(prompt),
            task: TurnTask::Existing {
                task_id: child.id.clone(),
            },
        };
        match self.inner.runtime.start_session_with(start, input).await {
            Ok(_) | Err(RuntimeError::Busy(_) | RuntimeError::ShuttingDown) => Ok(()),
            Err(e) => {
                let outcome = self.refusal_outcome(runtime_id, &e);
                let (id, reason) = (request.id.clone(), e.to_string());
                // Refused only while still waiting to be dispatched (not cancelled meanwhile).
                let _ = self
                    .blocking(move |l| {
                        Ok(l.fail_handoff_dispatch(
                            &id,
                            &reason,
                            json!({ "outcome": outcome }),
                            ACTOR,
                        )?)
                    })
                    .await;
                Ok(())
            }
        }
    }

    fn refusal_outcome(&self, runtime_id: &str, e: &RuntimeError) -> HandoffOutcome {
        match e {
            RuntimeError::NotReady(_) => self
                .inner
                .runtime
                .runtimes()
                .iter()
                .find(|r| r.id == runtime_id)
                .map_or(HandoffOutcome::ProviderUnavailable, |r| {
                    unavailable_outcome(r).into()
                }),
            _ => HandoffOutcome::Failed,
        }
    }

    fn spawn_deliver(&self, task_id: String) {
        let key = format!("deliver:{task_id}");
        if !self.claim(&key) {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            if let Err(e) = this.deliver(&task_id).await {
                this.notice(format!(
                    "Liaison could not deliver replies to task {task_id}: {e}"
                ));
            }
            this.release(&key);
        });
    }

    /// Resume a waiting task with all its pending replies, in its own provider session.
    async fn deliver(&self, task_id: &str) -> Result<()> {
        let tid = task_id.to_owned();
        let (task, replies, requests, rounds) = self
            .blocking(move |l| {
                let task = l
                    .task(&tid)?
                    .ok_or_else(|| LedgerError::NotFound(format!("task {tid}")))?;
                let replies = l.liaison_pending_replies(&tid)?;
                let mut requests = Vec::new();
                for r in &replies {
                    requests.push(l.liaison_message(r.in_reply_to.as_deref().unwrap_or_default())?);
                }
                let rounds = l.count_task_events(&tid, "liaison.replies_delivered")?;
                Ok((task, replies, requests, rounds))
            })
            .await?;
        if replies.is_empty() || task.state != TaskState::Blocked {
            return Ok(());
        }
        let Some(session_id) = task.metadata["sessionId"].as_str() else {
            return Err(LiaisonError::Internal(format!(
                "task {} has no worker session",
                task.id
            )));
        };
        let delivered: Vec<DeliveredReply> = replies
            .iter()
            .zip(&requests)
            .map(|(reply, request)| {
                let result = &reply.envelope["result"];
                let str_of = |v: &Value| v.as_str().map(str::to_owned);
                DeliveredReply {
                    from: if reply.source == ACTOR {
                        "Plenipo".into()
                    } else {
                        self.runtime_label(result["runtimeId"].as_str().unwrap_or("worker"))
                    },
                    request: request
                        .as_ref()
                        .and_then(|r| str_of(&r.envelope["objective"]))
                        .unwrap_or_else(|| "(an unreadable request)".into()),
                    outcome: str_of(&result["outcome"]).unwrap_or_else(|| "failed".into()),
                    summary: str_of(&result["summary"]).unwrap_or_default(),
                    text: str_of(&result["text"]),
                    error: str_of(&result["error"]),
                }
            })
            .collect();
        let rounds_left = self
            .inner
            .config
            .max_rounds
            .saturating_sub(rounds.saturating_add(1));
        let prompt = context::replies_prompt(
            &delivered,
            &replies[0].correlation_id,
            rounds_left,
            &self.destinations(),
        );
        let ids: Vec<String> = replies.iter().map(|r| r.id.clone()).collect();
        let note = StepNote {
            reason: format!(
                "delivering {} handoff repl{}",
                ids.len(),
                if ids.len() == 1 { "y" } else { "ies" }
            ),
            data: json!({ "deliver": ids }),
        };
        match self
            .inner
            .runtime
            .continue_turn(session_id, &task.id, &prompt, note)
            .await
        {
            Ok(_) | Err(RuntimeError::Busy(_) | RuntimeError::ShuttingDown) => Ok(()),
            Err(RuntimeError::NotWaiting(why)) => {
                // Nothing holds this task's session any more: end it rather than leave it
                // waiting forever.
                let tid = task.id.clone();
                self.blocking(move |l| {
                    l.complete_task(
                        &tid,
                        TaskState::Failed,
                        ACTOR,
                        Some("its worker session is no longer waiting for these replies"),
                        task_event(&tid, "liaison.delivery_failed", json!({ "reason": why })),
                    )?;
                    Ok(())
                })
                .await
            }
            Err(e) => {
                // The runtime ended the turn with the reason; record why the replies stopped.
                let (tid, why) = (task.id.clone(), e.to_string());
                self.blocking(move |l| {
                    l.append_event(task_event(
                        &tid,
                        "liaison.delivery_failed",
                        json!({ "reason": why }),
                    ))?;
                    Ok(())
                })
                .await
            }
        }
    }

    fn spawn_retire(&self, session_id: String) {
        let key = format!("retire:{session_id}");
        if !self.claim(&key) {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            // A worker still finishing is retired by a later pass.
            let _ = this.inner.runtime.close_session(&session_id).await;
            this.release(&key);
        });
    }

    // ---- Queries --------------------------------------------------------------------------

    /// The handoff that created `task_id` (if any) and those it made.
    pub fn task_handoffs(&self, task_id: &str) -> Result<TaskHandoffs> {
        let l = &self.inner.ledger;
        let task = l
            .task(task_id)?
            .ok_or_else(|| LedgerError::NotFound(format!("task {task_id}")))?;
        let info = task_info(&task.metadata);
        let messages = l.liaison_messages_for_task(task_id)?;
        let replies: HashMap<&str, &LiaisonMessage> = messages
            .iter()
            .filter(|m| m.kind == MessageKind::Reply)
            .filter_map(|m| m.in_reply_to.as_deref().map(|r| (r, m)))
            .collect();
        let sent = messages
            .iter()
            .filter(|m| m.kind == MessageKind::Request)
            .map(|m| self.view(m, replies.get(m.id.as_str()).copied()))
            .collect::<Result<Vec<_>>>()?;
        let received = match l.liaison_request_for_child(task_id)? {
            Some(request) => {
                let reply = l.liaison_reply_to(&request.id)?;
                Some(self.view(&request, reply.as_ref())?)
            }
            None => None,
        };
        Ok(TaskHandoffs {
            task_id: task.id,
            correlation_id: info.correlation_id,
            depth: info.present.then_some(info.depth),
            received,
            sent,
        })
    }

    fn view(
        &self,
        request: &LiaisonMessage,
        reply: Option<&LiaisonMessage>,
    ) -> Result<HandoffView> {
        let e = &request.envelope;
        let child = match &request.child_task_id {
            Some(id) => self.inner.ledger.task(id)?,
            None => None,
        };
        let strings = |v: &Value| -> Vec<String> {
            v.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        Ok(HandoffView {
            message_id: request.id.clone(),
            correlation_id: request.correlation_id.clone(),
            state: match request.state {
                MessageState::Dispatched => HandoffState::Dispatched,
                MessageState::Answered => HandoffState::Answered,
                MessageState::Cancelled => HandoffState::Cancelled,
                MessageState::Rejected => HandoffState::Rejected,
                _ => HandoffState::Accepted,
            },
            requester_task_id: request.task_id.clone(),
            requester: request.source.clone(),
            requester_runtime_id: e["sourceRuntime"].as_str().map(str::to_owned),
            step: e["step"].as_u64().and_then(|s| u32::try_from(s).ok()),
            destination: request.destination.clone(),
            destination_label: self.destination_label(&request.destination),
            objective: e["objective"]
                .as_str()
                .map_or_else(|| "(an unreadable request)".into(), str::to_owned),
            acceptance_criteria: e["acceptanceCriteria"].as_str().unwrap_or("").to_owned(),
            priority: e["priority"]
                .as_u64()
                .and_then(|p| u8::try_from(p).ok())
                .unwrap_or(2),
            depth: e["depth"]
                .as_u64()
                .and_then(|d| u32::try_from(d).ok())
                .unwrap_or(0),
            context: e["context"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(|c| ContextSummary {
                            kind: c["kind"].as_str().unwrap_or("").to_owned(),
                            title: c["title"].as_str().unwrap_or("").to_owned(),
                            chars: c["chars"]
                                .as_u64()
                                .and_then(|n| u32::try_from(n).ok())
                                .unwrap_or(0),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            artifacts: strings(&e["artifacts"]),
            capabilities_requested: strings(&e["capabilities"]["requested"]),
            rejection: e["rejection"].as_str().map(str::to_owned),
            child_task_id: request.child_task_id.clone(),
            child_session_id: child
                .as_ref()
                .and_then(|c| c.metadata["sessionId"].as_str().map(str::to_owned)),
            child_state: child.as_ref().map(|c| c.state),
            reply: reply.map(reply_view),
            created_at: request.created_at,
            updated_at: request.updated_at,
        })
    }

    /// The whole delegation tree around `task_id`, depth-first from its root.
    pub fn task_tree(&self, task_id: &str) -> Result<TaskTree> {
        let l = &self.inner.ledger;
        let root = l.task_root(task_id)?;
        let mut by_parent: HashMap<String, Vec<(Task, u32)>> = HashMap::new();
        for (task, depth) in l.descendant_tasks(&root.id)? {
            let parent = task.parent_task_id.clone().unwrap_or_default();
            by_parent.entry(parent).or_default().push((task, depth));
        }
        let correlation_id = task_info(&root.metadata).correlation_id;
        let mut nodes = Vec::new();
        let mut stack = vec![(root.clone(), 0_u32)];
        while let Some((task, depth)) = stack.pop() {
            if nodes.len() >= 500 {
                break;
            }
            if let Some(children) = by_parent.remove(&task.id) {
                stack.extend(children.into_iter().rev());
            }
            let handoff = match l.liaison_request_for_child(&task.id)? {
                Some(request) => {
                    let reply = l.liaison_reply_to(&request.id)?;
                    Some(HandoffBrief {
                        message_id: request.id.clone(),
                        state: match request.state {
                            MessageState::Dispatched => HandoffState::Dispatched,
                            MessageState::Answered => HandoffState::Answered,
                            MessageState::Cancelled => HandoffState::Cancelled,
                            MessageState::Rejected => HandoffState::Rejected,
                            _ => HandoffState::Accepted,
                        },
                        destination_label: self.destination_label(&request.destination),
                        reply_outcome: reply.map(|r| outcome_of(&r)),
                    })
                }
                None => None,
            };
            nodes.push(TaskTreeNode {
                runtime_id: task.metadata["runtimeId"].as_str().map(str::to_owned),
                session_id: task.metadata["sessionId"].as_str().map(str::to_owned),
                handoff,
                depth,
                task,
            });
        }
        Ok(TaskTree {
            root_id: root.id,
            focus_id: task_id.to_owned(),
            correlation_id,
            nodes,
        })
    }

    pub fn overview(&self) -> Result<LiaisonOverview> {
        let config = &self.inner.config;
        let open = self.inner.ledger.liaison_open_requests()?.len();
        Ok(LiaisonOverview {
            protocol: PROTOCOL.into(),
            context_format: CONTEXT_FORMAT.into(),
            limits: LiaisonLimits {
                max_depth: config.max_depth,
                max_requests_per_answer: u32::try_from(config.max_requests_per_answer)
                    .unwrap_or(u32::MAX),
                max_rounds: config.max_rounds,
                max_workflow_handoffs: config.max_workflow_handoffs,
            },
            destinations: self
                .destinations()
                .into_iter()
                .map(|d| DestinationInfo {
                    address: d.runtime_id.clone(),
                    runtime_id: d.runtime_id,
                    label: d.label,
                    ready: d.ready,
                })
                .collect(),
            open_handoffs: u32::try_from(open).unwrap_or(u32::MAX),
            notices: self.lock().notices.clone(),
        })
    }
}

fn outcome_of(reply: &LiaisonMessage) -> HandoffOutcome {
    serde_json::from_value(reply.envelope["result"]["outcome"].clone())
        .unwrap_or(HandoffOutcome::Failed)
}

fn reply_view(reply: &LiaisonMessage) -> ReplyView {
    let result = &reply.envelope["result"];
    ReplyView {
        message_id: reply.id.clone(),
        state: match reply.state {
            MessageState::Delivered => ReplyState::Delivered,
            MessageState::Discarded => ReplyState::Discarded,
            _ => ReplyState::Pending,
        },
        outcome: outcome_of(reply),
        summary: result["summary"].as_str().unwrap_or("").to_owned(),
        text: result["text"].as_str().map(str::to_owned),
        error: result["error"].as_str().map(str::to_owned),
        source: reply.source.clone(),
        created_at: reply.created_at,
    }
}

/// A finished child's reply: its final result, or why it never ran.
fn build_reply(
    l: &Ledger,
    request: &LiaisonMessage,
    child: &Task,
    text_limit: usize,
) -> Result<NewReply> {
    let (outcome, summary, text, error) =
        if let Some(e) = l.last_task_event(&child.id, "agent.result")? {
            let r: TurnResult = serde_json::from_value(e.payload)
                .map_err(|e| LiaisonError::Internal(format!("unreadable result: {e}")))?;
            (HandoffOutcome::from(r.outcome), r.summary, r.text, r.error)
        } else if let Some(e) = l.last_task_event(&child.id, "liaison.dispatch_failed")? {
            (
                serde_json::from_value(e.payload["outcome"].clone())
                    .unwrap_or(HandoffOutcome::ProviderUnavailable),
                e.payload["reason"]
                    .as_str()
                    .unwrap_or("The worker could not be started")
                    .to_owned(),
                None,
                None,
            )
        } else if child.state == TaskState::Cancelled {
            (
                HandoffOutcome::Cancelled,
                "Cancelled before it started".to_owned(),
                None,
                None,
            )
        } else {
            (
                HandoffOutcome::Failed,
                "The task ended without a result".to_owned(),
                None,
                None,
            )
        };
    let text = text.map(|t| cap_bytes(&t, text_limit));
    let source = child.metadata["sessionId"].as_str().map_or_else(
        || ACTOR.to_owned(),
        |s| Address::Session(s.into()).to_string(),
    );
    let message_id = uuid::Uuid::new_v4().to_string();
    Ok(NewReply {
        envelope: json!({
            "protocol": PROTOCOL,
            "kind": "reply",
            "messageId": message_id,
            "correlationId": request.correlation_id,
            "inReplyTo": request.id,
            "parentTaskId": request.task_id,
            "childTaskId": child.id,
            "source": source,
            "destination": request.source,
            "timestamp": plenipo_ledger::now_ms(),
            "result": {
                "outcome": outcome,
                "summary": summary,
                "text": text,
                "error": error,
                "runtimeId": child.metadata["runtimeId"],
            },
        }),
        summary: json!({ "outcome": outcome, "summary": first_line(&summary, 200) }),
        message_id,
        correlation_id: request.correlation_id.clone(),
        in_reply_to: request.id.clone(),
        child_task_id: Some(child.id.clone()),
        source,
    })
}
