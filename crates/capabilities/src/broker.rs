//! The capability broker (Phase 7, ADR-013). For every step of an organization worker's task it
//! opens a **grant** — a snapshot of the worker's permissions, its project folder, and a ticket
//! for Plenipo's tool server — and closes it when the step's program ends. Every tool call is
//! read, confined to the folder, checked by Guard against the grant and the settings as they
//! are now, sent to the owner for approval when Guard says so, carried out by Plenipo,
//! recorded in the Ledger, and returned to the worker with secrets hidden.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::Duration;

use plenipo_guard::engine::{doing, Request, Scope};
use plenipo_guard::redact::Redactor;
use plenipo_guard::{
    evaluate, level_for, levels_for, Capability, CommandLine, Decision, GrantState, Guard, Layer,
    Level, PathRefusal, Resolved, Risk, SensitiveKind, Verdict, Workspace,
};
use plenipo_ledger::{Approval, ApprovalState, Ledger, NewEvent};
use plenipo_runtime::agent::tools::{
    StepInfo, StepTools, TextFilter, ToolProvider, ToolServer, SERVER_NAME,
};
use plenipo_runtime::Supervisor;
use serde_json::{json, Value};
use tokio::runtime::Handle;
use tokio::sync::oneshot;

use crate::dto::*;
use crate::error::{BrokerError, Result};
use crate::files;
use crate::programs::{self, Run};
use crate::relay::{Ticket, ARG};
use crate::tools::{self, Action, ToolDef, TOOLS};
use crate::vault::{self, SecretStore};

/// Source of Guard's own events.
const GUARD: &str = "guard";
const PLENIPO: &str = "plenipo";
/// Longest detail kept in an event or approval card.
const MAX_DETAIL: usize = 2000;

#[derive(Debug, Clone)]
pub struct BrokerConfig {
    /// The relay program the AI tool starts, and arguments before its ticket argument.
    pub relay_command: PathBuf,
    pub relay_args: Vec<String>,
    /// Where tickets and MCP configuration files are written (Plenipo's private folder).
    pub tickets_dir: PathBuf,
    /// Longest one tool call may take (an approval waits inside a call).
    pub call_timeout: Duration,
    /// A program's time limit when the worker names none.
    pub command_timeout: Duration,
    /// How long one "minute" of the approval window lasts (tests shorten it).
    pub approval_minute: Duration,
}

impl BrokerConfig {
    pub fn new(relay_command: PathBuf, tickets_dir: PathBuf) -> Self {
        Self {
            relay_command,
            relay_args: Vec::new(),
            tickets_dir,
            call_timeout: Duration::from_secs(60 * 60),
            command_timeout: Duration::from_secs(10 * 60),
            approval_minute: Duration::from_secs(60),
        }
    }
}

/// A tool call's answer to the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallResult {
    pub text: String,
    pub is_error: bool,
}

impl CallResult {
    fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
        }
    }

    fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
        }
    }
}

struct Grant {
    id: String,
    ticket: String,
    files: Vec<PathBuf>,
    task_id: String,
    session_id: String,
    runtime_id: String,
    step: u32,
    position_id: Option<String>,
    worker: String,
    scope: Scope,
    workspace: Option<Workspace>,
    levels: BTreeMap<Capability, Level>,
    tools: Vec<&'static str>,
    opened_at: u64,
    revoked: bool,
    running: HashSet<String>,
    pending: HashSet<String>,
    used: u32,
    blocked: u32,
    asked: u32,
}

impl Grant {
    fn view(&self) -> GrantView {
        GrantView {
            grant_id: self.id.clone(),
            task_id: self.task_id.clone(),
            session_id: self.session_id.clone(),
            step: self.step,
            position_id: self.position_id.clone(),
            worker: self.worker.clone(),
            role: self.scope.role_name.clone(),
            project: self.scope.project.as_ref().map(|p| p.name.clone()),
            folder: self
                .workspace
                .as_ref()
                .map(|w| w.root().display().to_string()),
            permissions: self
                .levels
                .iter()
                .filter(|(c, l)| **l != Level::Blocked && c.has_tools())
                .map(|(c, l)| GrantPermission {
                    capability: *c,
                    label: c.label().into(),
                    level: *l,
                })
                .collect(),
            opened_at: self.opened_at,
            revoked: self.revoked,
            used: self.used,
            blocked: self.blocked,
            asked: self.asked,
        }
    }
}

#[derive(Default)]
struct State {
    grants: HashMap<String, Grant>,
    /// Ticket → grant ID.
    tickets: HashMap<String, String>,
    /// Approval ID → the tool call waiting for it.
    waiters: HashMap<String, oneshot::Sender<ApprovalState>>,
}

struct Inner {
    guard: Guard,
    supervisor: Supervisor,
    store: Arc<dyn SecretStore>,
    config: BrokerConfig,
    port: Mutex<Option<u16>>,
    handle: Mutex<Option<Handle>>,
    state: Mutex<State>,
    redactor: Arc<RwLock<Redactor>>,
    notices: Mutex<Vec<String>>,
}

/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct Broker {
    inner: Arc<Inner>,
}

/// What one tool call does, resolved and ready for Guard.
struct Prepared {
    capability: Capability,
    risk: Risk,
    summary: String,
    detail: String,
    files: Vec<Resolved>,
    writes_git_dir: bool,
    command: Option<CommandLine>,
    script: Option<String>,
    inherent: Option<(SensitiveKind, &'static str)>,
    work: Work,
}

/// The work itself, once allowed.
enum Work {
    List(Resolved),
    Read(Resolved, usize, usize),
    Search(Resolved, String, bool),
    Write(Resolved, String),
    Edit(Resolved, String, String, bool),
    Move(Resolved, Resolved),
    Delete(Resolved),
    /// Allowed, but it cannot be done here (a program that is not installed).
    Missing(String),
    Program {
        executable: PathBuf,
        args: Vec<String>,
        cwd: PathBuf,
        stdin: Option<Vec<u8>>,
        timeout: Duration,
        /// Git's own variables (git tools).
        git: bool,
    },
}

/// Why a call cannot even be put to Guard (the target is unusable).
struct Refused {
    layer: Layer,
    reason: String,
    summary: String,
}

fn cap(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

fn first_line(text: &str) -> String {
    cap(
        text.lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim(),
        300,
    )
}

/// A random ticket: two version-4 UUIDs (244 random bits).
fn new_ticket() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Write a file only its owner can read (on Unix; Windows keeps it in the user's profile).
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)?.write_all(text.as_bytes())
}

impl Broker {
    /// The broker. Approvals left pending when Plenipo stopped are marked expired, and old
    /// tickets are removed. Call [`Broker::start`] to open the tool server.
    pub fn new(
        guard: Guard,
        supervisor: Supervisor,
        store: Arc<dyn SecretStore>,
        config: BrokerConfig,
    ) -> Self {
        let this = Self {
            inner: Arc::new(Inner {
                guard,
                supervisor,
                store,
                config,
                port: Mutex::new(None),
                handle: Mutex::new(None),
                state: Mutex::new(State::default()),
                redactor: Arc::new(RwLock::new(Redactor::default())),
                notices: Mutex::new(Vec::new()),
            }),
        };
        match this
            .ledger()
            .expire_pending_approvals("Plenipo stopped before you answered.", PLENIPO)
        {
            Ok(0) => {}
            Ok(n) => this.notice(format!(
                "{n} approval request(s) from before Plenipo last stopped were marked expired."
            )),
            Err(e) => this.notice(format!("Could not settle earlier approval requests: {e}")),
        }
        let dir = &this.inner.config.tickets_dir;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.filter_map(std::result::Result::ok) {
                let _ = std::fs::remove_file(e.path());
            }
        }
        this.refresh_redactor();
        this
    }

    /// Open the tool server (on this async runtime).
    pub async fn start(&self) -> std::io::Result<u16> {
        *lock(&self.inner.handle) = Some(Handle::current());
        let port = crate::server::start(self.clone()).await?;
        *lock(&self.inner.port) = Some(port);
        Ok(port)
    }

    fn ledger(&self) -> &Arc<Ledger> {
        self.inner.guard.ledger()
    }

    /// The tool server's port, while it runs.
    pub fn port(&self) -> Option<u16> {
        *lock(&self.inner.port)
    }

    pub fn guard(&self) -> &Guard {
        &self.inner.guard
    }

    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.inner.state)
    }

    fn notice(&self, notice: String) {
        let mut n = lock(&self.inner.notices);
        if !n.contains(&notice) {
            n.push(notice);
        }
    }

    fn spawn(&self, work: impl std::future::Future<Output = ()> + Send + 'static) {
        let handle = lock(&self.inner.handle)
            .clone()
            .or_else(|| Handle::try_current().ok());
        if let Some(h) = handle {
            h.spawn(work);
        }
    }

    fn redactor(&self) -> Redactor {
        self.inner
            .redactor
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn redact(&self, text: &str) -> String {
        self.redactor().redact(text).into_owned()
    }

    /// Rebuild the redactor from the secrets in the Vault.
    pub fn refresh_redactor(&self) {
        let secrets = self
            .inner
            .guard
            .config()
            .map(|c| c.secrets)
            .unwrap_or_default();
        let known: Vec<(String, String)> = secrets
            .iter()
            .filter_map(|s| {
                self.inner
                    .store
                    .get(&s.id)
                    .ok()
                    .flatten()
                    .map(|v| (v, s.name.clone()))
            })
            .collect();
        *self
            .inner
            .redactor
            .write()
            .unwrap_or_else(|p| p.into_inner()) = Redactor::new(known);
    }

    /// A filter for the agent runtime: hides secrets in activity and results.
    pub fn text_filter(&self) -> TextFilter {
        let redactor = Arc::clone(&self.inner.redactor);
        Arc::new(move |text: &str| {
            redactor
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .redact(text)
                .into_owned()
        })
    }

    // ---- Grants -----------------------------------------------------------------------------

    fn try_open(&self, step: &StepInfo<'_>) -> Result<Option<StepTools>> {
        let workforce = &step.session.metadata["workforce"];
        let Some(scope) = self.inner.guard.scope_for(workforce)? else {
            return Ok(None);
        };
        let config = self.inner.guard.config()?;
        let levels = levels_for(&config, &scope);
        let permitted =
            |c: Capability| levels.get(&c).copied().unwrap_or_default() != Level::Blocked;
        let (workspace, problem) = match scope.project.as_ref().and_then(|p| p.folder.as_deref()) {
            Some(folder) => match Workspace::open(folder) {
                Ok(w) => (Some(w), None),
                Err(e) => (None, Some(e)),
            },
            None => (
                None,
                Some(match &scope.project {
                    Some(p) => format!("the {} project has no folder", p.name),
                    None => "this work belongs to no project, so there is no folder".into(),
                }),
            ),
        };
        let offered: Vec<&'static str> = TOOLS
            .iter()
            .filter(|t| {
                permitted(t.capability) && (!t.capability.needs_folder() || workspace.is_some())
            })
            .map(|t| t.name)
            .collect();
        let position_id = workforce["positionId"].as_str().map(str::to_owned);
        let worker = position_id
            .as_deref()
            .and_then(|id| self.ledger().position(id).ok().flatten())
            .map_or_else(|| scope.role_name.clone(), |p| p.title);
        if offered.is_empty() {
            if let (Some(problem), true) = (&problem, TOOLS.iter().any(|t| permitted(t.capability)))
            {
                // It has permissions it cannot use: say so where the owner will look.
                let _ = self.ledger().append_event(NewEvent {
                    task_id: Some(step.task_id.into()),
                    source: GUARD.into(),
                    event_type: "guard.grant_skipped".into(),
                    payload: json!({
                        "worker": worker,
                        "reason": format!("{worker} has permissions but got no tools: {problem}."),
                    }),
                    ..NewEvent::default()
                });
            }
            return Ok(None);
        }
        let Some(port) = *lock(&self.inner.port) else {
            self.notice("Plenipo's tool server is not running, so workers get no tools.".into());
            return Ok(None);
        };
        let dir = &self.inner.config.tickets_dir;
        std::fs::create_dir_all(dir)
            .map_err(|e| BrokerError::Invalid(format!("tickets folder: {e}")))?;
        let grant_id = uuid::Uuid::new_v4().to_string();
        let ticket = new_ticket();
        let ticket_path = dir.join(format!("{grant_id}.ticket.json"));
        let config_path = dir.join(format!("{grant_id}.mcp.json"));
        let mut args = self.inner.config.relay_args.clone();
        args.push(format!("{ARG}{}", ticket_path.display()));
        let write = || -> std::io::Result<()> {
            write_private(
                &ticket_path,
                &serde_json::to_string(&Ticket {
                    port,
                    ticket: ticket.clone(),
                })?,
            )?;
            write_private(
                &config_path,
                &json!({ "mcpServers": { SERVER_NAME: {
                    "type": "stdio",
                    "command": self.inner.config.relay_command.display().to_string(),
                    "args": args,
                } } })
                .to_string(),
            )
        };
        if let Err(e) = write() {
            let _ = std::fs::remove_file(&ticket_path);
            let _ = std::fs::remove_file(&config_path);
            return Err(BrokerError::Invalid(format!(
                "could not write the tool ticket: {e}"
            )));
        }
        let permissions: BTreeMap<String, Level> = levels
            .iter()
            .filter(|(c, l)| **l != Level::Blocked && c.has_tools())
            .map(|(c, l)| (c.id().to_owned(), *l))
            .collect();
        let folder = workspace.as_ref().map(|w| w.root().display().to_string());
        self.ledger().append_event(NewEvent {
            task_id: Some(step.task_id.into()),
            source: GUARD.into(),
            event_type: "guard.grant_opened".into(),
            payload: json!({
                "grantId": grant_id,
                "sessionId": step.session.id,
                "step": step.step,
                "positionId": position_id,
                "worker": worker,
                "role": scope.role_name,
                "project": scope.project.as_ref().map(|p| &p.name),
                "folder": folder,
                "permissions": permissions,
                "tools": offered,
                "note": problem,
            }),
            ..NewEvent::default()
        })?;
        let note = note_for(&scope, workspace.as_ref(), &levels, problem.as_deref());
        let grant = Grant {
            id: grant_id.clone(),
            ticket: ticket.clone(),
            files: vec![ticket_path, config_path.clone()],
            task_id: step.task_id.into(),
            session_id: step.session.id.clone(),
            runtime_id: step.session.runtime_id.clone(),
            step: step.step,
            position_id,
            worker,
            scope,
            workspace,
            levels,
            tools: offered,
            opened_at: plenipo_ledger::now_ms(),
            revoked: false,
            running: HashSet::new(),
            pending: HashSet::new(),
            used: 0,
            blocked: 0,
            asked: 0,
        };
        {
            let mut s = self.state();
            s.tickets.insert(ticket, grant_id.clone());
            s.grants.insert(grant_id.clone(), grant);
        }
        Ok(Some(StepTools {
            grant_id,
            server: ToolServer {
                name: SERVER_NAME.into(),
                command: self.inner.config.relay_command.clone(),
                args,
                config_file: config_path,
                call_timeout: self.inner.config.call_timeout,
            },
            note,
        }))
    }

    /// The grant a ticket belongs to, while it is open.
    pub fn grant_for_ticket(&self, ticket: &str) -> Option<String> {
        self.state().tickets.get(ticket).cloned()
    }

    fn end_grant(&self, grant_id: &str) {
        let removed = {
            let mut s = self.state();
            let g = s.grants.remove(grant_id);
            if let Some(g) = &g {
                s.tickets.remove(&g.ticket);
            }
            g
        };
        let Some(grant) = removed else { return };
        for f in &grant.files {
            let _ = std::fs::remove_file(f);
        }
        for execution in grant.running.clone() {
            let sup = self.inner.supervisor.clone();
            self.spawn(async move {
                let _ = sup.cancel(&execution).await;
            });
        }
        for approval in &grant.pending {
            self.settle(
                approval,
                ApprovalState::Expired,
                PLENIPO,
                "The worker's step ended before you answered.",
            );
        }
        let _ = self.ledger().append_event(NewEvent {
            task_id: Some(grant.task_id.clone()),
            source: GUARD.into(),
            event_type: "guard.grant_closed".into(),
            payload: json!({
                "grantId": grant.id,
                "worker": grant.worker,
                "used": grant.used,
                "blocked": grant.blocked,
                "asked": grant.asked,
                "revoked": grant.revoked,
            }),
            ..NewEvent::default()
        });
    }

    /// Settle a pending approval and wake the call waiting for it.
    fn settle(
        &self,
        approval_id: &str,
        state: ApprovalState,
        by: &str,
        note: &str,
    ) -> Option<Approval> {
        let settled = self
            .ledger()
            .settle_approval(approval_id, state, by, Some(note))
            .ok();
        let waiter = self.state().waiters.remove(approval_id);
        if let Some(tx) = waiter {
            let actual = settled.as_ref().map_or(state, |a| a.state);
            let _ = tx.send(actual);
        }
        settled
    }

    /// End a grant now: its running programs stop, its pending approvals are refused, and its
    /// later calls are blocked (the owner's "Revoke").
    pub fn revoke(&self, grant_id: &str, actor: &str) -> Result<GrantView> {
        let (view, pending, running, task_id) = {
            let mut s = self.state();
            let g = s.grants.get_mut(grant_id).ok_or_else(|| {
                BrokerError::Invalid("that worker's step has already ended".into())
            })?;
            g.revoked = true;
            (
                g.view(),
                std::mem::take(&mut g.pending),
                std::mem::take(&mut g.running),
                g.task_id.clone(),
            )
        };
        for approval in &pending {
            self.settle(
                approval,
                ApprovalState::Rejected,
                actor,
                "Permissions revoked.",
            );
        }
        for execution in running {
            let sup = self.inner.supervisor.clone();
            self.spawn(async move {
                let _ = sup.cancel(&execution).await;
            });
        }
        self.ledger().append_event(NewEvent {
            task_id: Some(task_id),
            source: actor.into(),
            event_type: "guard.grant_revoked".into(),
            payload: json!({ "grantId": grant_id, "worker": view.worker }),
            ..NewEvent::default()
        })?;
        Ok(view)
    }

    pub fn grants(&self) -> Vec<GrantView> {
        let mut out: Vec<GrantView> = self.state().grants.values().map(Grant::view).collect();
        out.sort_by_key(|g| g.opened_at);
        out
    }

    // ---- MCP ----------------------------------------------------------------------------------

    /// Server instructions for a grant (shown by AI tools that support them).
    pub fn instructions(&self, grant_id: &str) -> String {
        let s = self.state();
        s.grants.get(grant_id).map_or_else(String::new, |g| {
            note_for(&g.scope, g.workspace.as_ref(), &g.levels, None)
        })
    }

    /// The tools a grant offers, as MCP lists them.
    pub fn tool_list(&self, grant_id: &str) -> Vec<Value> {
        let s = self.state();
        let Some(g) = s.grants.get(grant_id) else {
            return Vec::new();
        };
        let folder = g
            .workspace
            .as_ref()
            .map(|w| w.root().display().to_string())
            .unwrap_or_default();
        g.tools
            .iter()
            .filter_map(|name| tools::find(name))
            .map(|t| {
                let level = g.levels.get(&t.capability).copied().unwrap_or_default();
                let mut description = t.description.to_owned();
                if t.capability.needs_folder() {
                    description.push_str(&format!(" Project folder: {folder}."));
                }
                if level == Level::Ask {
                    description.push_str(" Each use waits for the owner's approval.");
                }
                json!({ "name": t.name, "description": description, "inputSchema": t.schema() })
            })
            .collect()
    }

    /// Carry out one tool call for a grant.
    pub async fn call(&self, grant_id: &str, name: &str, args: Value) -> CallResult {
        let Some(tool) = tools::find(name) else {
            return CallResult::error(format!("There is no tool named {name}."));
        };
        let context = {
            let s = self.state();
            s.grants.get(grant_id).map(|g| {
                (
                    g.scope.clone(),
                    g.workspace.clone(),
                    g.levels.get(&tool.capability).copied().unwrap_or_default(),
                    g.revoked,
                    g.task_id.clone(),
                    g.runtime_id.clone(),
                    g.worker.clone(),
                )
            })
        };
        let Some((scope, workspace, grant_level, revoked, task_id, runtime_id, worker)) = context
        else {
            return CallResult::error("This task step has ended; its tools are closed.");
        };
        let action = match tools::parse(tool, &args) {
            Ok(a) => a,
            Err(e) => return CallResult::error(format!("{}: {e}", tool.name)),
        };
        let config = match self.inner.guard.config() {
            Ok(c) => c,
            Err(e) => {
                return CallResult::error(format!(
                    "Plenipo could not read its permission settings: {e}"
                ))
            }
        };
        let prepared = match prepare(
            tool,
            action,
            workspace.as_ref(),
            self.inner.config.command_timeout,
        ) {
            Ok(p) => p,
            Err(r) => {
                let decision = Decision {
                    verdict: Verdict::Deny,
                    reason: format!("Blocked: {}", r.reason),
                    layer: r.layer,
                    risk: tool.risk,
                    sensitive: None,
                    checks: Vec::new(),
                };
                return self.deny(grant_id, &task_id, &worker, tool, &r.summary, "", &decision);
            }
        };
        let current = level_for(&config, &scope, prepared.capability);
        let rels: Vec<String> = prepared.files.iter().map(|f| f.rel.clone()).collect();
        let root = workspace
            .as_ref()
            .map_or_else(PathBuf::new, |w| w.root().to_path_buf());
        let request = Request {
            capability: prepared.capability,
            risk: prepared.risk,
            summary: &prepared.summary,
            files: &rels,
            writes_git_dir: prepared.writes_git_dir,
            command: prepared.command.as_ref(),
            script: prepared.script.as_deref(),
            inherent: prepared.inherent,
            workspace: &root,
        };
        let decision = evaluate(
            &config,
            &request,
            &current,
            GrantState {
                level: grant_level,
                revoked,
            },
        );
        let detail = self.redact(&prepared.detail);
        let mut approval_id = None;
        match decision.verdict {
            Verdict::Deny => {
                return self.deny(
                    grant_id,
                    &task_id,
                    &worker,
                    tool,
                    &prepared.summary,
                    &detail,
                    &decision,
                )
            }
            Verdict::Ask => {
                let minutes = config.options.approval_minutes;
                match self
                    .ask(
                        grant_id,
                        &task_id,
                        &runtime_id,
                        &worker,
                        &scope,
                        workspace.as_ref(),
                        tool,
                        &prepared,
                        &detail,
                        &decision,
                        minutes,
                    )
                    .await
                {
                    Ok((id, ApprovalState::Approved)) => approval_id = Some(id),
                    Ok((_, state)) => {
                        let why = match state {
                            ApprovalState::Rejected => "the owner did not approve it",
                            _ => "the owner did not answer in time",
                        };
                        return CallResult::error(format!(
                            "Not done: {why} ({}). Do not try to do this another way; say in \
                             your answer what you needed and why.",
                            prepared.summary
                        ));
                    }
                    Err(e) => return CallResult::error(format!("Not done: {e}")),
                }
                // Revoked while waiting?
                if self.state().grants.get(grant_id).is_none_or(|g| g.revoked) {
                    return CallResult::error("Not done: this worker's permissions were revoked.");
                }
            }
            Verdict::Allow => {}
        }
        let (outcome, execution) = self
            .carry_out(
                grant_id,
                &worker,
                workspace.as_ref(),
                prepared.work,
                &prepared.summary,
            )
            .await;
        let (text, ok) = match outcome {
            Ok(text) => (text, true),
            Err(text) => (text, false),
        };
        let text = self.redact(&text);
        if let Some(g) = self.state().grants.get_mut(grant_id) {
            g.used += 1;
        }
        let _ = self.ledger().append_event(NewEvent {
            task_id: Some(task_id),
            execution_id: execution.clone(),
            source: format!("agent:{runtime_id}"),
            event_type: "capability.used".into(),
            payload: json!({
                "grantId": grant_id,
                "worker": worker,
                "tool": tool.name,
                "capability": prepared.capability,
                "summary": self.redact(&prepared.summary),
                "detail": cap(&detail, MAX_DETAIL),
                "ok": ok,
                "result": first_line(&text),
                "approvalId": approval_id,
                "executionId": execution,
            }),
            ..NewEvent::default()
        });
        if ok {
            CallResult::ok(text)
        } else {
            CallResult::error(text)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn deny(
        &self,
        grant_id: &str,
        task_id: &str,
        worker: &str,
        tool: &ToolDef,
        summary: &str,
        detail: &str,
        decision: &Decision,
    ) -> CallResult {
        if let Some(g) = self.state().grants.get_mut(grant_id) {
            g.blocked += 1;
        }
        let summary = self.redact(summary);
        let _ = self.ledger().append_event(NewEvent {
            task_id: Some(task_id.into()),
            source: GUARD.into(),
            event_type: "guard.denied".into(),
            payload: json!({
                "grantId": grant_id,
                "worker": worker,
                "tool": tool.name,
                "capability": tool.capability,
                "summary": summary,
                "detail": cap(detail, MAX_DETAIL),
                "reason": decision.reason,
                "layer": decision.layer,
                "checks": decision.checks,
            }),
            ..NewEvent::default()
        });
        CallResult::error(format!(
            "{} ({summary}) Do not try to get around this; if you need it, say so in your \
             answer and the owner can change your permissions.",
            decision.reason
        ))
    }

    #[allow(clippy::too_many_arguments)]
    async fn ask(
        &self,
        grant_id: &str,
        task_id: &str,
        runtime_id: &str,
        worker: &str,
        scope: &Scope,
        workspace: Option<&Workspace>,
        tool: &ToolDef,
        prepared: &Prepared,
        detail: &str,
        decision: &Decision,
        minutes: u32,
    ) -> std::result::Result<(String, ApprovalState), String> {
        let now = plenipo_ledger::now_ms();
        let wait = self.inner.config.approval_minute * minutes;
        let expires_at = now + wait.as_millis() as u64;
        let payload = json!({
            "summary": self.redact(&prepared.summary),
            "detail": cap(detail, MAX_DETAIL),
            "reason": decision.reason,
            "risk": prepared.risk,
            "riskLabel": prepared.risk.words(),
            "sensitive": decision.sensitive,
            "sensitiveLabel": decision.sensitive.map(SensitiveKind::label),
            "capability": prepared.capability,
            "capabilityLabel": prepared.capability.label(),
            "tool": tool.name,
            "worker": worker,
            "role": scope.role_name,
            "project": scope.project.as_ref().map(|p| &p.name),
            "folder": workspace.map(|w| w.root().display().to_string()),
            "grantId": grant_id,
            "sessionId": self.state().grants.get(grant_id).map(|g| g.session_id.clone()),
        });
        let (tx, rx) = oneshot::channel();
        let approval = self
            .ledger()
            .request_action_approval(
                task_id,
                prepared.capability.id(),
                &payload,
                expires_at,
                &format!("agent:{runtime_id}"),
            )
            .map_err(|e| format!("the approval could not be requested: {e}"))?;
        {
            let mut s = self.state();
            s.waiters.insert(approval.id.clone(), tx);
            if let Some(g) = s.grants.get_mut(grant_id) {
                g.asked += 1;
                g.pending.insert(approval.id.clone());
            } else {
                drop(s);
                let a = self.settle(
                    &approval.id,
                    ApprovalState::Expired,
                    PLENIPO,
                    "The worker's step ended.",
                );
                return Ok((approval.id, a.map_or(ApprovalState::Expired, |a| a.state)));
            }
        }
        let state = match tokio::time::timeout(wait, rx).await {
            Ok(Ok(state)) => state,
            Ok(Err(_)) => self
                .ledger()
                .approval(&approval.id)
                .ok()
                .flatten()
                .map_or(ApprovalState::Expired, |a| a.state),
            Err(_) => {
                let note = format!("No answer within {minutes} minute(s).");
                match self.settle(&approval.id, ApprovalState::Expired, PLENIPO, &note) {
                    Some(a) => a.state,
                    None => self
                        .ledger()
                        .approval(&approval.id)
                        .ok()
                        .flatten()
                        .map_or(ApprovalState::Expired, |a| a.state),
                }
            }
        };
        if let Some(g) = self.state().grants.get_mut(grant_id) {
            g.pending.remove(&approval.id);
        }
        Ok((approval.id, state))
    }

    /// The owner's answer to an approval.
    pub fn resolve_approval(
        &self,
        approval_id: &str,
        approve: bool,
        actor: &str,
    ) -> Result<ApprovalView> {
        let current = self
            .ledger()
            .approval(approval_id)?
            .ok_or_else(|| BrokerError::Invalid("that approval request no longer exists".into()))?;
        if current.state != ApprovalState::Pending {
            return Err(BrokerError::Invalid(format!(
                "that request was already {}",
                match current.state {
                    ApprovalState::Approved => "approved",
                    ApprovalState::Rejected => "refused",
                    _ => "expired",
                }
            )));
        }
        if current
            .expires_at
            .is_some_and(|t| t <= plenipo_ledger::now_ms())
        {
            self.settle(
                approval_id,
                ApprovalState::Expired,
                PLENIPO,
                "It expired before your answer.",
            );
            return Err(BrokerError::Invalid(
                "that request expired before your answer".into(),
            ));
        }
        let state = if approve {
            ApprovalState::Approved
        } else {
            ApprovalState::Rejected
        };
        let note = if approve {
            "Approved by you."
        } else {
            "Refused by you."
        };
        let settled = self
            .settle(approval_id, state, actor, note)
            .ok_or_else(|| {
                BrokerError::Invalid(
                    "that request could not be answered; it may have just ended".into(),
                )
            })?;
        Ok(self.approval_view(&settled, None))
    }

    // ---- Carrying out -------------------------------------------------------------------------

    async fn carry_out(
        &self,
        grant_id: &str,
        worker: &str,
        workspace: Option<&Workspace>,
        work: Work,
        summary: &str,
    ) -> (std::result::Result<String, String>, Option<String>) {
        let blocking = |f: Box<dyn FnOnce() -> std::result::Result<String, String> + Send>| async move {
            tokio::task::spawn_blocking(f)
                .await
                .unwrap_or_else(|e| Err(format!("the work stopped unexpectedly: {e}")))
        };
        match work {
            Work::List(p) => (blocking(Box::new(move || files::list(&p))).await, None),
            Work::Read(p, o, l) => (
                blocking(Box::new(move || files::read(&p, o, l))).await,
                None,
            ),
            Work::Search(p, q, c) => {
                let blocked = self
                    .inner
                    .guard
                    .config()
                    .map(|c| c.blocked_files)
                    .unwrap_or_default();
                let Some(ws) = workspace.cloned() else {
                    return (Err("no project folder".into()), None);
                };
                (
                    blocking(Box::new(move || files::search(&ws, &p, &q, c, &blocked))).await,
                    None,
                )
            }
            Work::Write(p, c) => (blocking(Box::new(move || files::write(&p, &c))).await, None),
            Work::Edit(p, o, n, a) => (
                blocking(Box::new(move || files::edit(&p, &o, &n, a))).await,
                None,
            ),
            Work::Move(f, t) => (
                blocking(Box::new(move || files::move_path(&f, &t))).await,
                None,
            ),
            Work::Delete(p) => (blocking(Box::new(move || files::delete(&p))).await, None),
            Work::Missing(why) => (Err(why), None),
            Work::Program {
                executable,
                args,
                cwd,
                stdin,
                timeout,
                git,
            } => {
                let mut env = programs::dev_env();
                if git {
                    env.extend(programs::git_env());
                }
                let program = CommandLine {
                    program: executable
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    args: Vec::new(),
                }
                .program_name();
                let mut secrets_used = Vec::new();
                if let Ok(config) = self.inner.guard.config() {
                    for s in config
                        .secrets
                        .iter()
                        .filter(|s| s.programs.contains(&program))
                    {
                        if let (Some(var), Ok(Some(value))) =
                            (&s.env_var, self.inner.store.get(&s.id))
                        {
                            env.push((var.clone(), value));
                            secrets_used.push(s.name.clone());
                        }
                    }
                }
                let this = self.clone();
                let grant = grant_id.to_owned();
                let ran = programs::run(
                    &self.inner.supervisor,
                    Run {
                        label: cap(&format!("{worker} · {summary}"), 200),
                        executable,
                        args,
                        working_dir: &cwd,
                        env,
                        stdin,
                        timeout,
                    },
                    move |id| {
                        if let Some(g) = this.state().grants.get_mut(&grant) {
                            g.running.insert(id.to_owned());
                        } else {
                            // Ended while starting: stop it.
                            let (sup, id) = (this.inner.supervisor.clone(), id.to_owned());
                            this.spawn(async move {
                                let _ = sup.cancel(&id).await;
                            });
                        }
                    },
                )
                .await;
                match ran {
                    Ok(ran) => {
                        if let Some(g) = self.state().grants.get_mut(grant_id) {
                            g.running.remove(&ran.execution_id);
                        }
                        let mut text = String::new();
                        if !secrets_used.is_empty() {
                            text.push_str(&format!(
                                "(Given the stored secret(s) {} by Plenipo.)\n",
                                secrets_used.join(", ")
                            ));
                        }
                        let output = ran.output.trim_end();
                        text.push_str(if output.is_empty() {
                            "(no output)"
                        } else {
                            output
                        });
                        text.push('\n');
                        text.push_str(&ran.ending(timeout));
                        let id = Some(ran.execution_id.clone());
                        if ran.succeeded() {
                            (Ok(text), id)
                        } else {
                            (Err(text), id)
                        }
                    }
                    Err(e) => (Err(e), None),
                }
            }
        }
    }

    // ---- Views --------------------------------------------------------------------------------

    fn approval_view(&self, a: &Approval, waiting: Option<&HashSet<String>>) -> ApprovalView {
        let p = &a.request_payload;
        let s = |k: &str| p[k].as_str().map(str::to_owned);
        let capability = p["capability"]
            .as_str()
            .and_then(Capability::parse)
            .or_else(|| Capability::parse(&a.action_type));
        let risk: Option<Risk> = serde_json::from_value(p["risk"].clone()).ok();
        let sensitive: Option<SensitiveKind> = serde_json::from_value(p["sensitive"].clone()).ok();
        ApprovalView {
            id: a.id.clone(),
            task_id: a.task_id.clone(),
            status: a.state.into(),
            requested_at: a.requested_at,
            expires_at: a.expires_at,
            resolved_at: a.resolved_at,
            resolved_by: a.resolved_by.clone(),
            worker: s("worker").unwrap_or_else(|| "A worker".into()),
            role: s("role").unwrap_or_default(),
            project: s("project"),
            folder: s("folder"),
            capability,
            capability_label: s("capabilityLabel")
                .or_else(|| capability.map(|c| c.label().to_owned()))
                .unwrap_or_else(|| a.action_type.clone()),
            summary: s("summary").unwrap_or_else(|| a.action_type.clone()),
            detail: s("detail").unwrap_or_default(),
            reason: s("reason").unwrap_or_default(),
            risk,
            risk_label: s("riskLabel").unwrap_or_default(),
            sensitive,
            sensitive_label: s("sensitiveLabel"),
            session_id: s("sessionId"),
            grant_id: s("grantId"),
            waiting: waiting.is_some_and(|w| w.contains(&a.id)),
            note: None,
        }
    }

    /// Pending approvals and recent outcomes.
    pub fn approvals(&self) -> Result<ApprovalQueue> {
        let waiting: HashSet<String> = self.state().waiters.keys().cloned().collect();
        let pending = self
            .ledger()
            .pending_approvals()?
            .iter()
            .map(|a| self.approval_view(a, Some(&waiting)))
            .collect();
        let notes: HashMap<String, String> = self
            .ledger()
            .events_of_types(&["approval.resolved", "approval.expired"], 100)?
            .into_iter()
            .filter_map(|e| {
                Some((
                    e.payload["approvalId"].as_str()?.to_owned(),
                    e.payload["note"].as_str()?.to_owned(),
                ))
            })
            .collect();
        let recent = self
            .ledger()
            .settled_approvals(20)?
            .iter()
            .map(|a| {
                let mut v = self.approval_view(a, None);
                v.note = notes.get(&a.id).cloned();
                v
            })
            .collect();
        Ok(ApprovalQueue { pending, recent })
    }

    /// Recently blocked requests.
    pub fn blocked(&self, limit: u32) -> Result<Vec<BlockedView>> {
        Ok(self
            .ledger()
            .events_of_types(&["guard.denied"], limit)?
            .into_iter()
            .map(|e| {
                let p = &e.payload;
                let s = |k: &str| p[k].as_str().unwrap_or_default().to_owned();
                BlockedView {
                    at: e.created_at,
                    task_id: e.task_id.clone(),
                    worker: s("worker"),
                    capability_label: p["capability"]
                        .as_str()
                        .and_then(Capability::parse)
                        .map_or_else(String::new, |c| c.label().to_owned()),
                    summary: s("summary"),
                    reason: s("reason"),
                }
            })
            .collect())
    }

    pub fn vault_status(&self) -> VaultStatus {
        let store = &self.inner.store;
        let check = store.check();
        let stored = match &check {
            Ok(()) => self
                .inner
                .guard
                .config()
                .map(|c| c.secrets)
                .unwrap_or_default()
                .into_iter()
                .filter(|s| store.get(&s.id).ok().flatten().is_some())
                .map(|s| s.id)
                .collect(),
            Err(_) => Vec::new(),
        };
        VaultStatus {
            available: check.is_ok(),
            label: store.label().to_owned(),
            detail: check.err(),
            stored,
        }
    }

    /// Everything the Permissions page shows besides the approval queue.
    pub fn snapshot(&self) -> Result<PermissionsSnapshot> {
        let port = *lock(&self.inner.port);
        let mut notices = self.inner.guard.notices();
        notices.extend(lock(&self.inner.notices).iter().cloned());
        Ok(PermissionsSnapshot {
            settings: self.inner.guard.settings()?,
            vault: self.vault_status(),
            tools: ToolsStatus {
                running: port.is_some(),
                detail: match port {
                    Some(_) => "Ready: workers with permissions get Plenipo's tools.".into(),
                    None => "Not running: workers get no tools until Plenipo restarts.".into(),
                },
            },
            grants: self.grants(),
            blocked: self.blocked(20)?,
            notices,
        })
    }

    // ---- Vault --------------------------------------------------------------------------------

    pub fn save_secret(&self, input: &plenipo_guard::SecretInput) -> Result<()> {
        vault::save(&self.inner.guard, self.inner.store.as_ref(), input)?;
        self.refresh_redactor();
        Ok(())
    }

    pub fn remove_secret(&self, id: &str) -> Result<()> {
        vault::remove(&self.inner.guard, self.inner.store.as_ref(), id)?;
        self.refresh_redactor();
        Ok(())
    }
}

impl ToolProvider for Broker {
    fn open(&self, step: &StepInfo<'_>) -> Option<StepTools> {
        match self.try_open(step) {
            Ok(tools) => tools,
            Err(e) => {
                self.notice(format!("A worker got no tools because of an error: {e}"));
                None
            }
        }
    }

    fn close(&self, grant_id: &str) {
        self.end_grant(grant_id);
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// What the worker is told about its tools.
fn note_for(
    scope: &Scope,
    workspace: Option<&Workspace>,
    levels: &BTreeMap<Capability, Level>,
    problem: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    let project = scope.project.as_ref().map_or_else(
        || "your work".to_owned(),
        |p| format!("the {} project", p.name),
    );
    lines.push(format!(
        "You can use Plenipo's tools (the \"plenipo\" tools) for {project}. They are the only way \
         to open or change its files, run programs, or use git; your AI tool's own tools are not \
         available for this."
    ));
    match workspace {
        Some(w) => lines.push(format!(
            "The project folder is {}. Give paths relative to it; nothing outside it can be used.",
            w.root().display()
        )),
        None => lines.push(format!(
            "You have no project folder ({}), so file, program, and git tools are not available.",
            problem.unwrap_or("none is set")
        )),
    }
    let mut allowed = Vec::new();
    for (c, l) in levels {
        if *l == Level::Blocked || !c.has_tools() || (c.needs_folder() && workspace.is_none()) {
            continue;
        }
        let what = doing(*c);
        allowed.push(match (c, l) {
            (Capability::ShellExec, Level::Allowed) => {
                format!(
                    "{what} (approved commands run at once; others wait for the owner's approval)"
                )
            }
            (Capability::GitWrite, Level::Allowed) => {
                format!("{what} (pushing to a server waits for the owner's approval)")
            }
            (_, Level::Ask) => format!("{what} (each time after the owner approves)"),
            _ => what.to_owned(),
        });
    }
    if !allowed.is_empty() {
        lines.push(format!("You may: {}.", allowed.join("; ")));
    }
    lines.push(
        "Every use is checked and recorded. If a tool says an action was blocked or not \
         approved, do not try another way around it: say in your answer what you needed and why."
            .into(),
    );
    lines.join("\n")
}

/// Resolve a path, or refuse the call.
fn resolve(
    ws: Option<&Workspace>,
    path: &str,
    summary: &str,
) -> std::result::Result<Resolved, Refused> {
    let Some(ws) = ws else {
        return Err(Refused {
            layer: Layer::Target,
            reason: "there is no project folder to work in.".into(),
            summary: summary.into(),
        });
    };
    ws.resolve(path).map_err(|r| Refused {
        layer: Layer::Target,
        reason: match r {
            PathRefusal::Invalid(m) | PathRefusal::Outside(m) => format!("{m}."),
        },
        summary: summary.into(),
    })
}

/// Resolve a program a worker names: a bare name on PATH, or `./path` inside the project. The
/// name Guard matches rules against comes back even when the program is not installed.
fn program_path(
    ws: &Workspace,
    program: &str,
    summary: &str,
) -> std::result::Result<(std::result::Result<PathBuf, String>, String), Refused> {
    let refuse = |reason: String| Refused {
        layer: Layer::Target,
        reason,
        summary: summary.into(),
    };
    if program.contains(['/', '\\']) {
        let b = program.as_bytes();
        let absolute = program.starts_with('/')
            || program.starts_with('\\')
            || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':');
        if absolute {
            return Err(refuse(format!(
                "{program} is a full path; name a program (like cargo) or a program inside the \
                 project folder (like ./gradlew)."
            )));
        }
        let r = ws
            .resolve(program)
            .map_err(|e| refuse(format!("{}.", e.message())))?;
        if !r.abs.is_file() {
            return Err(refuse(format!(
                "{} is not a file in the project folder.",
                r.shown()
            )));
        }
        return Ok((Ok(r.abs.clone()), format!("./{}", r.rel)));
    }
    let found = programs::find_on_path(program)
        .ok_or_else(|| format!("{program} is not installed (it was not found on PATH)."));
    Ok((found, program.to_owned()))
}

/// Read a call into what Guard checks and what Plenipo then does.
fn prepare(
    tool: &ToolDef,
    action: Action,
    ws: Option<&Workspace>,
    default_timeout: Duration,
) -> std::result::Result<Prepared, Refused> {
    let base = |summary: String, detail: String, files: Vec<Resolved>, work: Work| {
        let writes_git_dir = tool.capability == Capability::FilesystemWrite
            && files.iter().any(Resolved::in_git_dir);
        Prepared {
            capability: tool.capability,
            risk: tool.risk,
            summary,
            detail,
            files,
            writes_git_dir,
            command: None,
            script: None,
            inherent: None,
            work,
        }
    };
    let cwd_of = |cwd: &str, summary: &str| -> std::result::Result<PathBuf, Refused> {
        let r = resolve(ws, cwd, summary)?;
        if !r.abs.is_dir() {
            return Err(Refused {
                layer: Layer::Target,
                reason: format!("{} is not a folder in the project.", r.shown()),
                summary: summary.into(),
            });
        }
        Ok(r.abs)
    };
    let timeout_of = |t: Option<u64>| t.map_or(default_timeout, Duration::from_secs);
    Ok(match action {
        Action::List { path } => {
            let s = format!("list {path}");
            let r = resolve(ws, &path, &s)?;
            base(
                format!("list {}", r.shown()),
                r.shown().into(),
                vec![],
                Work::List(r),
            )
        }
        Action::Read {
            path,
            offset,
            limit,
        } => {
            let s = format!("read {path}");
            let r = resolve(ws, &path, &s)?;
            base(
                format!("read {}", r.shown()),
                r.shown().into(),
                vec![r.clone()],
                Work::Read(r, offset, limit),
            )
        }
        Action::Search {
            query,
            path,
            case_sensitive,
        } => {
            let s = format!("search for {query:?}");
            let r = resolve(ws, &path, &s)?;
            base(
                format!("search {} for {query:?}", r.shown()),
                format!("{query:?} in {}", r.shown()),
                vec![],
                Work::Search(r, query, case_sensitive),
            )
        }
        Action::Write { path, content } => {
            let s = format!("write {path}");
            let r = resolve(ws, &path, &s)?;
            base(
                format!("write {}", r.shown()),
                format!("{} ({} bytes)", r.shown(), content.len()),
                vec![r.clone()],
                Work::Write(r, content),
            )
        }
        Action::Edit {
            path,
            old,
            new,
            all,
        } => {
            let s = format!("edit {path}");
            let r = resolve(ws, &path, &s)?;
            base(
                format!("edit {}", r.shown()),
                format!(
                    "{}: replace {:?} with {:?}",
                    r.shown(),
                    cap(&old, 200),
                    cap(&new, 200)
                ),
                vec![r.clone()],
                Work::Edit(r, old, new, all),
            )
        }
        Action::Move { from, to } => {
            let s = format!("move {from} to {to}");
            let f = resolve(ws, &from, &s)?;
            let t = resolve(ws, &to, &s)?;
            base(
                format!("move {} to {}", f.shown(), t.shown()),
                format!("{} → {}", f.shown(), t.shown()),
                vec![f.clone(), t.clone()],
                Work::Move(f, t),
            )
        }
        Action::Delete { path } => {
            let s = format!("delete {path}");
            let r = resolve(ws, &path, &s)?;
            base(
                format!("delete {}", r.shown()),
                r.shown().into(),
                vec![r.clone()],
                Work::Delete(r),
            )
        }
        Action::Run {
            program,
            args,
            cwd,
            timeout,
        } => {
            let shown = CommandLine {
                program: program.clone(),
                args: args.clone(),
            }
            .shown();
            let s = format!("run {shown}");
            let Some(w) = ws else {
                return Err(Refused {
                    layer: Layer::Target,
                    reason: "there is no project folder to run it in.".into(),
                    summary: s,
                });
            };
            let (executable, key) = program_path(w, &program, &s)?;
            let cwd = cwd_of(&cwd, &s)?;
            let command = CommandLine {
                program: key,
                args: args.clone(),
            };
            let work = match executable {
                Ok(executable) => Work::Program {
                    executable,
                    args,
                    cwd,
                    stdin: None,
                    timeout: timeout_of(timeout),
                    git: false,
                },
                Err(why) => Work::Missing(why),
            };
            let mut p = base(
                format!("run {}", command.shown()),
                command.shown(),
                vec![],
                work,
            );
            p.command = Some(command);
            p
        }
        Action::Script {
            script,
            cwd,
            timeout,
        } => {
            let first = script
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim();
            let s = format!("run a PowerShell script ({})", cap(first, 60));
            let cwd = cwd_of(&cwd, &s)?;
            let work = match programs::powershell() {
                Some(ps) => Work::Program {
                    executable: ps,
                    args: programs::powershell_args(),
                    cwd,
                    stdin: Some(script.clone().into_bytes()),
                    timeout: timeout_of(timeout),
                    git: false,
                },
                None => Work::Missing("PowerShell is not installed on this computer.".into()),
            };
            let mut p = base(s, script.clone(), vec![], work);
            p.script = Some(script);
            p
        }
        git_action => {
            let s = tool.name.replace('_', " ");
            let Some(w) = ws else {
                return Err(Refused {
                    layer: Layer::Target,
                    reason: "there is no project folder (so no repository).".into(),
                    summary: s,
                });
            };
            let git = programs::find_on_path("git");
            let rel = |p: &str| -> std::result::Result<String, Refused> {
                let r = resolve(ws, p, &s)?;
                Ok(if r.rel.is_empty() { ".".into() } else { r.rel })
            };
            let mut inherent = None;
            let (summary, op): (String, Vec<String>) = match git_action {
                Action::GitStatus => (
                    "git status".into(),
                    vec!["status".into(), "--short".into(), "--branch".into()],
                ),
                Action::GitDiff { staged, path } => {
                    let mut op = vec!["diff".to_owned()];
                    if staged {
                        op.push("--cached".into());
                    }
                    if let Some(p) = path {
                        op.extend(["--".into(), rel(&p)?]);
                    }
                    ("git diff".into(), op)
                }
                Action::GitLog { count, path } => {
                    let mut op = vec![
                        "log".to_owned(),
                        "--oneline".into(),
                        "--decorate".into(),
                        "-n".into(),
                        count.to_string(),
                    ];
                    if let Some(p) = path {
                        op.extend(["--".into(), rel(&p)?]);
                    }
                    ("git log".into(), op)
                }
                Action::GitAdd { paths } => {
                    let mut op = vec!["add".to_owned(), "--".into()];
                    for p in &paths {
                        op.push(rel(p)?);
                    }
                    (format!("git add {}", op[2..].join(" ")), op)
                }
                Action::GitCommit { message } => (
                    format!(
                        "git commit ({})",
                        cap(message.lines().next().unwrap_or(""), 80)
                    ),
                    vec!["commit".into(), "-m".into(), message],
                ),
                Action::GitBranch { name, create } => {
                    let op = if create {
                        vec!["switch".into(), "-c".into(), name.clone()]
                    } else {
                        vec!["switch".into(), name.clone()]
                    };
                    (
                        format!("git switch {}{name}", if create { "-c " } else { "" }),
                        op,
                    )
                }
                Action::GitPush { remote, branch } => {
                    inherent = Some((SensitiveKind::Outbound, "it sends commits to a server"));
                    let mut op = vec!["push".to_owned(), remote.clone()];
                    if let Some(b) = &branch {
                        op.push(b.clone());
                    }
                    (format!("git push {}", op[1..].join(" ")), op)
                }
                _ => unreachable!("file and program actions are handled above"),
            };
            let args = programs::git_args(w.root(), &op);
            let work = match git {
                Some(executable) => Work::Program {
                    executable,
                    args,
                    cwd: w.root().to_path_buf(),
                    stdin: None,
                    timeout: default_timeout,
                    git: true,
                },
                None => Work::Missing("git is not installed (it was not found on PATH).".into()),
            };
            let mut p = base(
                summary.clone(),
                format!("git {}", op.join(" ")),
                vec![],
                work,
            );
            p.inherent = inherent;
            p
        }
    })
}
