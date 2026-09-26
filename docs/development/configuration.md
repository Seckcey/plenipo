# Configuration and Environment Conventions

## Principles

1. **Launching requires nothing.** Plenipo must start with zero environment variables and no
   config file. Every setting has a safe default.
2. **No secrets in files Plenipo reads from the repo or environment.** Provider credentials are
   never stored in `.env`, config files, or SQLite. Plenipo uses the Claude Code and Codex CLIs'
   own existing sign-ins (Phase 3, ADR-007) and never reads their credential files; from Phase 7,
   secrets are referenced through Windows Credential Manager (Plenipo Vault) by handle, not value.
3. **User settings live in the per-user app data directory**, not next to the executable.
4. **Environment variables are for development and automation only**, never for end-user
   configuration.

## Environment variables

All Plenipo-owned variables use the `PLENIPO_` prefix.

| Variable                     | Default | Purpose                                                                                             |
| ---------------------------- | ------- | --------------------------------------------------------------------------------------------------- |
| `PLENIPO_SMOKE_TEST`         | unset   | `1`/`true`: launch smoke-test mode (CI).                                                            |
| `PLENIPO_SMOKE_TIMEOUT_SECS` | `60`    | Smoke watchdog timeout, 1–600 seconds.                                                              |
| `RUST_LOG`                   | —       | Reserved for Rust log filtering once structured logging lands.                                      |
| `TAURI_WEBVIEW_AUTOMATION`   | unset   | **Tests only.** `true` lets WebDriver attach (set by the e2e harness). Never set it for normal use. |
| `PLENIPO_E2E_SCREENSHOTS`    | unset   | E2E harness: directory for screenshots.                                                             |
| `PLENIPO_FAKE_AGENT`         | unset   | **Tests only.** E2E harness: path to `plenipo-fake-agent` (default `target/release/…`).             |

Frontend build-time variables must use the `VITE_` prefix (only those and `TAURI_ENV_*` are
exposed to the UI bundle). Never put a secret in a `VITE_` variable — it is compiled into the
shipped JavaScript.

`.env.example` documents available variables. `.env` and `.env.*` are git-ignored.

Child processes launched by the runtime **do not** inherit these or any other variables except
a small OS baseline and what their launch profile declares (see ADR-005).

### Agent runtime processes (Phase 3)

Claude Code and Codex turns get the same OS baseline plus only these variables, when set in
Plenipo's own environment (ADR-007):

| Runtime     | Passed through                                                                                                                                  | Set by Plenipo          |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| Claude Code | `CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_GIT_BASH_PATH`, proxy and CA variables (`HTTPS_PROXY`, `NO_PROXY`, `SSL_CERT_FILE`, `NODE_EXTRA_CA_CERTS`, …) | `DISABLE_AUTOUPDATER=1` |
| Codex       | `CODEX_HOME`, proxy and CA variables                                                                                                            | —                       |

API keys and cloud-provider switches (`ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`,
`CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CODE_USE_BEDROCK`, `OPENAI_API_KEY`, `CODEX_API_KEY`, …) are
**never** passed, so a worker cannot silently bill an API account.

## Directories (Windows)

Plenipo's bundle identifier is `com.eightwest.plenipo`. Tauri resolves:

| Purpose                              | Location                                                                            |
| ------------------------------------ | ----------------------------------------------------------------------------------- |
| Config                               | `%APPDATA%\com.eightwest.plenipo\`                                                  |
| Local data / cache                   | `%LOCALAPPDATA%\com.eightwest.plenipo\`                                             |
| Logs                                 | `%LOCALAPPDATA%\com.eightwest.plenipo\logs\`                                        |
| Ledger database (Phase 2)            | `%LOCALAPPDATA%\com.eightwest.plenipo\ledger\plenipo.db` (+ `-wal`, `-shm`)         |
| Ledger backups and exports           | `%LOCALAPPDATA%\com.eightwest.plenipo\ledger\backups\`                              |
| Quarantined damaged ledgers          | `%LOCALAPPDATA%\com.eightwest.plenipo\ledger\plenipo.db.corrupt-<timestamp>`        |
| Diagnostic profile working directory | `%LOCALAPPDATA%\com.eightwest.plenipo\runtime\diagnostics-workspace\`               |
| Agent session workspaces (Phase 3+)  | `%LOCALAPPDATA%\com.eightwest.plenipo\runtime\agent-workspaces\<session-id>\`       |
| Phase 1 history after import         | `%LOCALAPPDATA%\com.eightwest.plenipo\runtime\executions.json.imported-<timestamp>` |

The ledger lives in **Local** (not Roaming) app data on purpose: roaming profiles can copy a
SQLite file mid-write (ADR-006). Never edit or copy `plenipo.db` while Plenipo is running — use
**Diagnostics → Create backup**.

Always resolve these through Tauri's path API (`app.path()`), never by hard-coding.

> The identifier was confirmed by the owner in Phase 1. Changing it later moves these
> directories and orphans existing user data — treat it as fixed.

## Naming conventions

- Config keys: `camelCase` in JSON, matching DTO field names.
- Rust crates: `plenipo-<component>`. JS packages: `@plenipo/<name>`.
- Tauri commands: `snake_case` verbs (`get_app_info`); TS wrappers: `camelCase` (`getAppInfo`).
- Core types are vendor-neutral (`RuntimeAdapter`, not `ClaudeWorker`) — see ADR-003.
