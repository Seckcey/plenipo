//! The provider-neutral runtime adapter contract (ADR-007) and helpers shared by adapters.
//!
//! Plan verbs → contract:
//!
//! | Plan                      | Here                                                        |
//! | ------------------------- | ----------------------------------------------------------- |
//! | detectInstallation        | [`RuntimeAdapter::executable_name`] + discovery, `version_args` |
//! | detectAuthentication      | [`RuntimeAdapter::auth_args`] + [`RuntimeAdapter::parse_auth`] |
//! | listRuntimeCapabilities   | [`RuntimeAdapter::capabilities`]                            |
//! | startSession / resumeSession / submitTask | [`RuntimeAdapter::turn_args`] ([`TurnRequest`]) |
//! | streamEvents              | [`TurnParser::line`]                                        |
//! | cancelExecution           | the supervisor's process-tree kill (common to all adapters) |
//! | closeSession              | Plenipo marks the session closed (the service)              |
//! | normalizeResult           | [`TurnParser::finish`]                                      |

use std::path::{Path, PathBuf};

use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    AccountAction, AgentEvent, AuthStatus, Effort, KnownModel, NoticeLevel, PlanReport,
    RuntimeCapabilities, TurnOutcome, TurnResult,
};
use crate::agent::tools::{FileAccess, FileAnswer, ToolServer};
use crate::dto::{ExecutionState, TokenUsage};

/// Longest final answer kept in a result.
pub const MAX_RESULT_TEXT: usize = 64 * 1024;
/// Longest text kept in one stored activity event or error.
pub const MAX_EVENT_TEXT: usize = 4 * 1024;
/// Longest tool summary.
pub const MAX_SUMMARY: usize = 200;
/// Lines of stderr kept to explain failures.
const STDERR_TAIL: usize = 20;

/// Captured output of a short probe command (version, sign-in status).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProbeOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    /// The probe could not be started at all.
    pub spawn_error: Option<String>,
}

impl ProbeOutput {
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out && self.spawn_error.is_none()
    }

    /// stdout followed by stderr.
    pub fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

/// Which provider session a turn runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSession {
    /// Start a new provider session. Adapters that let the caller choose the ID get one.
    New { preassigned: Option<String> },
    /// Continue the provider session with this (confirmed) ID.
    Resume { id: String },
}

impl Default for ProviderSession {
    fn default() -> Self {
        Self::New { preassigned: None }
    }
}

/// Everything an adapter needs to build one turn's launch. The objective itself is not
/// here: it is always written to stdin, never placed in arguments. The default is a new
/// session with the runtime's default model and effort, billing not confirmed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnRequest {
    pub session: ProviderSession,
    /// Validated model name, or `None` for the runtime's default.
    pub model: Option<String>,
    /// One of the runtime's effort levels, or `None` for its default.
    pub effort: Option<Effort>,
    /// The sign-in check confirmed a subscription. When false, a runtime that checks billing
    /// per turn must see a subscription credential in the stream, or stop the turn.
    pub billing_confirmed: bool,
    /// Plenipo's tool server for this step (Phase 7), if the worker has permissions.
    pub tools: Option<ToolServer>,
    /// The conversation's own folder, where the process runs (absolute). AI tools that talk
    /// over ACP (ADR-015) also name it in their messages.
    pub working_dir: PathBuf,
}

/// Why a parser asks Plenipo to stop the process immediately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    pub outcome: TurnOutcome,
    pub reason: String,
}

/// Result of parsing one stdout line.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub events: Vec<AgentEvent>,
    pub stop: Option<Stop>,
    /// Lines to write to the process's stdin, in order (a task that talks, ADR-015).
    pub send: Vec<String>,
    /// The task is over: close the process's stdin so it can exit (ADR-015).
    pub close_input: bool,
    /// Files the AI tool asked Plenipo to read or write for it (ADR-027). Plenipo carries each
    /// out through Guard and gives the answer to [`TurnParser::file_answered`].
    pub files: Vec<FileRequest>,
    /// File changes the AI tool is still writing (Phase 18, ADR-055), for Watch. Handed to the
    /// tool provider, which checks them before anything is shown; never stored.
    pub previews: Vec<crate::agent::preview::WritePreview>,
    /// How much of the plan the AI tool reported used, in its own stream (ADR-060 §3).
    pub plan: Option<PlanReport>,
}

/// One file the AI tool asked Plenipo to read or write (ADR-027), numbered by the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRequest {
    pub id: u64,
    pub access: FileAccess,
}

impl Parsed {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn one(event: AgentEvent) -> Self {
        Self {
            events: vec![event],
            ..Self::default()
        }
    }
}

/// How the turn's process ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessEnd {
    pub state: ExecutionState,
    pub exit_code: Option<i32>,
    /// False when the process could not be started.
    pub started: bool,
    pub detail: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Turns one turn's output stream into normalized events and a result.
pub trait TurnParser: Send {
    /// How the task begins. `None` (the default): the prompt is written to stdin, which is
    /// then closed. `Some(lines)`: an AI tool that talks during the task (ADR-015); these
    /// lines are written first, stdin stays open for [`Parsed::send`], and the parser keeps
    /// the prompt to send when the tool is ready for it.
    fn open(&mut self, _prompt: &str) -> Option<Vec<String>> {
        None
    }
    /// All of stdin for a task that does not talk ([`Self::open`] is `None`): the prompt itself
    /// by default. Antigravity reads it as one JSON message (ADR-082).
    fn input(&mut self, prompt: String) -> String {
        prompt
    }
    /// Lines that ask the AI tool to stop the task before Plenipo ends the process (ADR-015
    /// §7). Empty (the default): the process tree is ended right away.
    fn cancel(&mut self) -> Vec<String> {
        Vec::new()
    }
    /// One stdout line (JSON lines expected).
    fn line(&mut self, text: &str, truncated: bool) -> Parsed;
    /// Plenipo's answer to the file request `id` from [`Parsed::files`] (ADR-027). Answers may
    /// come in any order, and while the task goes on.
    fn file_answered(&mut self, _id: u64, _answer: FileAnswer) -> Parsed {
        Parsed::none()
    }
    /// One stderr line; kept to explain failures, never parsed as events.
    fn stderr(&mut self, text: &str);
    /// The process ended: produce the normalized result.
    fn finish(&mut self, end: &ProcessEnd) -> TurnResult;
    /// How much of the conversation's context the AI tool last reported in use, for an AI tool
    /// that reports it (ACP's `usage_update`, ADR-044). The runtime keeps it for the
    /// conversation's next step. Default: not reported.
    fn context_used(&self) -> Option<u64> {
        None
    }
    /// What the conversation's previous step reported ([`Self::context_used`]), so that a drop
    /// to less than half — the AI tool shortened its memory — shows up as
    /// [`AgentEvent::MemoryShortened`].
    fn set_context_used(&mut self, _previous: Option<u64>) {}
    /// A paid step's bill once it ended (ADR-085), at `price`: the service's own bill, its
    /// token counts priced, not sent, or not priced yet. None: not a paid AI tool.
    fn paid_bill(
        &self,
        _price: &crate::pricing::Price,
        _started: bool,
    ) -> Option<crate::agent::paid::PaidBill> {
        None
    }
}

/// A provider runtime. Implementations hold no per-turn state; they only describe how to
/// find, check, launch, and understand their CLI.
pub trait RuntimeAdapter: Send + Sync + 'static {
    /// Stable ID used in records and the UI, e.g. `claude-code`.
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Provider ID stored with executions, e.g. `anthropic`.
    fn provider(&self) -> &'static str;
    fn provider_label(&self) -> &'static str;
    fn capabilities(&self) -> RuntimeCapabilities;
    /// The CLI version whose models and effort levels [`Self::capabilities`] lists, as last
    /// checked against the real CLI (ADR-014), e.g. `2.1.283`.
    fn checked_version(&self) -> &'static str;
    fn install_hint(&self) -> &'static str;
    fn login_hint(&self) -> &'static str;

    // ---- detectInstallation -------------------------------------------------------------

    /// File name searched on PATH (without extension).
    fn executable_name(&self) -> &'static str;
    /// Well-known install locations checked after PATH (full file paths).
    fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf>;
    /// Map a found file to the executable Plenipo should run, or `None` if unusable.
    /// Receives Windows shims (`.cmd`/`.ps1`) and Unix scripts too. Default: Windows accepts
    /// only `.exe`; Unix accepts the file as is.
    fn resolve(&self, found: &Path) -> Option<PathBuf> {
        default_resolve(found)
    }
    fn version_args(&self) -> Vec<String> {
        vec!["--version".into()]
    }
    fn parse_version(&self, out: &ProbeOutput) -> Option<String> {
        find_version(&out.combined())
    }

    // ---- detectAuthentication -----------------------------------------------------------

    fn auth_args(&self) -> Vec<String>;
    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus;
    /// The sign-in check as a short talk instead of a command (ADR-083, GitHub Copilot): the
    /// requests are written to the tool's standard input, which stays open until each has an
    /// answer or the time runs out; [`Self::parse_auth`] then reads the answers, one per line
    /// of [`ProbeOutput::stdout`] ([`talk_answer`]). `None` (the default): the check is the
    /// command [`Self::auth_args`].
    fn auth_talk(&self) -> Option<Talk> {
        None
    }

    // ---- environment --------------------------------------------------------------------

    /// Variables passed through from Plenipo's own environment when set (names only).
    /// Never API keys or other credentials.
    fn passthrough_env(&self) -> Vec<&'static str>;
    /// Variables Plenipo sets for this runtime's processes.
    fn fixed_env(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    /// Settings files the AI tool reads from its home folder (ADR-082): each file's place in
    /// that folder and its contents. Not empty: the tool gets a home folder of its own, kept by
    /// Plenipo, where these files are written afresh before each run, and every process of the
    /// tool (its checks, tasks, sign-in, and update) has its home folder variable
    /// (`USERPROFILE` on Windows, `HOME` elsewhere) pointed there, so none of the owner's own
    /// settings for the tool apply. Never credentials.
    fn own_home(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }
    /// The tool's own variable for its settings folder (ADR-083: Copilot's `COPILOT_HOME`).
    /// `Some`: the tool gets a folder of its own, kept by Plenipo (with [`Self::own_home`]'s
    /// files in it, if any), named by this variable for every process of the tool; the home
    /// folder itself stays the owner's, so sign-ins kept there (the GitHub CLI's) still work.
    /// `None` (the default): a folder of its own replaces the home folder, as above.
    fn home_variable(&self) -> Option<&'static str> {
        None
    }

    // ---- start/resume session + submit task, stream, normalize --------------------------

    /// Whether Plenipo chooses the provider session ID for new sessions.
    fn preassigns_session_id(&self) -> bool {
        false
    }
    fn turn_args(&self, request: &TurnRequest) -> Vec<String>;
    /// Whether this AI tool runs through Plenipo's bridge ([`crate::agent::Bridge`]) instead of
    /// its own program: the tool's program is still found and its version read, but the sign-in
    /// check and every task run the bridge with [`Self::auth_args`] and [`Self::turn_args`]
    /// (ADR-017, Ollama's cloud models through its service on this PC).
    fn bridged(&self) -> bool {
        false
    }
    /// Plenipo's own helper is the tool's program (a paid AI service, ADR-085): no program is
    /// looked for on this PC, and its version is Plenipo's.
    fn built_in(&self) -> bool {
        false
    }
    /// The bridge's arguments for this tool (`--plenipo-paid openrouter`), when not the
    /// configured bridge's own (Ollama's).
    fn bridge_args(&self) -> Option<Vec<String>> {
        None
    }
    /// A paid AI tool (ADR-085): its key comes from the owner's Vault through the paid gate,
    /// only while paid keys are switched on, and every step is priced and set aside under the
    /// spending caps before it starts.
    fn paid(&self) -> bool {
        false
    }
    /// A paid AI tool's words for its card (ADR-087): until `key_works`, that it has not been
    /// checked with a real key; after, where its prices come from.
    fn paid_note(&self, _key_works: bool) -> Option<String> {
        None
    }
    /// The most output tokens a step on `request`'s model can be billed for, thinking included,
    /// where the answer-length field does not limit the thinking (ADR-087 §3): the step sets that
    /// aside instead of its asked-for answer. None: the field limits the thinking too.
    fn most_output_tokens(&self, _request: &TurnRequest) -> Option<u64> {
        None
    }
    /// The model a step runs when it names none (paid AI tools price it before it starts).
    fn default_model(&self) -> Option<&'static str> {
        None
    }
    /// `model`'s price, from the models the tool reported (paid AI tools). None: not priced.
    fn price_of(
        &self,
        _model: &str,
        _reported: Option<&[crate::agent::dto::KnownModel]>,
    ) -> Option<crate::pricing::Price> {
        None
    }
    /// How much a paid step with `prompt_bytes` of words may send and ask for.
    fn paid_limits(
        &self,
        _request: &TurnRequest,
        prompt_bytes: usize,
    ) -> crate::agent::paid::PaidLimits {
        let bytes = u64::try_from(prompt_bytes).unwrap_or(u64::MAX);
        crate::agent::paid::PaidLimits {
            input_bytes: bytes,
            input_tokens: crate::agent::paid::most_input_tokens(bytes, 2),
            output_tokens: crate::agent::paid::DEFAULT_OUTPUT_TOKENS,
        }
    }
    /// Whether the AI tool can use Plenipo's tools (Phase 7). When false, a worker on it is
    /// conversation only and no permission grant is opened for its steps.
    fn accepts_tools(&self) -> bool {
        true
    }
    /// Variables this turn's process gets in addition (for example tool-call time limits).
    fn turn_env(&self, _request: &TurnRequest) -> Vec<(String, String)> {
        Vec::new()
    }
    fn parser(&self, request: &TurnRequest) -> Box<dyn TurnParser>;
    /// Whether the AI tool always says, in what Plenipo reads, when it shortens its memory of a
    /// conversation ([`AgentEvent::MemoryShortened`], ADR-044 §2.5). Without that, Plenipo cannot
    /// tell whether something it sent earlier is still in the conversation, so it pastes saved
    /// records again rather than naming them. An AI tool that reports the context it has in use
    /// is heard once it does (a drop means a shortened memory), even when this is false.
    fn reports_memory_shortened(&self) -> bool {
        false
    }
    /// Whether the AI tool would leave earlier messages of the conversation out of the next
    /// step, with a prompt of `prompt_bytes` (the Ollama bridge sends a window of the
    /// conversation). Known before the step goes out, it counts as a shortened memory then
    /// (ADR-044 §2.5). By default, never.
    fn leaves_out(&self, _request: &TurnRequest, _prompt_bytes: usize) -> bool {
        false
    }

    // ---- The AI tools page (Phase 19, ADR-058 to ADR-060) --------------------------------

    /// The tool's own command to sign in (Reconnect is the same) or out, run in a terminal tab
    /// where the owner signs in themselves (ADR-058). Never a flag that changes what is billed
    /// (`--console`, `--with-api-key`). `None`: the tool has no such command.
    fn account_command(&self, _action: AccountAction) -> Option<Vec<String>> {
        None
    }
    /// Where Plenipo learns the newest version of the tool (ADR-059 §2).
    fn newest_version(&self) -> NewestVersion {
        NewestVersion::None
    }
    /// The newest version, from the answer to [`NewestVersion::Command`].
    fn parse_newest(&self, _out: &ProbeOutput) -> Option<String> {
        None
    }
    /// The tool's own official update command (ADR-059 §3), run with standard input closed.
    /// `None`: the tool has none (Ollama updates itself from its tray app, with the owner's
    /// click).
    fn update_command(&self) -> Option<Vec<String>> {
        None
    }
    /// Variables the update command gets in addition, so it never stops to ask a question.
    fn update_env(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    /// The tool's own command that puts back an earlier version, when it has one (ADR-059 §6).
    fn put_back_command(&self, _version: &str) -> Option<Vec<String>> {
        None
    }
    /// When the tool cannot update itself because it was installed another way: the official
    /// command for the owner to type, in a sentence (ADR-059 §10).
    fn update_by_hand(&self) -> Option<&'static str> {
        None
    }
    /// A short, task-free check that reads the tool's own list of models, and for some tools
    /// how much of the plan is used (ADR-060 §3, §5). `dir` is Plenipo's empty check folder.
    fn status_check(&self, _dir: &Path) -> StatusCheck {
        StatusCheck::None
    }
    /// The check leaves something behind in the tool's own history (Kimi keeps an empty
    /// conversation), so it runs only when the owner asks and after an update.
    fn status_check_leaves_a_trace(&self) -> bool {
        false
    }
    /// The models the tool reported, from the answer to [`Self::status_check`]. `None`: the
    /// answer was not understood.
    fn parse_models(&self, _out: &ProbeOutput) -> Option<Vec<KnownModel>> {
        None
    }
    /// Whether [`Self::parse_models`] lists every model the tool offers (Ollama lists only the
    /// models downloaded to this PC).
    fn reports_every_model(&self) -> bool {
        true
    }
    /// How much of the plan is used, from the answer to [`Self::status_check`], when the tool
    /// reports it there (Codex's app server).
    fn parse_plan(&self, _out: &ProbeOutput) -> Option<PlanReport> {
        None
    }
    /// Whether the tool officially reports how much of the plan is used — in its task stream or
    /// its check (ADR-060 §3). Without it, the card says the tool does not report it.
    fn reports_plan_left(&self) -> bool {
        false
    }
}

/// Where Plenipo learns the newest version of an AI tool (ADR-059 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewestVersion {
    /// No list matches the program: Update checks and installs in one step (Kimi).
    None,
    /// The tool's own check, with these arguments ([`RuntimeAdapter::parse_newest`]).
    Command(Vec<String>),
    /// The AI company's own published release list, read-only, through Guard's gate for
    /// Plenipo's own requests.
    Published(PublishedList),
}

/// A published release list Plenipo reads only a version number from (ADR-059 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishedList {
    /// The latest version of an npm package, for example `@openai/codex`.
    Npm(&'static str),
    /// The latest release of a GitHub repository, for example `ollama/ollama`.
    GitHub(&'static str),
}

impl PublishedList {
    /// The one address this list is read from.
    pub fn address(self) -> String {
        match self {
            Self::Npm(package) => format!("https://registry.npmjs.org/{package}/latest"),
            Self::GitHub(repo) => format!("https://api.github.com/repos/{repo}/releases/latest"),
        }
    }

    /// The version in the list's answer: npm's `version`, GitHub's `tag_name` without its `v`.
    pub fn parse(self, body: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(body).ok()?;
        let text = match self {
            Self::Npm(_) => value.get("version")?.as_str()?,
            Self::GitHub(_) => value.get("tag_name")?.as_str()?,
        };
        let version = find_version(text)?;
        (version.len() <= 64).then_some(version)
    }
}

/// The `result` of the answer numbered `id` in a [`StatusCheck::Talk`]'s output, when the tool
/// answered it without an error.
pub fn talk_answer(out: &ProbeOutput, id: u64) -> Option<serde_json::Value> {
    out.stdout.lines().find_map(|line| {
        let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        (v.get("id").and_then(serde_json::Value::as_u64) == Some(id))
            .then(|| v.get("result").cloned())
            .flatten()
    })
}

/// A short name the tool gave (a plan's name, a model's label), kept only when it is plain
/// words: letters, digits, spaces, and `. _ - ( ) /`, at most `max` characters.
pub fn plain_name(text: &str, max: usize) -> Option<String> {
    let text = text.trim();
    let plain = !text.is_empty()
        && text.chars().count() <= max
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || " ._-()/:".contains(c));
    plain.then(|| text.to_owned())
}

/// A model name as the tool's model option takes it: exactly the rule Plenipo checks typed
/// names against (ADR-007 §3), so a model the tool reports can always be chosen.
pub fn model_name(text: &str) -> Option<String> {
    crate::agent::service::validate_model(text.trim()).ok()
}

/// A short, task-free check of an AI tool (ADR-060): no conversation with a model, no prompt,
/// so no usage is spent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusCheck {
    /// The tool has no list of its own (Claude Code): its models come with Plenipo's updates.
    None,
    /// Run the tool with these arguments.
    Command(Vec<String>),
    /// Run Plenipo's bridge with these arguments (Ollama, ADR-017).
    Bridge(Vec<String>),
    /// Talk to the tool over standard input and output (JSON-RPC, framed as `framing` says):
    /// write `lines`, then wait for the answers to the requests numbered `answers`.
    Talk {
        args: Vec<String>,
        lines: Vec<String>,
        answers: Vec<u64>,
        framing: Framing,
    },
}

/// How the messages of a talk are marked off from each other.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Framing {
    /// One message per line (ACP, Codex's app server).
    #[default]
    Lines,
    /// Each message after a `Content-Length: N` header and a blank line (Copilot's
    /// `--headless --stdio`, ADR-083).
    Headers,
}

/// A short talk with the tool: its arguments, the messages to write (each one JSON-RPC
/// message, without framing), and the requests whose answers are waited for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Talk {
    pub args: Vec<String>,
    pub lines: Vec<String>,
    pub answers: Vec<u64>,
    pub framing: Framing,
}

/// Proxy and certificate settings every runtime may need on managed networks.
pub const NETWORK_ENV: &[&str] = &[
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "https_proxy",
    "http_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NODE_EXTRA_CA_CERTS",
];

fn default_resolve(found: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        let exe = found
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
        exe.then(|| found.to_path_buf())
    } else {
        Some(found.to_path_buf())
    }
}

/// First `N.N[.N][-suffix]` token in `text`.
pub fn find_version(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',')
        .map(|t| t.trim_start_matches('v'))
        .find(|t| {
            let mut parts = t.split('.');
            let major = parts.next().unwrap_or("");
            let minor = parts.next().unwrap_or("");
            !major.is_empty()
                && major.chars().all(|c| c.is_ascii_digit())
                && minor.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .map(|t| cap(t, 64))
}

/// Cut `text` to at most `max` bytes on a character boundary, marking the cut.
pub fn cap(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max.saturating_sub(3);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// First line of `text`, capped for one-line summaries.
pub fn first_line(text: &str, max: usize) -> String {
    cap(
        text.lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim(),
        max,
    )
}

/// Classify a provider error message. Order matters: usage limits often mention "limit"
/// and "429"; sign-in problems mention "login"/"401".
pub fn classify_error(message: &str) -> TurnOutcome {
    let m = message.to_ascii_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|n| m.contains(n));
    if any(&[
        "usage limit",
        "rate limit",
        "rate_limit",
        "hit your limit",
        "limit reached",
        "quota",
        "too many requests",
        "429",
    ]) {
        TurnOutcome::UsageLimited
    } else if any(&[
        "not logged in",
        "please run /login",
        "run `codex login`",
        "log in again",
        "login required",
        "invalid api key",
        "authentication",
        "unauthorized",
        "401",
        "oauth token",
        "token has expired",
        "token expired",
    ]) {
        TurnOutcome::AuthRequired
    } else if any(&[
        "connection error",
        "connection refused",
        "network",
        "enotfound",
        "econnrefused",
        "econnreset",
        "service unavailable",
        "overloaded",
        "503",
        "502",
        "stream disconnected",
    ]) {
        TurnOutcome::ProviderUnavailable
    } else {
        TurnOutcome::Failed
    }
}

/// State every turn parser keeps; adapters compose it.
#[derive(Debug, Default)]
pub struct TurnState {
    pub runtime_label: &'static str,
    pub provider_session_id: Option<String>,
    pub model: Option<String>,
    pub usage: Option<TokenUsage>,
    /// Last complete assistant message.
    pub last_message: Option<String>,
    /// The provider's own final answer, when it reports one separately.
    pub final_text: Option<String>,
    /// The provider reported successful completion.
    pub completed: bool,
    /// The provider reported a terminal error.
    pub error: Option<String>,
    /// Last non-fatal error (e.g. a retry notice).
    pub last_warning: Option<String>,
    pub provider_duration_ms: Option<u64>,
    pub stop: Option<Stop>,
    pub understood: u32,
    pub malformed: u32,
    pub unknown: u32,
    stderr_tail: Vec<String>,
}

impl TurnState {
    pub fn new(runtime_label: &'static str) -> Self {
        Self {
            runtime_label,
            ..Self::default()
        }
    }

    /// A stdout line that is not JSON. Reported once, counted always.
    pub fn malformed_line(&mut self, truncated: bool) -> Parsed {
        self.malformed += 1;
        if self.malformed > 1 {
            return Parsed::none();
        }
        let why = if truncated {
            "an output line was too long to read"
        } else {
            "output that is not a recognized event"
        };
        Parsed::one(AgentEvent::Notice {
            level: NoticeLevel::Warning,
            text: format!("{} produced {why}; it was ignored.", self.runtime_label),
        })
    }

    pub fn stderr(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if self.stderr_tail.len() == STDERR_TAIL {
            self.stderr_tail.remove(0);
        }
        self.stderr_tail.push(cap(text, 1024));
    }

    fn stderr_text(&self) -> Option<String> {
        (!self.stderr_tail.is_empty()).then(|| self.stderr_tail.join("\n"))
    }

    /// Normalize the end of the turn (shared rules; see ADR-007 §7).
    pub fn finish(&mut self, end: &ProcessEnd) -> TurnResult {
        let label = self.runtime_label;
        let text = self
            .final_text
            .clone()
            .or_else(|| self.last_message.clone())
            .map(|t| cap(&t, MAX_RESULT_TEXT));
        let (outcome, summary, error) = if let Some(stop) = self.stop.clone() {
            (stop.outcome, stop.reason.clone(), Some(stop.reason))
        } else {
            match end.state {
                ExecutionState::Cancelled => {
                    let why = end.detail.clone().unwrap_or_else(|| "Cancelled".into());
                    (TurnOutcome::Cancelled, why, None)
                }
                ExecutionState::TimedOut => (
                    TurnOutcome::TimedOut,
                    "The turn exceeded its time limit and was stopped".into(),
                    end.detail.clone(),
                ),
                ExecutionState::Interrupted => (
                    TurnOutcome::Interrupted,
                    "Plenipo stopped while this turn was running".into(),
                    None,
                ),
                ExecutionState::Failed if !end.started => (
                    TurnOutcome::ProviderUnavailable,
                    format!("{label} could not be started"),
                    end.detail.clone(),
                ),
                _ => self.classify_exit(end),
            }
        };
        TurnResult {
            outcome,
            summary: first_line(&summary, 300),
            text,
            error: error.map(|e| cap(&e, MAX_EVENT_TEXT)),
            provider_session_id: self.provider_session_id.clone(),
            model: self.model.clone(),
            usage: self.usage,
            duration_ms: self.provider_duration_ms.or(end.duration_ms),
            ignored_lines: self.malformed + self.unknown,
            // Filled in by the session service, which knows what it sent.
            prompt: None,
        }
    }

    fn classify_exit(&self, end: &ProcessEnd) -> (TurnOutcome, String, Option<String>) {
        let label = self.runtime_label;
        if self.completed {
            let summary = self
                .final_text
                .as_deref()
                .or(self.last_message.as_deref())
                .map_or_else(|| "Completed".to_owned(), |t| first_line(t, 160));
            return (TurnOutcome::Completed, summary, None);
        }
        if let Some(error) = &self.error {
            let outcome = classify_error(error);
            let summary = match outcome {
                TurnOutcome::UsageLimited => {
                    format!("{label} reported a usage limit; resume this session later")
                }
                TurnOutcome::AuthRequired => format!("{label} needs you to sign in again"),
                TurnOutcome::ProviderUnavailable => format!("{label} could not reach its service"),
                _ => format!("{label} reported an error: {}", first_line(error, 200)),
            };
            return (outcome, summary, Some(error.clone()));
        }
        let stderr = self.stderr_text();
        if self.malformed > 0 && self.understood == 0 {
            return (
                TurnOutcome::MalformedOutput,
                format!("{label} produced output Plenipo could not understand"),
                stderr,
            );
        }
        match end.exit_code {
            Some(0) => (
                TurnOutcome::MalformedOutput,
                format!("{label} ended without reporting a result"),
                stderr,
            ),
            code => {
                let explained = stderr
                    .as_deref()
                    .map(classify_error)
                    .filter(|o| *o != TurnOutcome::Failed);
                let how = code.map_or_else(
                    || end.detail.clone().unwrap_or_else(|| "abnormally".into()),
                    |c| format!("with code {c}"),
                );
                match explained {
                    Some(outcome) => (
                        outcome,
                        format!("{label} exited {how}: {}", last_line(stderr.as_deref())),
                        stderr,
                    ),
                    None => (
                        TurnOutcome::Crashed,
                        format!("{label} exited {how} before reporting a result"),
                        stderr.or_else(|| self.last_warning.clone()),
                    ),
                }
            }
        }
    }
}

fn last_line(text: Option<&str>) -> String {
    text.and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()))
        .map_or_else(String::new, |l| cap(l.trim(), 200))
}

/// A short, single-line description of a tool call's target (path, command, query, …).
pub fn tool_summary(input: &serde_json::Value) -> String {
    const KEYS: &[&str] = &[
        "file_path",
        "path",
        "command",
        "pattern",
        "url",
        "query",
        "from",
        "script",
        "message",
        "description",
        "prompt",
    ];
    // Plenipo's run_command: a program and its arguments.
    let program = input
        .get("program")
        .and_then(serde_json::Value::as_str)
        .map(|p| {
            let args = input
                .get("args")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str);
            std::iter::once(p).chain(args).collect::<Vec<_>>().join(" ")
        });
    let found = program.or_else(|| {
        KEYS.iter()
            .find_map(|k| input.get(*k).and_then(serde_json::Value::as_str))
            .map(str::to_owned)
    });
    found.map_or_else(String::new, |s| {
        first_line(&s.replace(['\r', '\n'], " "), MAX_SUMMARY)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end(state: ExecutionState, code: Option<i32>) -> ProcessEnd {
        ProcessEnd {
            state,
            exit_code: code,
            started: true,
            detail: None,
            duration_ms: Some(5),
        }
    }

    #[test]
    fn versions() {
        assert_eq!(
            find_version("2.1.283 (Claude Code)").as_deref(),
            Some("2.1.283")
        );
        assert_eq!(
            find_version("codex-cli 0.50.0\n").as_deref(),
            Some("0.50.0")
        );
        assert_eq!(find_version("v1.2-beta.1").as_deref(), Some("1.2-beta.1"));
        assert_eq!(find_version("no version here 3"), None);
    }

    #[test]
    fn caps_on_char_boundaries() {
        assert_eq!(cap("hello", 10), "hello");
        let capped = cap(&"é".repeat(100), 11);
        assert!(capped.len() <= 11 && capped.ends_with('…'), "{capped}");
        assert_eq!(first_line("\n\n  first \nsecond", 50), "first");
    }

    #[test]
    fn error_classification() {
        use TurnOutcome::*;
        for (msg, want) in [
            ("Claude AI usage limit reached|1760000000", UsageLimited),
            ("You've hit your limit · resets 3pm", UsageLimited),
            ("stream error: 429 Too Many Requests", UsageLimited),
            ("Invalid API key · Please run /login", AuthRequired),
            ("OAuth token has expired", AuthRequired),
            ("Not logged in", AuthRequired),
            ("API Error: Connection error.", ProviderUnavailable),
            ("503 Service Unavailable", ProviderUnavailable),
            ("Something unexpected", Failed),
        ] {
            assert_eq!(classify_error(msg), want, "{msg}");
        }
    }

    #[test]
    fn finish_rules() {
        let mut s = TurnState::new("Test");
        s.completed = true;
        s.last_message = Some("Answer line\nmore".into());
        let r = s.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(
            (r.outcome, r.summary.as_str()),
            (TurnOutcome::Completed, "Answer line")
        );
        assert_eq!(r.text.as_deref(), Some("Answer line\nmore"));

        let mut s = TurnState::new("Test");
        let r = s.finish(&end(ExecutionState::Cancelled, None));
        assert_eq!(r.outcome, TurnOutcome::Cancelled);

        let mut s = TurnState::new("Test");
        s.stderr("boom: segfault");
        let r = s.finish(&end(ExecutionState::Failed, Some(139)));
        assert_eq!(r.outcome, TurnOutcome::Crashed);
        assert_eq!(r.error.as_deref(), Some("boom: segfault"));

        let mut s = TurnState::new("Test");
        s.stderr("Error: Not logged in");
        let r = s.finish(&end(ExecutionState::Failed, Some(1)));
        assert_eq!(r.outcome, TurnOutcome::AuthRequired);

        let mut s = TurnState::new("Test");
        s.malformed_line(false);
        let r = s.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::MalformedOutput);
        assert_eq!(r.ignored_lines, 1);

        let mut s = TurnState::new("Test");
        let r = s.finish(&ProcessEnd {
            started: false,
            ..end(ExecutionState::Failed, None)
        });
        assert_eq!(r.outcome, TurnOutcome::ProviderUnavailable);

        let mut s = TurnState::new("Test");
        s.error = Some("usage limit reached".into());
        s.completed = false;
        let r = s.finish(&end(ExecutionState::Succeeded, Some(0)));
        assert_eq!(r.outcome, TurnOutcome::UsageLimited);

        let mut s = TurnState::new("Test");
        s.stop = Some(Stop {
            outcome: TurnOutcome::BillingNotAllowed,
            reason: "nope".into(),
        });
        let r = s.finish(&end(ExecutionState::Cancelled, None));
        assert_eq!(r.outcome, TurnOutcome::BillingNotAllowed);
    }

    #[test]
    fn malformed_is_reported_once() {
        let mut s = TurnState::new("Test");
        assert_eq!(s.malformed_line(false).events.len(), 1);
        assert!(s.malformed_line(false).events.is_empty());
        assert_eq!(s.malformed, 2);
    }

    #[test]
    fn tool_summaries() {
        let v = serde_json::json!({ "command": "ls -la\nrm x", "other": 1 });
        assert_eq!(tool_summary(&v), "ls -la rm x");
        assert_eq!(tool_summary(&serde_json::json!({})), "");
        let run = serde_json::json!({ "program": "git", "args": ["--version"] });
        assert_eq!(tool_summary(&run), "git --version");
        let commit = serde_json::json!({ "message": "Fix the form\n\nDetails" });
        assert_eq!(tool_summary(&commit), "Fix the form  Details");
    }
}

/// Phase 19: the published release lists (ADR-059 §2).
#[cfg(test)]
mod published_list_tests {
    use super::*;

    #[test]
    fn a_reported_model_is_kept_only_when_it_can_be_chosen() {
        for name in [
            "grok-5",
            "kimi-code/kimi-for-coding",
            "llama3.2:3b",
            "gpt-6[1m]",
        ] {
            assert_eq!(model_name(name).as_deref(), Some(name), "{name}");
            assert!(
                crate::agent::service::validate_model(name).is_ok(),
                "{name}"
            );
        }
        // Names Plenipo would refuse when chosen are never offered.
        for name in [
            "hf.co/bartowski/Llama-3.2-1B-Instruct-GGUF:Q4_K_M",
            "C:/models/x",
            "-rf",
            "a b",
            "",
        ] {
            assert_eq!(model_name(name), None, "{name}");
        }
    }

    #[test]
    fn each_list_has_one_address_and_gives_only_a_version() {
        let npm = PublishedList::Npm("@openai/codex");
        assert_eq!(
            npm.address(),
            "https://registry.npmjs.org/@openai/codex/latest"
        );
        assert_eq!(
            npm.parse(r#"{"name":"@openai/codex","version":"0.158.0"}"#)
                .as_deref(),
            Some("0.158.0")
        );
        let github = PublishedList::GitHub("ollama/ollama");
        assert_eq!(
            github.address(),
            "https://api.github.com/repos/ollama/ollama/releases/latest"
        );
        assert_eq!(
            github
                .parse(r#"{"tag_name":"v0.34.5","name":"v0.34.5"}"#)
                .as_deref(),
            Some("0.34.5")
        );
        assert_eq!(npm.parse("not json"), None);
        assert_eq!(npm.parse(r#"{"version":"latest"}"#), None);
        assert_eq!(github.parse(r#"{"version":"1.2.3"}"#), None);
    }

    #[test]
    fn names_from_a_tool_are_kept_only_in_plain_words() {
        assert_eq!(plain_name("pro", 32).as_deref(), Some("pro"));
        assert_eq!(
            plain_name("GPT-6 Sol (preview)", 64).as_deref(),
            Some("GPT-6 Sol (preview)")
        );
        assert_eq!(plain_name("<script>", 32), None);
        assert_eq!(plain_name("", 32), None);
        assert_eq!(model_name("kimi-code/k3").as_deref(), Some("kimi-code/k3"));
        assert_eq!(
            model_name("gpt-oss:120b-cloud").as_deref(),
            Some("gpt-oss:120b-cloud")
        );
        assert_eq!(model_name("--help"), None);
        assert_eq!(model_name("a b"), None);
    }
}
