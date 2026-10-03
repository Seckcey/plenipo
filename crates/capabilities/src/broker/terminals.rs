//! The owner's terminal, through the broker (Phase 12, ADR-031): a shell on this PC, or on one
//! of the owner's servers from Settings → Servers.
//!
//! - **This PC:** the shell chosen in Settings → Terminal (Windows PowerShell by default), in the
//!   owner's home folder, as the owner's own Windows user, never as administrator.
//! - **A server:** only while Settings → Switches → Remote computers (SSH) is on. The pinned
//!   server ID is checked first; a changed ID is refused before Plenipo signs in or sends
//!   anything. The sign-in comes from the Vault, is never shown, and is dropped after use. Only
//!   this terminal asks the server for a terminal and a shell; workers never do.
//! - **The owner is in charge:** Guard does not check what the owner types, and no approval cards
//!   are shown for it. **Nothing typed or shown is recorded**, only that a terminal opened and
//!   closed: where, when, and for how long (`terminal.opened`, `terminal.closed`).
//! - **Stop all** stops workers. It does not close the owner's terminals: they are kept apart
//!   from every worker's grant and control session.
//! - **An AI tool's sign-in tab** (Phase 19, ADR-058) runs that tool's own sign-in or sign-out
//!   program, built by Plenipo's AI tools service from the tool's fixed list, directly (no
//!   shell) and with only the environment the tool's tasks get. It opens only through that
//!   service ([`crate::ai_tools`]), never from a place alone.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::Engine as _;
use serde_json::json;

use super::{lock, Broker};
use crate::control::{ControlKind, ControlState};
use crate::dto::*;
use crate::error::{BrokerError, Result};
use crate::ssh::{self, ConnectError, Endpoint};
use crate::terminal::{self, Ending, LocalShell, RemoteShell, Size};

/// The Ledger setting that holds the owner's preferences (the terminal's shell, the notices).
pub const PREFERENCES: &str = "preferences";
/// Who does all of this.
const OWNER: &str = "owner";
/// Most terminals open at once.
pub const MAX_TERMINALS: usize = 12;

/// Where a terminal's output and its end go (the owner's screen).
pub type TerminalSink = Arc<dyn Fn(TerminalEvent) + Send + Sync>;

enum Shell {
    Local(LocalShell),
    Remote(RemoteShell),
}

struct Open {
    info: TerminalInfo,
    started: Instant,
    shell: Mutex<Option<Shell>>,
    ended: AtomicBool,
}

/// The owner's open terminals, apart from every worker's grant.
#[derive(Default)]
pub(super) struct Terminals {
    open: Mutex<HashMap<String, Arc<Open>>>,
    /// Terminals still opening (a server's can take a while): they count toward the limit.
    opening: AtomicUsize,
    /// Counts the times every terminal was closed (a reload of the page, or quitting): one
    /// still opening then is closed as soon as it opens, since nobody is left to show it.
    closed_all: AtomicU64,
}

/// A place kept for a terminal while it opens; given back when the opening ends, whichever
/// way it ends.
struct Slot<'a>(&'a Terminals);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.opening.fetch_sub(1, Ordering::SeqCst);
    }
}

fn place_word(place: &TerminalPlace) -> &'static str {
    match place {
        TerminalPlace::ThisPc => "thisPc",
        TerminalPlace::Server { .. } => "server",
        TerminalPlace::AiTool { .. } => "aiTool",
    }
}

/// An AI tool's sign-in tab's tool and action, for its records.
fn ai_tool_fields(place: &TerminalPlace) -> serde_json::Value {
    match place {
        TerminalPlace::AiTool { runtime_id, action } => {
            json!({ "runtimeId": runtime_id, "action": action.as_str() })
        }
        _ => json!({}),
    }
}

/// What the AI tools service does when a sign-in tab's program ends (ADR-058 §4).
pub type AfterEnd = Box<dyn FnOnce(&Ending) + Send>;

impl Broker {
    fn terminals(&self) -> &Terminals {
        &self.inner.terminals
    }

    /// Settings → Terminal: the shell for this PC, the choices, and the terminals open now.
    pub fn terminal_settings(&self) -> Result<TerminalSettings> {
        let shell = self
            .ledger()
            .setting(PREFERENCES)?
            .and_then(|p| serde_json::from_value(p["terminalShell"].clone()).ok())
            .unwrap_or_default();
        let config = self.inner.guard.config()?;
        Ok(TerminalSettings {
            // The shell this system will start: a choice no shell here answers to (the
            // default, on a Mac or Linux) is this system's first one (Phase 23).
            shell: terminal::effective_shell(shell),
            shells: terminal::shell_options(),
            runs_as: terminal::runs_as().to_owned(),
            servers_switched_on: config.switches.servers,
            open: self.open_terminals(),
        })
    }

    /// Choose the shell a new terminal on this PC starts (terminals open now keep theirs). Only
    /// one this system offers and has.
    pub fn set_terminal_shell(&self, shell: TerminalShell) -> Result<TerminalSettings> {
        let installed = terminal::shell_options()
            .iter()
            .any(|o| o.shell == shell && o.installed);
        if !installed {
            return Err(BrokerError::Invalid(format!(
                "{} is not installed on {}",
                terminal::shell_label(shell),
                plenipo_core::WORDS.this_computer
            )));
        }
        self.ledger()
            .merge_setting(PREFERENCES, &json!({ "terminalShell": shell }), OWNER)?;
        self.terminal_settings()
    }

    /// The terminals open now, oldest first.
    pub fn open_terminals(&self) -> Vec<TerminalInfo> {
        let mut open: Vec<TerminalInfo> = lock(&self.terminals().open)
            .values()
            .map(|o| o.info.clone())
            .collect();
        open.sort_by(|a, b| a.opened_at.cmp(&b.opened_at).then(a.id.cmp(&b.id)));
        open
    }

    /// Open a terminal for the owner, sized `cols` × `rows`. Its output and its end go to
    /// `sink`, as they happen.
    pub async fn open_terminal(
        &self,
        place: &TerminalPlace,
        cols: u16,
        rows: u16,
        sink: TerminalSink,
    ) -> Result<TerminalInfo> {
        self.refuse_while_a_worker_has_the_screen()?;
        let _slot = self.reserve_terminal()?;
        let born = self.terminals().closed_all.load(Ordering::SeqCst);
        let size = Size::clamped(cols, rows);
        let id = uuid::Uuid::new_v4().to_string();
        let output: terminal::Output = {
            let sink = Arc::clone(&sink);
            Arc::new(move |bytes: &[u8]| {
                sink(TerminalEvent::Output {
                    data: base64::engine::general_purpose::STANDARD.encode(bytes),
                });
            })
        };
        let info = match place {
            TerminalPlace::ThisPc => self.open_here(id, size, output, sink)?,
            TerminalPlace::Server { server_id } => {
                self.open_on_server(id, server_id, size, output, sink)
                    .await?
            }
            // Only the AI tools service opens these, with the tool's own program.
            TerminalPlace::AiTool { .. } => {
                return Err(BrokerError::Invalid(
                    "An AI tool's sign-in opens from its card on the AI tools page".into(),
                ))
            }
        };
        // While it opened, the page that asked for it went (a reload, or Plenipo quitting), or
        // Remote computers (SSH) was switched off: it closes at once.
        let gone = self.terminals().closed_all.load(Ordering::SeqCst) != born;
        let switched_off = matches!(info.place, TerminalPlace::Server { .. })
            && !self.inner.guard.config()?.switches.servers;
        if gone || switched_off {
            let why = if gone {
                "the window was reloaded"
            } else {
                "you switched Remote computers (SSH) off"
            };
            let _ = self.close_terminal(&info.id, why);
            return Err(BrokerError::Invalid(format!(
                "The terminal closed as it opened: {why}"
            )));
        }
        Ok(info)
    }

    /// Open a terminal that runs an AI tool's own sign-in or sign-out program (ADR-058):
    /// `program` comes from the tool's adapter, from a fixed list, and runs directly with its
    /// own environment. Only Plenipo's AI tools service calls this. `after_end` runs once the
    /// program has ended and its closing is recorded.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_program_terminal(
        &self,
        place: TerminalPlace,
        title: String,
        detail: String,
        program: &terminal::ShellProgram,
        cols: u16,
        rows: u16,
        sink: TerminalSink,
        after_end: AfterEnd,
    ) -> Result<TerminalInfo> {
        self.refuse_while_a_worker_has_the_screen()?;
        if self.inner.config.terminal_refuses_administrator {
            terminal::refuse_elevated().map_err(BrokerError::Invalid)?;
        }
        let _slot = self.reserve_terminal()?;
        let size = Size::clamped(cols, rows);
        let output: terminal::Output = {
            let sink = Arc::clone(&sink);
            Arc::new(move |bytes: &[u8]| {
                sink(TerminalEvent::Output {
                    data: base64::engine::general_purpose::STANDARD.encode(bytes),
                });
            })
        };
        let info = TerminalInfo {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            place,
            detail,
            environment: None,
            opened_at: plenipo_ledger::now_ms(),
        };
        let open = self.keep_terminal(info.clone());
        let recorded = self.ended_hook(&open, sink);
        let ended: terminal::Ended = Box::new(move |ending: Ending| {
            let copy = ending.clone();
            recorded(ending);
            after_end(&copy);
        });
        let home = terminal::home_folder();
        match terminal::start_local(program, home.as_deref(), size, output, ended) {
            Ok(shell) => {
                self.record_opened(&info, ai_tool_fields(&info.place));
                self.hold_shell(&open, Shell::Local(shell));
                Ok(info)
            }
            Err(why) => {
                open.ended.store(true, Ordering::SeqCst);
                lock(&self.terminals().open).remove(&info.id);
                Err(BrokerError::Invalid(why))
            }
        }
    }

    /// Record the terminal as open and keep it; `start` starts its shell. If the shell ended
    /// before it was kept, it is simply let go.
    fn keep_terminal(&self, info: TerminalInfo) -> Arc<Open> {
        let open = Arc::new(Open {
            info,
            started: Instant::now(),
            shell: Mutex::new(None),
            ended: AtomicBool::new(false),
        });
        lock(&self.terminals().open).insert(open.info.id.clone(), Arc::clone(&open));
        open
    }

    /// What happens when a terminal ends: its closing is recorded (where, when, how long; never
    /// what was typed or shown), it is forgotten, and the screen is told. Recorded first, so
    /// that when Plenipo quits and waits for its terminals to close, their closing is written.
    fn ended_hook(&self, open: &Arc<Open>, sink: TerminalSink) -> terminal::Ended {
        let this = self.clone();
        let open = Arc::clone(open);
        Box::new(move |ending: Ending| {
            if open.ended.swap(true, Ordering::SeqCst) {
                return;
            }
            let seconds = open.started.elapsed().as_secs_f64();
            let mut payload = json!({
                "terminalId": open.info.id,
                "place": place_word(&open.info.place),
                "title": open.info.title,
                "serverId": match &open.info.place {
                    TerminalPlace::Server { server_id } => Some(server_id.clone()),
                    TerminalPlace::ThisPc | TerminalPlace::AiTool { .. } => None,
                },
                "environment": open.info.environment,
                "seconds": (seconds * 10.0).round() / 10.0,
                "why": ending.why,
                "exitCode": ending.code,
            });
            if let (Some(p), Some(extra)) = (
                payload.as_object_mut(),
                ai_tool_fields(&open.info.place).as_object(),
            ) {
                p.extend(extra.clone());
            }
            this.event(None, OWNER, "terminal.closed", payload);
            lock(&this.terminals().open).remove(&open.info.id);
            sink(TerminalEvent::Ended {
                why: ending.why,
                code: ending.code,
            });
        })
    }

    fn record_opened(&self, info: &TerminalInfo, extra: serde_json::Value) {
        let mut payload = json!({
            "terminalId": info.id,
            "place": place_word(&info.place),
            "title": info.title,
            "detail": info.detail,
        });
        if let (Some(p), Some(extra)) = (payload.as_object_mut(), extra.as_object()) {
            p.extend(extra.clone());
        }
        self.event(None, OWNER, "terminal.opened", payload);
    }

    fn open_here(
        &self,
        id: String,
        size: Size,
        output: terminal::Output,
        sink: TerminalSink,
    ) -> Result<TerminalInfo> {
        if self.inner.config.terminal_refuses_administrator {
            terminal::refuse_elevated().map_err(BrokerError::Invalid)?;
        }
        let choice = self.terminal_settings()?.shell;
        let program = terminal::shell_program(choice).map_err(BrokerError::Invalid)?;
        let info = TerminalInfo {
            id,
            // "This PC", "This Mac", "This computer" (ADR-155).
            title: plenipo_core::words::sentence_start(plenipo_core::WORDS.this_computer),
            place: TerminalPlace::ThisPc,
            detail: program.label.clone(),
            environment: None,
            opened_at: plenipo_ledger::now_ms(),
        };
        let open = self.keep_terminal(info.clone());
        let ended = self.ended_hook(&open, sink);
        let home = terminal::home_folder();
        match terminal::start_local(&program, home.as_deref(), size, output, ended) {
            Ok(shell) => {
                self.record_opened(&info, json!({ "shell": program.label }));
                self.hold_shell(&open, Shell::Local(shell));
                Ok(info)
            }
            Err(why) => {
                open.ended.store(true, Ordering::SeqCst);
                lock(&self.terminals().open).remove(&info.id);
                Err(BrokerError::Invalid(why))
            }
        }
    }

    /// Keep a started shell with its terminal, unless the terminal already ended.
    fn hold_shell(&self, open: &Arc<Open>, shell: Shell) {
        let still_open = lock(&self.terminals().open).contains_key(&open.info.id);
        if still_open && !open.ended.load(Ordering::SeqCst) {
            *lock(&open.shell) = Some(shell);
        }
    }

    async fn open_on_server(
        &self,
        id: String,
        server_id: &str,
        size: Size,
        output: terminal::Output,
        sink: TerminalSink,
    ) -> Result<TerminalInfo> {
        let config = self.inner.guard.config()?;
        let server = config
            .server(server_id)
            .cloned()
            .ok_or_else(|| BrokerError::Invalid("that server is no longer in the list".into()))?;
        if !config.switches.servers {
            return Err(BrokerError::Invalid(
                "Remote computers (SSH) is off in Settings → Switches, so no terminal opens on \
                 a server. Turn it on to use one"
                    .into(),
            ));
        }
        let Some(pinned) = &server.host_key else {
            return Err(BrokerError::Invalid(format!(
                "{}'s server ID is not checked and pinned yet: check it in Settings → Servers \
                 first",
                server.name
            )));
        };
        let credential = self.credential(&server).map_err(BrokerError::Invalid)?;
        let endpoint = Endpoint {
            host: server.host.clone(),
            port: server.port,
            user: server.user.clone(),
            expected: pinned.fingerprint.clone(),
        };
        let connection = match ssh::connect(&endpoint, credential, &self.inner.config.ssh).await {
            Ok(c) => c,
            Err(ConnectError::Changed { expected, seen }) => {
                self.event(
                    None,
                    super::GUARD,
                    "ssh.host_key_changed",
                    json!({
                        "serverId": server.id,
                        "server": server.name,
                        "address": server.address(),
                        "expected": expected,
                        "seen": seen.fingerprint,
                        "algorithm": seen.algorithm,
                        "by": "terminal",
                    }),
                );
                return Err(BrokerError::Invalid(format!(
                    "This server's ID changed. {} now shows {} ({}), not the {expected} you \
                     pinned. This can mean the server was reinstalled, or that another computer \
                     is pretending to be it, so Plenipo did not sign in and sent nothing. Check \
                     the server ID in Settings → Servers.",
                    server.name, seen.fingerprint, seen.algorithm
                )));
            }
            Err(e) => {
                return Err(BrokerError::Invalid(format!(
                    "Plenipo could not connect to {}: {e}",
                    server.name
                )))
            }
        };
        let channel = match connection
            .open_shell(u32::from(size.cols), u32::from(size.rows))
            .await
        {
            Ok(c) => c,
            Err(why) => {
                connection.close("the terminal could not open").await;
                return Err(BrokerError::Invalid(format!(
                    "Plenipo signed in to {}, but {why}",
                    server.name
                )));
            }
        };
        let info = TerminalInfo {
            id,
            title: server.name.clone(),
            place: TerminalPlace::Server {
                server_id: server.id.clone(),
            },
            detail: server.address(),
            environment: Some(server.environment),
            opened_at: plenipo_ledger::now_ms(),
        };
        let open = self.keep_terminal(info.clone());
        let ended = self.ended_hook(&open, sink);
        self.record_opened(
            &info,
            json!({
                "serverId": server.id,
                "environment": server.environment,
                "address": server.address(),
                "hostKey": connection.identity.fingerprint,
            }),
        );
        let shell = terminal::start_remote(connection, channel, output, ended);
        self.hold_shell(&open, Shell::Remote(shell));
        Ok(info)
    }

    /// Keep a place for one more terminal, counting those open and those still opening.
    fn reserve_terminal(&self) -> Result<Slot<'_>> {
        let terminals = self.terminals();
        let open = lock(&terminals.open);
        if open.len() + terminals.opening.load(Ordering::SeqCst) >= MAX_TERMINALS {
            return Err(BrokerError::Invalid(format!(
                "{MAX_TERMINALS} terminals are open already; close one first"
            )));
        }
        terminals.opening.fetch_add(1, Ordering::SeqCst);
        Ok(Slot(terminals))
    }

    /// While a worker uses the screen, mouse, and keyboard, what reaches the terminal could be
    /// the worker's, so the terminal takes nothing until the owner takes over.
    fn refuse_while_a_worker_has_the_screen(&self) -> Result<()> {
        let status = self.inner.control.status();
        match status
            .sessions
            .iter()
            .find(|s| s.kind == ControlKind::Desktop && s.state == ControlState::Active)
        {
            Some(s) => Err(BrokerError::Invalid(format!(
                "{} is using the screen, mouse, and keyboard, so the terminal takes no typing \
                 now. Take over first",
                s.worker
            ))),
            None => Ok(()),
        }
    }

    fn open_terminal_by_id(&self, id: &str) -> Result<Arc<Open>> {
        lock(&self.terminals().open)
            .get(id)
            .cloned()
            .ok_or_else(|| BrokerError::Invalid("that terminal has closed".into()))
    }

    /// The owner's typing, as it comes. Never recorded, never checked (the owner is in charge).
    pub fn write_terminal(&self, id: &str, bytes: &[u8]) -> Result<()> {
        let open = self.open_terminal_by_id(id)?;
        self.refuse_while_a_worker_has_the_screen()?;
        let shell = lock(&open.shell);
        match shell.as_ref() {
            Some(Shell::Local(s)) => s.write(bytes),
            Some(Shell::Remote(s)) => s.write(bytes),
            None => Err("the terminal is still opening".into()),
        }
        .map_err(BrokerError::Invalid)
    }

    /// The terminal's panel changed size.
    pub fn resize_terminal(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let open = self.open_terminal_by_id(id)?;
        let size = Size::clamped(cols, rows);
        let shell = lock(&open.shell);
        match shell.as_ref() {
            Some(Shell::Local(s)) => s.resize(size),
            Some(Shell::Remote(s)) => s.resize(size),
            None => Ok(()),
        }
        .map_err(BrokerError::Invalid)
    }

    /// Close a terminal (its shell and the programs it started end).
    pub fn close_terminal(&self, id: &str, why: &str) -> Result<()> {
        let open = self.open_terminal_by_id(id)?;
        let shell = lock(&open.shell);
        match shell.as_ref() {
            Some(Shell::Local(s)) => s.close(why),
            Some(Shell::Remote(s)) => s.close(why),
            None => {}
        }
        Ok(())
    }

    /// Close every terminal (Plenipo is quitting, or its window's page loaded again and no
    /// longer shows them).
    pub fn close_all_terminals(&self, why: &str) {
        self.terminals().closed_all.fetch_add(1, Ordering::SeqCst);
        self.close_terminals_where(why, |_| true);
    }

    /// Close every terminal but those `keep` names by ID (Phase 21, ADR-094: the first
    /// organization's window loaded again, while another organization's window shows an AI
    /// tool's sign-in, which the first organization's broker runs).
    pub fn close_terminals_but(&self, why: &str, keep: impl Fn(&str) -> bool) {
        self.terminals().closed_all.fetch_add(1, Ordering::SeqCst);
        let ids: Vec<String> = lock(&self.terminals().open)
            .values()
            .filter(|o| !keep(&o.info.id))
            .map(|o| o.info.id.clone())
            .collect();
        for id in ids {
            let _ = self.close_terminal(&id, why);
        }
    }

    /// Close every terminal on a server (Remote computers (SSH) was switched off).
    pub fn close_server_terminals(&self, why: &str) {
        self.close_terminals_where(why, |place| matches!(place, TerminalPlace::Server { .. }));
    }

    fn close_terminals_where(&self, why: &str, which: impl Fn(&TerminalPlace) -> bool) {
        let ids: Vec<String> = lock(&self.terminals().open)
            .values()
            .filter(|o| which(&o.info.place))
            .map(|o| o.info.id.clone())
            .collect();
        for id in ids {
            let _ = self.close_terminal(&id, why);
        }
    }
}
