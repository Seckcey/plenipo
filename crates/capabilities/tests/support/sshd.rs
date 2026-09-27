//! A synthetic SSH server on 127.0.0.1 for Phase 11's tests (and, as `plenipo-test-sshd`, for
//! the end-to-end test): a real SSH server (the same library's server side) with a fixed host
//! key, password and key sign-in, and a few scripted commands. It runs nothing on this computer.
//! It records everything it is asked, so tests can show what never reached it.
//!
//! Commands (the line Plenipo sends is `[cd 'DIR' && ]exec 'PROGRAM' 'ARG' …`):
//! - `echo ARGS…`, `whoami`, `pwd`, `uptime`, `hostname`: what you would expect;
//! - `systemctl status|restart NAME`: a service's status, or nothing;
//! - `cat FILE`: a file of the options' `files`, else "No such file";
//! - `count N MS`: prints `line 1` … `line N`, one every MS milliseconds;
//! - `sleep S`: waits S seconds; stops on TERM or KILL (recorded);
//! - `stubborn`: waits a minute and ignores TERM; stops on KILL;
//! - `fail`: prints an error and exits with code 3;
//! - `drop`: prints `going away`, then drops the connection without saying goodbye;
//! - anything else: `PROGRAM: command not found`, exit code 127.
//!
//! Port forwarding (direct-tcpip) to any destination reaches `forward_to` on 127.0.0.1.
//!
//! A terminal and a shell (`pty-req`, `shell`) are refused and counted, since workers must never
//! ask for them, unless `shell` is set: then the owner's terminal (Phase 12, ADR-031) gets a small
//! shell with a prompt (`deploy@synthetic:~$ `) that echoes what is typed and knows `echo`,
//! `whoami`, `hostname`, `pwd`, `size` (the terminal's size), and `exit`. Requests are still
//! counted either way, and each line typed is recorded, so tests can check what reached it.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::keys::ssh_key::LineEnding;
use russh::keys::{HashAlg, PrivateKey, PublicKey};
use russh::server::{self, Auth, ChannelOpenHandle, Msg, Session};
use russh::{Channel, ChannelId, Sig};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::AbortHandle;

/// A private key made from `seed` (the same seed makes the same key).
pub fn key(seed: u8) -> PrivateKey {
    PrivateKey::from(Ed25519Keypair::from_seed(&[seed; 32]))
}

/// A key's SHA-256 fingerprint, as `ssh-keygen -lf` shows it.
pub fn fingerprint(key: &PrivateKey) -> String {
    key.public_key().fingerprint(HashAlg::Sha256).to_string()
}

/// A key as an OpenSSH private key file.
pub fn openssh(key: &PrivateKey) -> String {
    key.to_openssh(LineEnding::LF).unwrap().to_string()
}

#[derive(Clone)]
pub struct Options {
    /// The host key's seed.
    pub seed: u8,
    /// 0: any free port.
    pub port: u16,
    pub user: String,
    pub password: String,
    /// Public keys that may sign in as `user`.
    pub authorized: Vec<PublicKey>,
    /// What `cat` finds.
    pub files: HashMap<String, String>,
    /// Where forwarded connections go (a port on 127.0.0.1).
    pub forward_to: Option<u16>,
    /// Give a terminal and a shell to whoever asks (the owner's terminal); off: refuse them.
    pub shell: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            seed: 1,
            port: 0,
            user: "deploy".into(),
            password: "correct horse battery staple".into(),
            authorized: Vec::new(),
            files: HashMap::new(),
            forward_to: None,
            shell: false,
        }
    }
}

/// Everything the server was asked.
#[derive(Debug, Clone, Default)]
pub struct Seen {
    /// Sign-in attempts: `password:USER:ok` or `key:USER:refused`.
    pub sign_ins: Vec<String>,
    /// Command lines, exactly as received.
    pub execs: Vec<String>,
    /// Signals, as `SIGNAL:PROGRAM`.
    pub signals: Vec<String>,
    /// Requests Plenipo must never make.
    pub pty: usize,
    pub env: usize,
    pub agent: usize,
    pub x11: usize,
    pub shell: usize,
    /// Port forwards asked for: `host:port`.
    pub forwards: Vec<String>,
    /// Terminal sizes asked for (the first request, then each change): `COLSxROWS`.
    pub sizes: Vec<String>,
    /// Lines typed into a shell (with `shell` on).
    pub shell_lines: Vec<String>,
    /// Connections accepted.
    pub connections: usize,
}

pub struct Sshd {
    pub port: u16,
    pub fingerprint: String,
    pub algorithm: String,
    seen: Arc<Mutex<Seen>>,
    sessions: Arc<Mutex<Vec<AbortHandle>>>,
    accept: AbortHandle,
}

impl Drop for Sshd {
    fn drop(&mut self) {
        self.accept.abort();
        for s in self.sessions.lock().unwrap().drain(..) {
            s.abort();
        }
    }
}

impl Sshd {
    pub async fn start(options: Options) -> Self {
        let host = key(options.seed);
        let fingerprint = fingerprint(&host);
        let algorithm = host.algorithm().to_string();
        let config = Arc::new(server::Config {
            keys: vec![host],
            auth_rejection_time: Duration::from_millis(10),
            auth_rejection_time_initial: Some(Duration::ZERO),
            inactivity_timeout: Some(Duration::from_secs(3600)),
            ..server::Config::default()
        });
        // A port just given up may take a moment to be free again.
        let mut tries = 0;
        let listener = loop {
            match TcpListener::bind(("127.0.0.1", options.port)).await {
                Ok(l) => break l,
                Err(e) if tries < 100 => {
                    tries += 1;
                    let _ = e;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => panic!("the synthetic SSH server could not listen: {e}"),
            }
        };
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Seen::default()));
        let sessions: Arc<Mutex<Vec<AbortHandle>>> = Arc::new(Mutex::new(Vec::new()));
        let options = Arc::new(options);
        let accept = {
            let seen = Arc::clone(&seen);
            let sessions = Arc::clone(&sessions);
            tokio::spawn(async move {
                while let Ok((stream, _)) = listener.accept().await {
                    let _ = stream.set_nodelay(true);
                    seen.lock().unwrap().connections += 1;
                    let kill: Arc<Mutex<Option<AbortHandle>>> = Arc::new(Mutex::new(None));
                    let handler = Conn {
                        options: Arc::clone(&options),
                        seen: Arc::clone(&seen),
                        user: None,
                        programs: HashMap::new(),
                        kill: Arc::clone(&kill),
                        terminals: HashMap::new(),
                        shells: HashMap::new(),
                    };
                    let config = Arc::clone(&config);
                    // The session runs over one end of a pipe; a pump this server owns relays the
                    // connection to it. Stopping the pump drops the connection at once, as a
                    // network failure would (the session itself runs in the library's own task).
                    let (ours, theirs) = tokio::io::duplex(256 * 1024);
                    tokio::spawn(async move {
                        if let Ok(running) = server::run_stream(config, theirs, handler).await {
                            let _ = running.await;
                        }
                    });
                    let pump = tokio::spawn(async move {
                        let (mut tcp, mut ours) = (stream, ours);
                        let _ = tokio::io::copy_bidirectional(&mut tcp, &mut ours).await;
                    });
                    *kill.lock().unwrap() = Some(pump.abort_handle());
                    sessions.lock().unwrap().push(pump.abort_handle());
                }
            })
            .abort_handle()
        };
        Self {
            port,
            fingerprint,
            algorithm,
            seen,
            sessions,
            accept,
        }
    }

    pub fn seen(&self) -> Seen {
        self.seen.lock().unwrap().clone()
    }

    /// Stop listening and drop every connection (the port is free again soon after).
    pub fn stop(&self) {
        self.accept.abort();
        self.drop_connections();
    }

    /// Drop every connection at once, without an SSH goodbye (as a network failure would).
    pub fn drop_connections(&self) {
        for s in self.sessions.lock().unwrap().drain(..) {
            s.abort();
        }
    }
}

/// Read the line Plenipo sends: `[cd 'DIR' && ]exec 'W' 'W' …` (POSIX single quotes).
pub fn parse_line(line: &str) -> (Option<String>, Vec<String>) {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut had = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                quoted = !quoted;
                had = true;
            }
            '\\' if !quoted => {
                if let Some(n) = chars.next() {
                    current.push(n);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if had || !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                    had = false;
                }
            }
            c => current.push(c),
        }
    }
    if had || !current.is_empty() {
        words.push(current);
    }
    let mut cwd = None;
    let mut rest: &[String] = &words;
    if rest.first().map(String::as_str) == Some("cd")
        && rest.get(2).map(String::as_str) == Some("&&")
    {
        cwd = Some(rest[1].clone());
        rest = &rest[3..];
    }
    if rest.first().map(String::as_str) == Some("exec") {
        rest = &rest[1..];
    }
    (cwd, rest.to_vec())
}

struct Conn {
    options: Arc<Options>,
    seen: Arc<Mutex<Seen>>,
    user: Option<String>,
    /// Running programs: their channel → (name, the signal sent to them).
    programs: HashMap<ChannelId, (String, watch::Sender<Option<Sig>>)>,
    kill: Arc<Mutex<Option<AbortHandle>>>,
    /// Terminals given (channel → size), and the shells running in them.
    terminals: HashMap<ChannelId, (u32, u32)>,
    shells: HashMap<ChannelId, String>,
}

/// The shell's prompt.
fn prompt(user: &str) -> String {
    format!("{user}@synthetic:~$ ")
}

impl Conn {
    /// One line typed into the shell: its output, and whether the shell ends.
    fn shell_line(&self, channel: ChannelId, line: &str) -> (String, bool) {
        let user = self.user.clone().unwrap_or_default();
        let mut words = line.split_whitespace();
        let program = words.next().unwrap_or("");
        let args: Vec<&str> = words.collect();
        let out = match program {
            "" => String::new(),
            "echo" => format!("{}\r\n", args.join(" ")),
            "whoami" => format!("{user}\r\n"),
            "hostname" => "synthetic\r\n".into(),
            "pwd" => format!("/home/{user}\r\n"),
            "size" => {
                let (cols, rows) = self.terminals.get(&channel).copied().unwrap_or((0, 0));
                format!("{cols}x{rows}\r\n")
            }
            "exit" => return ("logout\r\n".into(), true),
            other => format!("{other}: command not found\r\n"),
        };
        (out, false)
    }
}

impl Conn {
    fn note(&self, f: impl FnOnce(&mut Seen)) {
        f(&mut self.seen.lock().unwrap());
    }
}

async fn say(handle: &server::Handle, channel: ChannelId, text: &str) {
    let _ = handle.data(channel, text.as_bytes().to_vec()).await;
}

async fn say_err(handle: &server::Handle, channel: ChannelId, text: &str) {
    let _ = handle
        .extended_data(channel, 1, text.as_bytes().to_vec())
        .await;
}

async fn end(handle: &server::Handle, channel: ChannelId, code: u32) {
    let _ = handle.exit_status_request(channel, code).await;
    let _ = handle.eof(channel).await;
    let _ = handle.close(channel).await;
}

async fn end_by(handle: &server::Handle, channel: ChannelId, sig: Sig) {
    let _ = handle
        .exit_signal_request(channel, sig, false, String::new(), String::new())
        .await;
    let _ = handle.eof(channel).await;
    let _ = handle.close(channel).await;
}

/// Wait `total`, or until a signal this program heeds. The signal it stopped on, if any.
async fn wait_or_signal(
    signals: &mut watch::Receiver<Option<Sig>>,
    total: Duration,
    heeds: impl Fn(&Sig) -> bool,
) -> Option<Sig> {
    let deadline = tokio::time::Instant::now() + total;
    loop {
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => return None,
            changed = signals.changed() => {
                if changed.is_err() {
                    // The channel closed: the program ends with it.
                    return Some(Sig::HUP);
                }
                let sig = signals.borrow().clone();
                if let Some(sig) = sig.filter(|s| heeds(s)) {
                    return Some(sig);
                }
            }
        }
    }
}

impl server::Handler for Conn {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        let ok = user == self.options.user && password == self.options.password;
        self.note(|s| {
            s.sign_ins.push(format!(
                "password:{user}:{}",
                if ok { "ok" } else { "refused" }
            ))
        });
        if ok {
            self.user = Some(user.to_owned());
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        let ok = user == self.options.user
            && self
                .options
                .authorized
                .iter()
                .any(|k| k.key_data() == key.key_data());
        self.note(|s| {
            s.sign_ins
                .push(format!("key:{user}:{}", if ok { "ok" } else { "refused" }))
        });
        if ok {
            self.user = Some(user.to_owned());
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.note(|s| s.forwards.push(format!("{host}:{port}")));
        let Some(to) = self.options.forward_to else {
            reply
                .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        };
        reply.accept().await;
        tokio::spawn(async move {
            if let Ok(mut socket) = tokio::net::TcpStream::connect(("127.0.0.1", to)).await {
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut stream, &mut socket).await;
            }
        });
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.note(|s| {
            s.pty += 1;
            s.sizes.push(format!("{col_width}x{row_height}"));
        });
        if self.options.shell {
            self.terminals.insert(channel, (col_width, row_height));
            session.channel_success(channel)?;
        } else {
            session.channel_failure(channel)?;
        }
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.note(|s| s.sizes.push(format!("{col_width}x{row_height}")));
        if let Some(size) = self.terminals.get_mut(&channel) {
            *size = (col_width, row_height);
        }
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if !self.shells.contains_key(&channel) {
            return Ok(());
        }
        let user = self.user.clone().unwrap_or_default();
        for &byte in data {
            match byte {
                b'\r' | b'\n' => {
                    let line = self
                        .shells
                        .get_mut(&channel)
                        .map(std::mem::take)
                        .unwrap_or_default();
                    self.note(|s| s.shell_lines.push(line.clone()));
                    let (out, exit) = self.shell_line(channel, &line);
                    session.data(channel, format!("\r\n{out}").into_bytes())?;
                    if exit {
                        self.shells.remove(&channel);
                        session.exit_status_request(channel, 0)?;
                        session.eof(channel)?;
                        session.close(channel)?;
                        return Ok(());
                    }
                    session.data(channel, prompt(&user).into_bytes())?;
                }
                // Backspace (DEL, or ^H): rub out the last character.
                0x7f | 0x08 => {
                    let rubbed = self
                        .shells
                        .get_mut(&channel)
                        .and_then(String::pop)
                        .is_some();
                    if rubbed {
                        session.data(channel, b"\x08 \x08".to_vec())?;
                    }
                }
                // Ctrl+C: drop the line.
                0x03 => {
                    if let Some(line) = self.shells.get_mut(&channel) {
                        line.clear();
                    }
                    session.data(channel, format!("^C\r\n{}", prompt(&user)).into_bytes())?;
                }
                b if b >= 0x20 => {
                    if let Some(line) = self.shells.get_mut(&channel) {
                        line.push(char::from(b));
                    }
                    session.data(channel, vec![b])?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn env_request(
        &mut self,
        channel: ChannelId,
        _name: &str,
        _value: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.note(|s| s.env += 1);
        session.channel_failure(channel)?;
        Ok(())
    }

    async fn agent_request(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.note(|s| s.agent += 1);
        Ok(false)
    }

    async fn x11_request(
        &mut self,
        channel: ChannelId,
        _single_connection: bool,
        _x11_auth_protocol: &str,
        _x11_auth_cookie: &str,
        _x11_screen_number: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.note(|s| s.x11 += 1);
        session.channel_failure(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.note(|s| s.shell += 1);
        if self.options.shell && self.terminals.contains_key(&channel) {
            session.channel_success(channel)?;
            self.shells.insert(channel, String::new());
            let user = self.user.clone().unwrap_or_default();
            session.data(
                channel,
                format!("Welcome to the synthetic server.\r\n{}", prompt(&user)).into_bytes(),
            )?;
        } else {
            session.channel_failure(channel)?;
        }
        Ok(())
    }

    async fn signal(
        &mut self,
        channel: ChannelId,
        signal: Sig,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some((name, tx)) = self.programs.get(&channel) {
            let name = name.clone();
            let label = format!("{signal:?}");
            self.note(|s| s.signals.push(format!("{label}:{name}")));
            tx.send_replace(Some(signal));
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Its program (or shell) ends with it.
        self.programs.remove(&channel);
        self.shells.remove(&channel);
        self.terminals.remove(&channel);
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let line = String::from_utf8_lossy(data).into_owned();
        self.note(|s| s.execs.push(line.clone()));
        session.channel_success(channel)?;
        let (cwd, argv) = parse_line(&line);
        let program = argv.first().cloned().unwrap_or_default();
        let args: Vec<String> = argv.iter().skip(1).cloned().collect();
        let (tx, mut signals) = watch::channel(None);
        self.programs.insert(channel, (program.clone(), tx));
        let handle = session.handle();
        let options = Arc::clone(&self.options);
        let user = self.user.clone().unwrap_or_default();
        let kill = Arc::clone(&self.kill);
        tokio::spawn(async move {
            let h = &handle;
            match program.as_str() {
                "echo" => {
                    say(h, channel, &format!("{}\n", args.join(" "))).await;
                    end(h, channel, 0).await;
                }
                "whoami" => {
                    say(h, channel, &format!("{user}\n")).await;
                    end(h, channel, 0).await;
                }
                "pwd" => {
                    say(
                        h,
                        channel,
                        &format!("{}\n", cwd.unwrap_or(format!("/home/{user}"))),
                    )
                    .await;
                    end(h, channel, 0).await;
                }
                "hostname" => {
                    say(h, channel, "synthetic\n").await;
                    end(h, channel, 0).await;
                }
                "uptime" => {
                    say(
                        h,
                        channel,
                        " 10:00:00 up 3 days,  1 user,  load average: 0.00, 0.01, 0.05\n",
                    )
                    .await;
                    end(h, channel, 0).await;
                }
                "systemctl" => {
                    let unit = args.get(1).cloned().unwrap_or_else(|| "nginx".into());
                    if args.first().map(String::as_str) == Some("status") {
                        say(
                            h,
                            channel,
                            &format!("● {unit}.service - {unit}\n   Active: active (running)\n"),
                        )
                        .await;
                    }
                    end(h, channel, 0).await;
                }
                "cat" => {
                    let file = args.first().cloned().unwrap_or_default();
                    let path = match (&cwd, file.starts_with('/')) {
                        (_, true) => file.clone(),
                        (Some(dir), false) if dir.starts_with('/') => format!("{dir}/{file}"),
                        (Some(dir), false) => format!("/home/{user}/{dir}/{file}"),
                        (None, false) => format!("/home/{user}/{file}"),
                    };
                    match options.files.get(&path) {
                        Some(text) => {
                            say(h, channel, text).await;
                            end(h, channel, 0).await;
                        }
                        None => {
                            say_err(
                                h,
                                channel,
                                &format!("cat: {file}: No such file or directory\n"),
                            )
                            .await;
                            end(h, channel, 1).await;
                        }
                    }
                }
                "count" => {
                    let n: u32 = args.first().and_then(|a| a.parse().ok()).unwrap_or(3);
                    let ms: u64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(100);
                    for i in 1..=n {
                        if let Some(sig) = wait_or_signal(
                            &mut signals,
                            if i == 1 {
                                Duration::ZERO
                            } else {
                                Duration::from_millis(ms)
                            },
                            |_| true,
                        )
                        .await
                        {
                            end_by(h, channel, sig).await;
                            return;
                        }
                        say(h, channel, &format!("line {i}\n")).await;
                    }
                    end(h, channel, 0).await;
                }
                "sleep" => {
                    let s: u64 = args.first().and_then(|a| a.parse().ok()).unwrap_or(1);
                    match wait_or_signal(&mut signals, Duration::from_secs(s), |_| true).await {
                        Some(sig) => end_by(h, channel, sig).await,
                        None => end(h, channel, 0).await,
                    }
                }
                "stubborn" => {
                    let heeds = |s: &Sig| matches!(s, Sig::KILL);
                    match wait_or_signal(&mut signals, Duration::from_secs(60), heeds).await {
                        Some(sig) => end_by(h, channel, sig).await,
                        None => end(h, channel, 0).await,
                    }
                }
                "fail" => {
                    say(h, channel, "starting\n").await;
                    say_err(h, channel, "something went wrong\n").await;
                    end(h, channel, 3).await;
                }
                "drop" => {
                    say(h, channel, "going away\n").await;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if let Some(k) = kill.lock().unwrap().take() {
                        k.abort();
                    }
                }
                other => {
                    say_err(h, channel, &format!("{other}: command not found\n")).await;
                    end(h, channel, 127).await;
                }
            }
        });
        Ok(())
    }
}
