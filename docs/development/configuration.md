# Configuration and Environment Conventions

## Principles

1. **Launching requires nothing.** Plenipo must start with zero environment variables and no
   config file. Every setting has a safe default.
2. **No secrets in files Plenipo reads from the repo or environment.** Provider credentials are
   never stored in `.env`, config files, or SQLite. Plenipo uses the Claude Code, Codex, Grok,
   and Kimi CLIs' own existing sign-ins (Phase 3, ADR-007; Grok, ADR-015; Kimi, ADR-027) and
   never reads their credential files; from Phase 7,
   secrets are referenced through Windows Credential Manager (Plenipo Vault) by handle, not value.
3. **User settings live in the per-user app data directory**, not next to the executable.
4. **Environment variables are for development and automation only**, never for end-user
   configuration.

## Environment variables

All Plenipo-owned variables use the `PLENIPO_` prefix.

| Variable                        | Default | Purpose                                                                                                        |
| ------------------------------- | ------- | -------------------------------------------------------------------------------------------------------------- |
| `PLENIPO_SMOKE_TEST`            | unset   | `1`/`true`: launch smoke-test mode (CI).                                                                       |
| `PLENIPO_SMOKE_TIMEOUT_SECS`    | `60`    | Smoke watchdog timeout, 1–600 seconds.                                                                         |
| `PLENIPO_SMOKE_SCENARIO`        | unset   | **Tests only.** With smoke mode: `stay`, `start-work`, `window-crash`, or `update` (Phase 13 installer tests). |
| `PLENIPO_SMOKE_REPORT`          | unset   | **Tests only.** With smoke mode: file the smoke run writes its JSON report to.                                 |
| `PLENIPO_E2E_UPDATER_KEY`       | unset   | **Tests only.** E2E harness: a throwaway updater key's private half, for the update test.                      |
| `TAURI_WEBVIEW_AUTOMATION`      | unset   | **Tests only.** `true` lets WebDriver attach (set by the e2e harness). Never set it for normal use.            |
| `PLENIPO_E2E_SCREENSHOTS`       | unset   | E2E harness: directory for screenshots.                                                                        |
| `PLENIPO_FAKE_AGENT`            | unset   | **Tests only.** E2E harness: path to `plenipo-fake-agent` (default `target/release/…`).                        |
| `PLENIPO_BROWSER`               | unset   | Full path of the browser to use as Plenipo's browser; wins over the Browser menu in Settings.                  |
| `PLENIPO_BROWSER_START_SECONDS` | `30`    | **Test computers only.** Seconds Plenipo's browser may take to start, 5–300 (CI uses 90, ADR-028).             |

Plenipo's log files are not set by a variable: Plenipo writes what it does (warnings from every
part, and its own steps) to `logs\plenipo.log`, starts a new file at 2 MB, and keeps five. Secrets
and what you type in the terminal are never written to them (Phase 13).

### Build-time variables (Phase 13, ADR-038 updates)

Read when the app is **built**, never at run time. The Release workflow sets them; copies built
without them cannot install updates.

| Variable                     | Set by                                                               | Purpose                                                                                                                                                       |
| ---------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PLENIPO_UPDATER_PUBLIC_KEY` | Release workflow (repository variable), CI's test jobs (a throwaway) | The updater key's public half. Plenipo installs only an update signed with its private half.                                                                  |
| `PLENIPO_UPDATE_ENDPOINT`    | CI's test jobs only                                                  | Where to look for updates. Unset: GitHub Releases. The tests use `http://127.0.0.1:8765/latest.json` (never a release).                                       |
| `PLENIPO_AI_TOOL_RELEASES`   | CI's end-to-end build only                                           | Where the AI tools' release lists are read (Phase 19, ADR-059). Unset: npm and GitHub. The tests use a stand-in at `http://127.0.0.1:8766` (never a release). |

Frontend build-time variables must use the `VITE_` prefix (only those and `TAURI_ENV_*` are
exposed to the UI bundle). Never put a secret in a `VITE_` variable — it is compiled into the
shipped JavaScript.

`.env.example` documents available variables. `.env` and `.env.*` are git-ignored.

Child processes launched by the runtime **do not** inherit these or any other variables except
a small OS baseline and what their launch profile declares (see ADR-005).

### Agent runtime processes (Phase 3)

AI tool turns get the same OS baseline plus only these variables, when set in Plenipo's own
environment (ADR-007), and the ones Plenipo sets itself:

| Runtime     | Passed through                                                                                                                                  | Set by Plenipo          |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| Claude Code | `CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_GIT_BASH_PATH`, proxy and CA variables (`HTTPS_PROXY`, `NO_PROXY`, `SSL_CERT_FILE`, `NODE_EXTRA_CA_CERTS`, …) | `DISABLE_AUTOUPDATER=1` |
| Codex       | `CODEX_HOME`, proxy and CA variables                                                                                                            | —                       |
| Grok        | `GROK_HOME`, proxy and CA variables                                                                                                             | see below               |
| Kimi        | proxy and CA variables                                                                                                                          | —                       |

Grok (ADR-015, running AI tools over ACP) gets `GROK_DISABLE_API_KEY_AUTH=1` (Grok itself refuses
API keys, including a key set on a model in its own settings), `GROK_DISABLE_AUTOUPDATER=1`, and
switches that keep it to the least it can do until Guard grants permissions: `GROK_SUBAGENTS=0`,
`GROK_MEMORY=0`, `GROK_WEB_FETCH=0`, `GROK_BACKEND_SEARCH=0` (xAI's own web and X search, which
run on xAI's side), and `GROK_CLAUDE_*_ENABLED=0` / `GROK_CURSOR_*_ENABLED=0` for skills, hooks,
tool servers, agents, and rules (so it does not load your Claude Code or Cursor settings).

Kimi (ADR-027, Kimi over ACP, with its file reads and writes going through Plenipo) finds its
settings and sign-in in your user folder (`%USERPROFILE%\.kimi-code`, which Plenipo never
reads), so it needs no variable of its own. Its model and thinking level are set in the ACP
session, not in variables.

API keys and cloud-provider switches (`ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`,
`CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_USE_BEDROCK`, `OPENAI_API_KEY`, `CODEX_API_KEY`,
`XAI_API_KEY`, `GROK_CODE_XAI_API_KEY`, `GROK_DEPLOYMENT_KEY`, Grok's auth-provider, OIDC, and
endpoint variables, any Kimi or Moonshot variable, …) are
**never** passed, so a worker cannot silently bill an API account.

## Directories (Windows)

Plenipo's bundle identifier is `com.eightwest.plenipo`. Tauri resolves:

| Purpose                               | Location                                                                                      |
| ------------------------------------- | --------------------------------------------------------------------------------------------- |
| Config                                | `%APPDATA%\com.eightwest.plenipo\`                                                            |
| Local data / cache                    | `%LOCALAPPDATA%\com.eightwest.plenipo\`                                                       |
| Logs (Phase 13: 2 MB each, five kept) | `%LOCALAPPDATA%\com.eightwest.plenipo\logs\plenipo.log` (+ `plenipo.1.log` … `plenipo.4.log`) |
| Diagnostics files you saved           | `%LOCALAPPDATA%\com.eightwest.plenipo\diagnostics\plenipo-diagnostics-<time>.zip` (five kept) |
| The "running" note (Phase 13)         | `%LOCALAPPDATA%\com.eightwest.plenipo\run\plenipo-running.json` (gone after a clean exit)     |
| An update being installed             | `%LOCALAPPDATA%\com.eightwest.plenipo\updates\` (removed at the next start)                   |
| Ledger database (Phase 2)             | `%LOCALAPPDATA%\com.eightwest.plenipo\ledger\plenipo.db` (+ `-wal`, `-shm`)                   |
| Ledger backups and exports            | `%LOCALAPPDATA%\com.eightwest.plenipo\ledger\backups\`                                        |
| A restore waiting for the next start  | `%LOCALAPPDATA%\com.eightwest.plenipo\ledger\restore-request.json`                            |
| Quarantined damaged ledgers           | `%LOCALAPPDATA%\com.eightwest.plenipo\ledger\plenipo.db.corrupt-<timestamp>`                  |
| Diagnostic profile working directory  | `%LOCALAPPDATA%\com.eightwest.plenipo\runtime\diagnostics-workspace\`                         |
| Agent session workspaces (Phase 3+)   | `%LOCALAPPDATA%\com.eightwest.plenipo\runtime\agent-workspaces\<session-id>\`                 |
| Phase 1 history after import          | `%LOCALAPPDATA%\com.eightwest.plenipo\runtime\executions.json.imported-<timestamp>`           |

The ledger lives in **Local** (not Roaming) app data on purpose: roaming profiles can copy a
SQLite file mid-write (ADR-006). Never edit or copy `plenipo.db` while Plenipo is running — use
**Diagnostics → Create backup**.

Backups by kind (Phase 13), in `ledger\backups\`, told apart by their names: `plenipo-backup-*`
(made by you, ten kept), `daily-backup-*` (a week), `pre-upgrade-<version>-*` (before a new
version first used the Ledger, five), `pre-update-<version>-*` (before installing an update,
five), `pre-migration-v<n>-*` (before a Ledger layout change, all kept), and `before-restore-*`
(the Ledger as it was before a restore, three).

Always resolve these through Tauri's path API (`app.path()`), never by hard-coding.

> The identifier was confirmed by the owner in Phase 1. Changing it later moves these
> directories and orphans existing user data — treat it as fixed.

## Naming conventions

- Config keys: `camelCase` in JSON, matching DTO field names.
- Rust crates: `plenipo-<component>`. JS packages: `@plenipo/<name>`.
- Tauri commands: `snake_case` verbs (`get_app_info`); TS wrappers: `camelCase` (`getAppInfo`).
- Core types are vendor-neutral (`RuntimeAdapter`, not `ClaudeWorker`) — see ADR-003.
