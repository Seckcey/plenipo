//! Phase 12 terminal tests (ADR-031): the owner's terminal on this PC and on a synthetic SSH
//! server on 127.0.0.1 (`support/sshd.rs`, with its small shell on), through the real broker,
//! Guard, Vault stand-in, and a file-backed Ledger. No internet.
//!
//! The plan's tests covered here: "the owner's terminal on this PC and on a synthetic SSH
//! server, with a changed server ID refused". Also: only opening and closing is recorded, never
//! what the owner types or sees; the server terminal needs the Remote computers (SSH) switch;
//! and Stop all does not close the owner's terminals. (A worker cannot type into the owner's
//! terminal: `ssh.rs`, with workers.)

mod support;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use plenipo_capabilities::{
    Broker, BrokerConfig, MemorySecretStore, TerminalEvent, TerminalInfo, TerminalPlace,
    TerminalShell,
};
use plenipo_guard::{Environment, Guard, HostKeyInput, ServerInput, SignIn, Switches};
use plenipo_ledger::{Ledger, DB_FILE_NAME};
use plenipo_liaison::store::LedgerExecutionStore;
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ProfileRegistry, RuntimeEvent, Supervisor, SupervisorConfig,
};
use serde_json::Value;
use support::sshd::{Options, Sshd};

const WAIT: Duration = Duration::from_secs(60);
const PASSWORD: &str = "terminal-password-5d2e81";

struct NoOutput;

impl EventSink for NoOutput {
    fn emit(&self, _: RuntimeEvent) {}
}

struct H {
    ledger: Arc<Ledger>,
    guard: Guard,
    broker: Broker,
    sshd: Sshd,
    _dir: tempfile::TempDir,
}

/// What one terminal sent to the screen.
#[derive(Default)]
struct Screen {
    bytes: Vec<u8>,
    ended: Option<(String, Option<i32>)>,
}

type Shared = Arc<Mutex<Screen>>;

fn sink(screen: &Shared) -> plenipo_capabilities::broker::TerminalSink {
    let screen = Arc::clone(screen);
    Arc::new(move |event: TerminalEvent| {
        let mut s = screen.lock().unwrap();
        match event {
            TerminalEvent::Output { data } => s.bytes.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap(),
            ),
            TerminalEvent::Ended { why, code } => s.ended = Some((why, code)),
        }
    })
}

fn text(screen: &Shared) -> String {
    String::from_utf8_lossy(&screen.lock().unwrap().bytes).into_owned()
}

async fn until(what: &str, pred: impl Fn() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !pred() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn shows(screen: &Shared, what: &str) {
    let deadline = Instant::now() + WAIT;
    while !text(screen).contains(what) {
        assert!(
            Instant::now() < deadline,
            "the terminal never showed {what:?}; it shows:\n{}",
            text(screen)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn harness() -> H {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let ledger = Arc::new(Ledger::open(&dir.path().join("ledger").join(DB_FILE_NAME)).unwrap());
    let sup = Supervisor::new(
        SupervisorConfig::default(),
        ExecutablePolicy::default(),
        ProfileRegistry::default(),
        Arc::new(LedgerExecutionStore(Arc::clone(&ledger))),
        Arc::new(NoOutput),
        vec![],
    );
    let guard = Guard::new(Arc::clone(&ledger));
    guard
        .set_switches(&Switches {
            servers: true,
            ..Switches::default()
        })
        .unwrap();
    let broker = Broker::new(
        guard.clone(),
        sup,
        Arc::new(MemorySecretStore::with_limit(1_280)),
        BrokerConfig::new(PathBuf::from("unused-relay"), dir.path().join("tickets")),
    );
    let sshd = Sshd::start(Options {
        seed: 7,
        user: "deploy".into(),
        password: PASSWORD.into(),
        shell: true,
        ..Options::default()
    })
    .await;
    broker
        .save_server(&ServerInput {
            name: "Shop".into(),
            host: "127.0.0.1".into(),
            port: sshd.port,
            user: "deploy".into(),
            environment: Environment::Production,
            sign_in: SignIn::Password,
            password: Some(PASSWORD.into()),
            host_key: Some(HostKeyInput {
                algorithm: sshd.algorithm.clone(),
                fingerprint: sshd.fingerprint.clone(),
            }),
            ..ServerInput::default()
        })
        .unwrap();
    H {
        ledger,
        guard,
        broker,
        sshd,
        _dir: dir,
    }
}

impl H {
    fn shop(&self) -> TerminalPlace {
        let id = self
            .guard
            .config()
            .unwrap()
            .servers
            .into_iter()
            .find(|s| s.name == "Shop")
            .unwrap()
            .id;
        TerminalPlace::Server { server_id: id }
    }

    fn events(&self, event_type: &str) -> Vec<Value> {
        self.ledger
            .events_of_types(&[event_type], 500)
            .unwrap()
            .into_iter()
            .map(|e| e.payload)
            .collect()
    }

    /// The whole Ledger, as text.
    fn everything_recorded(&self) -> String {
        self.ledger
            .recent_events(100_000)
            .unwrap()
            .into_iter()
            .map(|e| format!("{} {}\n", e.event_type, e.payload))
            .collect()
    }

    async fn open(&self, place: &TerminalPlace, screen: &Shared) -> TerminalInfo {
        self.broker
            .open_terminal(place, 80, 24, sink(screen))
            .await
            .unwrap()
    }

    async fn closed(&self, info: &TerminalInfo, screen: &Shared) {
        self.broker
            .close_terminal(&info.id, "you closed it")
            .unwrap();
        until("the terminal to end", || {
            screen.lock().unwrap().ended.is_some()
        })
        .await;
        assert!(self.broker.open_terminals().is_empty());
    }
}

/// Plan: the owner's terminal on this PC. It starts the chosen shell (Windows PowerShell by
/// default; off Windows, the user's own shell), takes typing and a new size, and ends when the
/// owner closes it. Only its opening and closing are recorded, never what was typed or shown.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_the_owners_terminal_on_this_pc() {
    let h = harness().await;
    let settings = h.broker.terminal_settings().unwrap();
    assert_eq!(settings.shell, TerminalShell::WindowsPowerShell);
    assert!(settings.servers_switched_on);
    let screen = Shared::default();
    let info = h.open(&TerminalPlace::ThisPc, &screen).await;
    assert_eq!(info.title, "This PC");
    assert!(!info.detail.is_empty());
    assert_eq!(h.broker.open_terminals(), std::slice::from_ref(&info));

    // Something only the shell can work out: its answer, not the typing echoed back.
    let (typed, answer) = if cfg!(windows) {
        ("Write-Output ('pc-' + 6*7)\r", "pc-42")
    } else {
        ("echo pc-$((6*7))\r", "pc-42")
    };
    h.broker.write_terminal(&info.id, typed.as_bytes()).unwrap();
    shows(&screen, answer).await;

    h.broker.resize_terminal(&info.id, 100, 30).unwrap();
    let size = if cfg!(windows) {
        "Write-Output ('size-' + $Host.UI.RawUI.WindowSize.Width + 'x' + $Host.UI.RawUI.WindowSize.Height)\r"
    } else {
        "echo size-$(stty size | tr ' ' x)\r"
    };
    h.broker.write_terminal(&info.id, size.as_bytes()).unwrap();
    shows(
        &screen,
        if cfg!(windows) {
            "size-100x30"
        } else {
            "size-30x100"
        },
    )
    .await;

    h.closed(&info, &screen).await;
    let opened = h.events("terminal.opened");
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0]["place"], "thisPc");
    assert_eq!(opened[0]["terminalId"], info.id.as_str());
    let closed = h.events("terminal.closed");
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0]["why"], "you closed it");
    assert!(closed[0]["seconds"].as_f64().unwrap() >= 0.0);
    // Where, when, and how long: nothing typed, nothing shown.
    let recorded = h.everything_recorded();
    for secret in ["pc-42", "6*7", "stty", "WindowSize", "size-"] {
        assert!(!recorded.contains(secret), "{secret:?} was recorded");
    }
    // Typing into a closed terminal is refused plainly.
    let err = h.broker.write_terminal(&info.id, b"x").unwrap_err();
    assert!(err.to_string().contains("closed"), "{err}");
}

/// Plan: the owner's terminal on a synthetic SSH server. The pinned server ID is checked,
/// Plenipo signs in with the stored password (never shown), asks for a terminal and a shell —
/// for the owner only, with no agent forwarding, X11, or environment — and relays typing, a new
/// size, and the shell's output. Only opening and closing are recorded.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_the_owners_terminal_on_a_server() {
    let h = harness().await;
    let screen = Shared::default();
    let info = h.open(&h.shop(), &screen).await;
    assert_eq!(info.title, "Shop");
    assert_eq!(info.environment, Some(Environment::Production));
    assert!(
        info.detail.starts_with("deploy@127.0.0.1:"),
        "{}",
        info.detail
    );
    shows(&screen, "deploy@synthetic:~$ ").await;

    h.broker
        .write_terminal(&info.id, b"echo typed-7c1f\r")
        .unwrap();
    shows(&screen, "\r\ntyped-7c1f\r\n").await;
    h.broker.resize_terminal(&info.id, 120, 40).unwrap();
    h.broker.write_terminal(&info.id, b"size\r").unwrap();
    shows(&screen, "\r\n120x40\r\n").await;

    h.closed(&info, &screen).await;
    let seen = h.sshd.seen();
    assert_eq!(seen.sign_ins, ["password:deploy:ok"]);
    assert_eq!((seen.pty, seen.shell), (1, 1), "one terminal, one shell");
    assert_eq!(
        (seen.env, seen.agent, seen.x11),
        (0, 0, 0),
        "no environment, agent forwarding, or X11"
    );
    assert_eq!(seen.sizes, ["80x24", "120x40"]);
    assert_eq!(seen.shell_lines, ["echo typed-7c1f", "size"]);
    assert!(seen.execs.is_empty(), "no command was run for a worker");

    let opened = h.events("terminal.opened");
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0]["place"], "server");
    assert_eq!(opened[0]["title"], "Shop");
    assert_eq!(opened[0]["environment"], "production");
    assert_eq!(opened[0]["hostKey"], h.sshd.fingerprint.as_str());
    assert_eq!(h.events("terminal.closed").len(), 1);
    let recorded = h.everything_recorded();
    assert!(
        !recorded.contains("typed-7c1f"),
        "what the owner typed was recorded"
    );
    assert!(
        !recorded.contains("120x40"),
        "what the owner saw was recorded"
    );
    // The sign-in never reached the screen or the Ledger.
    assert!(!text(&screen).contains(PASSWORD));
    assert!(!recorded.contains(PASSWORD));
}

/// Plan: a changed server ID is refused. Plenipo leaves before signing in or sending anything,
/// says "This server's ID changed", and records the change (as it does for workers).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plan_a_changed_server_id_is_refused() {
    let h = harness().await;
    // Another server now answers at Shop's address: its ID is not the one pinned.
    let other = Sshd::start(Options {
        seed: 99,
        user: "deploy".into(),
        password: PASSWORD.into(),
        shell: true,
        ..Options::default()
    })
    .await;
    let mut input = h.server_input("Shop");
    input.port = other.port;
    h.broker.save_server(&input).unwrap();

    let screen = Shared::default();
    let err = h
        .broker
        .open_terminal(&h.shop(), 80, 24, sink(&screen))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("This server's ID changed."), "{err}");
    assert!(err.contains(&other.fingerprint), "{err}");
    assert!(
        err.contains("Plenipo did not sign in and sent nothing"),
        "{err}"
    );
    let seen = other.seen();
    assert!(seen.sign_ins.is_empty(), "nothing was sent to sign in");
    assert_eq!((seen.pty, seen.shell), (0, 0));
    let changed = h.events("ssh.host_key_changed");
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["by"], "terminal");
    assert!(h.events("terminal.opened").is_empty());
    assert!(h.broker.open_terminals().is_empty());
    assert!(text(&screen).is_empty());
}

/// The server terminal needs Settings → Switches → Remote computers (SSH); the terminal on this
/// PC does not. A server whose ID is not pinned yet is refused too.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_server_terminal_needs_the_switch_and_a_pinned_id() {
    let h = harness().await;
    h.guard.set_switches(&Switches::default()).unwrap();
    assert!(!h.broker.terminal_settings().unwrap().servers_switched_on);
    let screen = Shared::default();
    let err = h
        .broker
        .open_terminal(&h.shop(), 80, 24, sink(&screen))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Remote computers (SSH) is off"), "{err}");
    assert_eq!(h.sshd.seen().connections, 0, "the server was never reached");
    // This PC's terminal does not depend on the switch.
    let here = h.open(&TerminalPlace::ThisPc, &screen).await;
    h.closed(&here, &screen).await;

    h.guard
        .set_switches(&Switches {
            servers: true,
            ..Switches::default()
        })
        .unwrap();
    let mut input = h.server_input("Shop");
    input.host_key = None;
    h.broker.save_server(&input).unwrap();
    let err = h
        .broker
        .open_terminal(&h.shop(), 80, 24, sink(&Shared::default()))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("not checked and pinned yet"), "{err}");
    assert_eq!(h.sshd.seen().connections, 0);
}

/// Stop all stops every worker; the owner's terminals stay open and keep working. A shell that
/// ends by itself (`exit`) closes its terminal, and the screen is told why.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_all_does_not_close_the_owners_terminals() {
    let h = harness().await;
    let screen = Shared::default();
    let info = h.open(&h.shop(), &screen).await;
    shows(&screen, "deploy@synthetic:~$ ").await;
    h.broker.stop_all_control("owner").await.unwrap();
    assert_eq!(h.broker.open_terminals().len(), 1);
    h.broker
        .write_terminal(&info.id, b"echo still-here\r")
        .unwrap();
    shows(&screen, "\r\nstill-here\r\n").await;
    h.broker.allow_control("owner").unwrap();

    h.broker.write_terminal(&info.id, b"exit\r").unwrap();
    until("the shell to end", || {
        screen.lock().unwrap().ended.is_some()
    })
    .await;
    let (why, code) = screen.lock().unwrap().ended.clone().unwrap();
    assert_eq!((why.as_str(), code), ("the shell ended", Some(0)));
    assert!(h.broker.open_terminals().is_empty());
    assert_eq!(h.events("terminal.closed")[0]["why"], "the shell ended");
}

/// The shell for this PC is chosen in Settings → Terminal, and kept.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_shell_choice_is_kept() {
    let h = harness().await;
    if cfg!(windows) {
        // Command Prompt is part of Windows.
        let s = h
            .broker
            .set_terminal_shell(TerminalShell::CommandPrompt)
            .unwrap();
        assert_eq!(s.shell, TerminalShell::CommandPrompt);
        assert!(s.other_shell.is_none());
    } else {
        let s = h
            .broker
            .set_terminal_shell(TerminalShell::PowerShell7)
            .unwrap();
        assert_eq!(s.shell, TerminalShell::PowerShell7);
        // Off Windows, the user's own shell is used whatever the choice.
        assert!(s.other_shell.is_some());
    }
    let again = h.broker.terminal_settings().unwrap();
    assert_ne!(again.shell, TerminalShell::WindowsPowerShell);
    assert_eq!(again.shells.len(), 3);
}

impl H {
    fn server_input(&self, name: &str) -> ServerInput {
        let s = self
            .guard
            .config()
            .unwrap()
            .servers
            .into_iter()
            .find(|s| s.name == name)
            .unwrap();
        ServerInput {
            id: Some(s.id),
            name: s.name,
            host: s.host,
            port: s.port,
            user: s.user,
            environment: s.environment,
            sign_in: s.sign_in,
            host_key: s.host_key.map(|k| HostKeyInput {
                algorithm: k.algorithm,
                fingerprint: k.fingerprint,
            }),
            roles: s.roles,
            classes: s.classes,
            approval: s.approval,
            folders: s.folders,
            forwards: s.forwards,
            ..ServerInput::default()
        }
    }
}
