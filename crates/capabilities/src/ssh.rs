//! SSH to the owner's servers (Phase 11, ADR-025): connecting to a server only when its identity
//! (host key) is the one the owner pinned, signing in with a key or password from the Vault or
//! with the owner's SSH agent, running one command at a time on a channel with its output as it
//! arrives, stopping it, and forwarding a local port through it.
//!
//! What is never done: forwarding the owner's SSH agent, X11, sending environment variables, or
//! reaching any computer but the one named. A terminal and a shell are asked for only for the
//! owner's own terminal (Phase 12, ADR-031), never for a worker. The server is checked before
//! Plenipo signs in, so nothing about the owner's key or password reaches a server whose
//! identity changed.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use plenipo_guard::CommandLine;
use russh::client::{self, Handle, Handler};
use russh::keys::{self, HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, ChannelReadHalf, ChannelWriteHalf, Disconnect, Sig};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::AbortHandle;

/// How long connecting and signing in may take, and how a quiet connection is kept alive (and
/// found lost).
#[derive(Debug, Clone)]
pub struct Limits {
    /// To open the network connection.
    pub connect: Duration,
    /// For the server to identify itself, and to sign in.
    pub handshake: Duration,
    /// How often a quiet connection is checked.
    pub keepalive: Duration,
    /// Unanswered checks before the connection counts as lost.
    pub keepalive_max: usize,
    /// After a stop: how long the command has to end after TERM, then after KILL.
    pub grace: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(15),
            handshake: Duration::from_secs(30),
            keepalive: Duration::from_secs(15),
            keepalive_max: 3,
            grace: Duration::from_secs(3),
        }
    }
}

/// A server's identity: its host key's type and SHA-256 fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub algorithm: String,
    pub fingerprint: String,
}

/// How Plenipo signs in (the values come from the Vault, and are dropped after use).
pub enum Credential {
    Key {
        key: String,
        passphrase: Option<String>,
    },
    Password(String),
    /// The owner's SSH agent.
    Agent,
}

/// Where to connect, as whom, and the identity to expect.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub user: String,
    /// The pinned fingerprint (`SHA256:…`).
    pub expected: String,
}

/// Why a connection could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// The server could not be reached.
    Unreachable(String),
    /// The server's identity is not the one pinned. Nothing was sent to sign in.
    Changed { expected: String, seen: Identity },
    /// The server did not accept the sign-in.
    SignIn(String),
    /// The key, password, or agent could not be used.
    Credential(String),
    /// Something else went wrong in the SSH conversation.
    Protocol(String),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(why)
            | Self::SignIn(why)
            | Self::Credential(why)
            | Self::Protocol(why) => f.write_str(why),
            Self::Changed { expected, seen } => write!(
                f,
                "its server ID changed: it now shows {} ({}), not the {expected} \
                 pinned when it was set up",
                seen.fingerprint, seen.algorithm
            ),
        }
    }
}

fn identity_of(key: &PublicKeyOrCertificate) -> Identity {
    let data = match key {
        PublicKeyOrCertificate::PublicKey { key, .. } => key.key_data(),
        PublicKeyOrCertificate::Certificate(c) => c.public_key(),
    };
    Identity {
        algorithm: data.algorithm().to_string(),
        fingerprint: data.fingerprint(HashAlg::Sha256).to_string(),
    }
}

/// Plenipo's side of an SSH connection: it accepts only the pinned identity, and asks for
/// nothing else.
struct Client {
    expected: Option<String>,
    seen: Arc<Mutex<Option<Identity>>>,
}

impl Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let seen = identity_of(key);
        let ok = self.expected.as_deref() == Some(seen.fingerprint.as_str());
        *self.seen.lock().unwrap_or_else(|p| p.into_inner()) = Some(seen);
        Ok(ok)
    }
}

fn config(limits: &Limits) -> Arc<client::Config> {
    Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(limits.keepalive),
        keepalive_max: limits.keepalive_max,
        nodelay: true,
        ..client::Config::default()
    })
}

async fn tcp(host: &str, port: u16, limits: &Limits) -> Result<TcpStream, ConnectError> {
    match tokio::time::timeout(limits.connect, TcpStream::connect((host, port))).await {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => Err(ConnectError::Unreachable(format!(
            "Plenipo could not reach {host} on port {port} ({e})"
        ))),
        Err(_) => Err(ConnectError::Unreachable(format!(
            "{host} did not answer on port {port} within {} seconds",
            limits.connect.as_secs()
        ))),
    }
}

/// Open the SSH conversation, expecting `expected` (`None`: only read the identity).
async fn handshake(
    host: &str,
    port: u16,
    expected: Option<String>,
    limits: &Limits,
) -> (Result<Handle<Client>, ConnectError>, Option<Identity>) {
    let stream = match tcp(host, port, limits).await {
        Ok(s) => s,
        Err(e) => return (Err(e), None),
    };
    let seen = Arc::new(Mutex::new(None));
    let handler = Client {
        expected,
        seen: Arc::clone(&seen),
    };
    let outcome = tokio::time::timeout(
        limits.handshake,
        client::connect_stream(config(limits), stream, handler),
    )
    .await;
    let seen = seen.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let result = match outcome {
        Ok(Ok(h)) => Ok(h),
        Ok(Err(e)) => Err(ConnectError::Protocol(format!(
            "the SSH conversation with {host} failed ({e})"
        ))),
        Err(_) => Err(ConnectError::Unreachable(format!(
            "{host} did not identify itself as an SSH server within {} seconds",
            limits.handshake.as_secs()
        ))),
    };
    (result, seen)
}

/// Read a server's identity (for the owner to check and pin), without signing in.
pub async fn identity(host: &str, port: u16, limits: &Limits) -> Result<Identity, ConnectError> {
    let (result, seen) = handshake(host, port, None, limits).await;
    match (seen, result) {
        (Some(seen), _) => Ok(seen),
        (None, Err(e)) => Err(e),
        (None, Ok(_)) => Err(ConnectError::Protocol(
            "the server did not show its host key".into(),
        )),
    }
}

/// A signed-in connection to a server.
pub struct Connection {
    handle: Handle<Client>,
    pub identity: Identity,
}

/// Connect to `endpoint` and sign in. A server that does not show the pinned identity is left
/// before anything is sent to sign in.
pub async fn connect(
    endpoint: &Endpoint,
    credential: Credential,
    limits: &Limits,
) -> Result<Connection, ConnectError> {
    let (result, seen) = handshake(
        &endpoint.host,
        endpoint.port,
        Some(endpoint.expected.clone()),
        limits,
    )
    .await;
    if let Some(seen) = &seen {
        if seen.fingerprint != endpoint.expected {
            return Err(ConnectError::Changed {
                expected: endpoint.expected.clone(),
                seen: seen.clone(),
            });
        }
    }
    let mut handle = result?;
    let Some(identity) = seen else {
        return Err(ConnectError::Protocol(
            "the server did not show its host key".into(),
        ));
    };
    match tokio::time::timeout(
        limits.handshake,
        sign_in(&mut handle, &endpoint.user, credential),
    )
    .await
    {
        Ok(Ok(())) => Ok(Connection { handle, identity }),
        Ok(Err(e)) => {
            let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
            Err(e)
        }
        Err(_) => Err(ConnectError::SignIn(format!(
            "signing in did not finish within {} seconds",
            limits.handshake.as_secs()
        ))),
    }
}

fn is_rsa(algorithm: &keys::Algorithm) -> bool {
    matches!(algorithm, keys::Algorithm::Rsa { .. })
}

/// The hash an RSA key signs with: the best the server says it takes, else SHA-256.
async fn rsa_hash(handle: &Handle<Client>) -> Option<HashAlg> {
    match handle.best_supported_rsa_hash().await {
        Ok(Some(hash)) => hash,
        _ => Some(HashAlg::Sha256),
    }
}

async fn sign_in(
    handle: &mut Handle<Client>,
    user: &str,
    credential: Credential,
) -> Result<(), ConnectError> {
    let refused = |what: &str| {
        ConnectError::SignIn(format!(
            "the server did not accept {what} for the user {user}"
        ))
    };
    let failed = |e: russh::Error| ConnectError::Protocol(format!("signing in failed ({e})"));
    match credential {
        Credential::Key { key, passphrase } => {
            let key = keys::decode_secret_key(&key, passphrase.as_deref())
                .map_err(|e| ConnectError::Credential(key_problem(&e.to_string())))?;
            let hash = if is_rsa(&key.algorithm()) {
                rsa_hash(handle).await
            } else {
                None
            };
            let result = handle
                .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
                .await
                .map_err(failed)?;
            if result.success() {
                Ok(())
            } else {
                Err(refused("the key"))
            }
        }
        Credential::Password(password) => {
            let result = handle
                .authenticate_password(user, password)
                .await
                .map_err(failed)?;
            if result.success() {
                Ok(())
            } else {
                Err(refused("the password"))
            }
        }
        Credential::Agent => agent::sign_in(handle, user).await,
    }
}

/// Plain words for a key that cannot be read.
pub fn key_problem(error: &str) -> String {
    let lower = error.to_lowercase();
    if lower.contains("encrypt") || lower.contains("passphrase") || lower.contains("decrypt") {
        "the private key is protected by a passphrase, and the passphrase stored with it is \
         missing or wrong"
            .into()
    } else {
        format!("the private key could not be read ({error}); paste the whole key file, from its BEGIN line to its END line")
    }
}

/// Check that a key can be read (with its passphrase), before it is stored. Returns its public
/// fingerprint.
pub fn check_key(key: &str, passphrase: Option<&str>) -> Result<String, String> {
    keys::decode_secret_key(key, passphrase)
        .map(|k| k.public_key().fingerprint(HashAlg::Sha256).to_string())
        .map_err(|e| key_problem(&e.to_string()))
}

mod agent {
    use russh::keys::agent::client::{AgentClient, AgentStream};
    use russh::keys::agent::AgentIdentity;

    use super::{is_rsa, rsa_hash, Client, ConnectError};
    use russh::client::Handle;

    /// Sign in with the owner's SSH agent: each key it holds, in turn. The agent only signs;
    /// it is never forwarded to the server.
    pub(super) async fn sign_in(
        handle: &mut Handle<Client>,
        user: &str,
    ) -> Result<(), ConnectError> {
        #[cfg(windows)]
        {
            match AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await {
                Ok(agent) => return try_keys(handle, user, agent).await,
                Err(_) => match AgentClient::connect_pageant().await {
                    Ok(agent) => return try_keys(handle, user, agent).await,
                    Err(_) => Err(ConnectError::Credential(
                        "no SSH agent is running on this computer (neither Windows' OpenSSH \
                             Authentication Agent service nor Pageant)"
                            .into(),
                    )),
                },
            }
        }
        #[cfg(unix)]
        {
            match AgentClient::connect_env().await {
                Ok(agent) => try_keys(handle, user, agent).await,
                Err(_) => Err(ConnectError::Credential(
                    "no SSH agent is running on this computer (SSH_AUTH_SOCK is not set)".into(),
                )),
            }
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = (handle, user);
            Err(ConnectError::Credential(
                "an SSH agent cannot be used on this computer".into(),
            ))
        }
    }

    #[allow(dead_code)]
    async fn try_keys<S: AgentStream + Unpin + Send + 'static>(
        handle: &mut Handle<Client>,
        user: &str,
        mut agent: AgentClient<S>,
    ) -> Result<(), ConnectError> {
        let identities = agent.request_identities().await.map_err(|e| {
            ConnectError::Credential(format!("your SSH agent did not list its keys ({e})"))
        })?;
        let keys: Vec<_> = identities
            .into_iter()
            .filter_map(|i| match i {
                AgentIdentity::PublicKey { key, .. } => Some(key),
                _ => None,
            })
            .collect();
        if keys.is_empty() {
            return Err(ConnectError::Credential(
                "your SSH agent holds no keys: add yours to it first (ssh-add)".into(),
            ));
        }
        for key in keys {
            let hash = if is_rsa(&key.algorithm()) {
                rsa_hash(handle).await
            } else {
                None
            };
            match handle
                .authenticate_publickey_with(user, key, hash, &mut agent)
                .await
            {
                Ok(r) if r.success() => return Ok(()),
                Ok(_) => {}
                Err(e) => {
                    return Err(ConnectError::SignIn(format!(
                        "your SSH agent could not sign in ({e})"
                    )))
                }
            }
        }
        Err(ConnectError::SignIn(format!(
            "the server did not accept any key in your SSH agent for the user {user}"
        )))
    }
}

/// Which stream output came on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
}

impl Stream {
    pub fn word(self) -> &'static str {
        match self {
            Self::Out => "out",
            Self::Err => "err",
        }
    }
}

/// How a command ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    /// It finished with this exit code.
    Exited(u32),
    /// It was ended by a signal on the server.
    Signal(String),
    /// Plenipo stopped it (the owner, or the worker's step ended).
    Stopped(String),
    /// It ran past its time limit and was stopped.
    TimedOut,
    /// The connection was lost while it ran: whether it finished on the server is unknown.
    Lost,
    /// The server refused to run it.
    Refused(String),
    /// The server closed it without saying how it ended.
    Unknown,
}

impl Ending {
    /// The word recorded in the Ledger.
    pub fn word(&self) -> &'static str {
        match self {
            Self::Exited(_) => "exited",
            Self::Signal(_) => "signal",
            Self::Stopped(_) => "stopped",
            Self::TimedOut => "timedOut",
            Self::Lost => "connectionLost",
            Self::Refused(_) => "refused",
            Self::Unknown => "unknown",
        }
    }

    pub fn succeeded(&self) -> bool {
        matches!(self, Self::Exited(0))
    }
}

/// A single-quoted word for a POSIX shell: the server's shell reads it as text, never as more
/// commands.
pub fn quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// The line sent to the server: go to `cwd` (when given), then run the program with its
/// arguments, each quoted.
pub fn remote_line(cwd: Option<&str>, cmd: &CommandLine) -> String {
    let run = std::iter::once(cmd.program.as_str())
        .chain(cmd.args.iter().map(String::as_str))
        .map(quote)
        .collect::<Vec<_>>()
        .join(" ");
    match cwd {
        Some(dir) => format!("cd {} && exec {run}", quote(dir)),
        None => format!("exec {run}"),
    }
}

impl Connection {
    /// The connection has ended (closed, or lost).
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    /// End the connection politely.
    pub async fn close(&self, why: &str) {
        let _ = self
            .handle
            .disconnect(Disconnect::ByApplication, why, "en")
            .await;
    }

    /// Wait up to a second for the connection to be known closed (it may lag its channels).
    async fn closed_soon(&self) -> bool {
        for _ in 0..20 {
            if self.is_closed() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    /// Run `line` on its own channel. Output is handed to `output` as it arrives. When `stop`
    /// turns true, or `time_limit` passes, the program is sent TERM, then KILL, and then its
    /// channel is closed.
    pub async fn run(
        &self,
        line: &str,
        mut output: impl FnMut(Stream, &[u8]) + Send,
        mut stop: watch::Receiver<Option<String>>,
        time_limit: Duration,
        limits: &Limits,
    ) -> (Ending, Duration) {
        let started = Instant::now();
        let mut channel = match self.handle.channel_open_session().await {
            Ok(c) => c,
            Err(e) => {
                let ending = if self.closed_soon().await {
                    Ending::Lost
                } else {
                    Ending::Refused(format!("the server would not open a channel ({e})"))
                };
                return (ending, started.elapsed());
            }
        };
        if channel.exec(true, line).await.is_err() {
            let ending = if self.closed_soon().await {
                Ending::Lost
            } else {
                Ending::Refused("the server did not take the command".into())
            };
            return (ending, started.elapsed());
        }
        let deadline = tokio::time::Instant::now() + time_limit;
        let mut code = None;
        let mut signal = None;
        let mut refused = false;
        // Why and when Plenipo is stopping it, and how far it has gone (TERM, KILL, close).
        let mut stopping: Option<Ending> = None;
        let mut step = 0u8;
        let mut next = tokio::time::Instant::now();
        loop {
            tokio::select! {
                msg = channel.wait() => match msg {
                    Some(ChannelMsg::Data { data }) => output(Stream::Out, &data),
                    Some(ChannelMsg::ExtendedData { data, .. }) => output(Stream::Err, &data),
                    Some(ChannelMsg::ExitStatus { exit_status }) => code = Some(exit_status),
                    Some(ChannelMsg::ExitSignal { signal_name, .. }) => {
                        signal = Some(format!("{signal_name:?}"));
                    }
                    Some(ChannelMsg::Failure) if code.is_none() => refused = true,
                    Some(ChannelMsg::Close) | None => break,
                    Some(_) => {}
                },
                changed = stop.changed(), if stopping.is_none() => {
                    let why = match changed {
                        Ok(()) => stop.borrow().clone(),
                        Err(_) => Some("its worker's step ended".to_owned()),
                    };
                    if let Some(why) = why {
                        stopping = Some(Ending::Stopped(why));
                        next = tokio::time::Instant::now();
                    }
                }
                () = tokio::time::sleep_until(deadline), if stopping.is_none() => {
                    stopping = Some(Ending::TimedOut);
                    next = tokio::time::Instant::now();
                }
                () = tokio::time::sleep_until(next), if stopping.is_some() && step < 3 => {
                    step += 1;
                    match step {
                        1 => { let _ = channel.signal(Sig::TERM).await; }
                        2 => { let _ = channel.signal(Sig::KILL).await; }
                        _ => {
                            let _ = channel.eof().await;
                            let _ = channel.close().await;
                        }
                    }
                    next = tokio::time::Instant::now() + limits.grace;
                }
                () = tokio::time::sleep_until(next), if step >= 3 => break,
            }
        }
        let ending = match (stopping, code, signal) {
            (Some(stopped), _, _) => stopped,
            (None, Some(c), _) => Ending::Exited(c),
            (None, None, Some(s)) => Ending::Signal(s),
            (None, None, None) if refused => {
                Ending::Refused("the server refused to run the command".into())
            }
            (None, None, None) => {
                if self.closed_soon().await {
                    Ending::Lost
                } else {
                    Ending::Unknown
                }
            }
        };
        (ending, started.elapsed())
    }
}

/// The owner's terminal on a server (Phase 12, ADR-031): its channel, split so the owner's
/// typing and the server's output can flow at the same time.
pub struct ShellChannel {
    pub read: ChannelReadHalf,
    pub write: ChannelWriteHalf<client::Msg>,
    /// Output that arrived while Plenipo waited for the server's answers (shown first).
    pub early: Vec<u8>,
}

/// How long the server has to answer a request for a terminal or a shell.
const SHELL_ANSWER: Duration = Duration::from_secs(15);

/// Wait for the server's answer to the request just sent on this channel. Output that comes
/// first is kept in `early`.
async fn answered(
    read: &mut ChannelReadHalf,
    what: &str,
    early: &mut Vec<u8>,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + SHELL_ANSWER;
    loop {
        match tokio::time::timeout_at(deadline, read.wait()).await {
            Ok(Some(ChannelMsg::Success)) => return Ok(()),
            Ok(Some(ChannelMsg::Failure)) => return Err(format!("the server refused {what}")),
            Ok(Some(ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. })) => {
                early.extend_from_slice(&data);
            }
            // Window sizes and the like: not the answer yet.
            Ok(Some(_)) => {}
            Ok(None) => return Err(format!("the server closed the channel instead of {what}")),
            Err(_) => {
                return Err(format!(
                    "the server did not answer the request for {what} within {} seconds",
                    SHELL_ANSWER.as_secs()
                ))
            }
        }
    }
}

impl Connection {
    /// Open the owner's terminal on this connection (ADR-031): a terminal (`pty-req`, sized
    /// `cols` × `rows`) and then a shell, on a channel of their own. Only the owner's terminal
    /// asks for these; a worker's commands never do (they run one program at a time, with
    /// `exec`, through Guard). No agent forwarding, X11, or environment variables are asked for.
    pub async fn open_shell(&self, cols: u32, rows: u32) -> Result<ShellChannel, String> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| format!("the server would not open a channel ({e})"))?;
        let (mut read, write) = channel.split();
        let mut early = Vec::new();
        write
            .request_pty(true, "xterm-256color", cols, rows, 0, 0, &[])
            .await
            .map_err(|e| format!("asking for a terminal failed ({e})"))?;
        answered(&mut read, "a terminal", &mut early).await?;
        write
            .request_shell(true)
            .await
            .map_err(|e| format!("asking for a shell failed ({e})"))?;
        answered(&mut read, "a shell", &mut early).await?;
        Ok(ShellChannel { read, write, early })
    }
}

/// A local port forwarded through a connection, until it is closed.
pub struct Forward {
    pub local: SocketAddr,
    task: AbortHandle,
}

impl Forward {
    pub fn close(&self) {
        self.task.abort();
    }
}

impl Drop for Forward {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Forward a new port on this computer (127.0.0.1 only) to `host:port` as the server sees it.
pub async fn forward(
    connection: Arc<Connection>,
    host: String,
    port: u16,
) -> Result<Forward, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| format!("no local port could be opened ({e})"))?;
    let local = listener
        .local_addr()
        .map_err(|e| format!("no local port could be opened ({e})"))?;
    let task = tokio::spawn(async move {
        while let Ok((mut socket, peer)) = listener.accept().await {
            let connection = Arc::clone(&connection);
            let host = host.clone();
            tokio::spawn(async move {
                let channel = connection
                    .handle
                    .channel_open_direct_tcpip(
                        host,
                        u32::from(port),
                        peer.ip().to_string(),
                        u32::from(peer.port()),
                    )
                    .await;
                if let Ok(channel) = channel {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
                }
            });
        }
    });
    Ok(Forward {
        local,
        task: task.abort_handle(),
    })
}

/// Output split into lines: a line ends at a newline; a carriage return starts it over
/// (progress bars keep only their last state); very long lines are cut.
#[derive(Default)]
pub struct Lines {
    partial: Vec<u8>,
}

/// Longest line kept (characters).
pub const MAX_LINE_CHARS: usize = 4000;

fn tidy(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.rsplit('\r').find(|p| !p.is_empty()).unwrap_or("");
    let text: String = text
        .chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .collect();
    if text.chars().count() > MAX_LINE_CHARS {
        let cut: String = text.chars().take(MAX_LINE_CHARS).collect();
        format!("{cut}…")
    } else {
        text
    }
}

impl Lines {
    /// The whole lines in `bytes` (with what came before).
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.partial.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(i) = self.partial.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.partial.drain(..=i).collect();
            out.push(tidy(&line[..line.len() - 1]));
        }
        // A runaway line with no newline is cut into pieces.
        if self.partial.len() > MAX_LINE_CHARS * 4 {
            let line: Vec<u8> = std::mem::take(&mut self.partial);
            out.push(tidy(&line));
        }
        out
    }

    /// What is left after the output ends.
    pub fn finish(&mut self) -> Option<String> {
        let rest = std::mem::take(&mut self.partial);
        (!rest.is_empty()).then(|| tidy(&rest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_quoted_for_the_servers_shell() {
        assert_eq!(quote("it's"), r"'it'\''s'");
        let cmd = CommandLine::new("echo", &["a b", "$(reboot)", "; rm -rf /", "x'y"]);
        assert_eq!(
            remote_line(Some("/srv/my app"), &cmd),
            r"cd '/srv/my app' && exec 'echo' 'a b' '$(reboot)' '; rm -rf /' 'x'\''y'"
        );
        assert_eq!(
            remote_line(None, &CommandLine::new("uptime", &[])),
            "exec 'uptime'"
        );
    }

    #[test]
    fn output_is_split_into_lines() {
        let mut l = Lines::default();
        assert_eq!(l.push(b"one\ntw"), ["one"]);
        assert_eq!(l.push(b"o\r\nthree\n"), ["two", "three"]);
        assert_eq!(l.push(b"10%\r50%\r100%\n"), ["100%"]);
        assert!(l.push(b"no newline").is_empty());
        assert_eq!(l.finish().as_deref(), Some("no newline"));
        assert_eq!(l.finish(), None);
        let long = vec![b'x'; MAX_LINE_CHARS * 5];
        let pieces = l.push(&long);
        assert_eq!(pieces.len(), 1);
        assert!(pieces[0].ends_with('…'));
        assert_eq!(l.push(b"\x1b[31mred\x1b[0m\n"), ["[31mred[0m"]);
    }

    #[test]
    fn unreadable_keys_are_explained() {
        assert!(check_key("not a key", None)
            .unwrap_err()
            .contains("BEGIN line"));
    }
}
