# Phase 16 evidence: Claude's exact versions, Codex's models, Gemini CLI, and Antigravity

What Plenipo's build machine saw before the owner's checks. Gemini CLI then failed on the owner's
PC ([the finding](../../ai-tools-gemini-finding.md)); Antigravity CLI is checked in its place.

# Gemini CLI 0.61.0, read before the owner's check

Recorded on 2026-09-29 (Pacific time) on Plenipo's Linux build machine, **signed out**, with a
cleared environment (only `HOME`, `PATH`, and `TERM`), so no Google sign-in and no key reached
it. Installed with `npm install @google/gemini-cli@0.61.0` into an empty folder. This is not the
owner's step 0; it only shows the program's own help and what it does signed out, so the steps
for the owner's PC use its real options. `<check folder>` replaces the folder's path.

## What it shows

- **The package is a Node.js program** (`bundle/gemini.js`, `"bin": {"gemini": "bundle/gemini.js"}`,
  Node 20 or newer). There is no native `gemini.exe` in it: on Windows, npm installs
  `gemini.cmd` and `gemini.ps1` shims that start Node. Plenipo runs only real `.exe` files
  (the guide, §2), so a Gemini adapter would have to start `node.exe` with the program's path.
  The owner's check records what Windows really gets.
- **The prompt can go in on standard input.** Piped in with no `-p`, it ran without the
  interactive screen and exited by itself (exit code 41 signed out). `-p` "is appended to input
  on stdin", so Plenipo would never need to put the prompt in the arguments.
- **Structured output:** `-o stream-json`. Its first event is `init` with `session_id` and
  `model` only (read from the program: `JsonStreamEventType.INIT`); it does **not** say how the
  program is signed in.
- **No sign-in status command.** `--help` lists `mcp`, `extensions`, `skills`, `hooks`, and
  `gemma`; nothing for sign-in.
- **ACP mode** (`--acp`): `initialize` answers with four sign-in methods — Log in with Google,
  Gemini API key, Vertex AI, and "AI API Gateway" — and `loadSession: true`. Signed out,
  `session/new` answers `-32000 Gemini API key is missing or not configured.`
- **A possible status check, over ACP:** the program's ACP commands include `/about`
  (`packages/cli/src/acp/commands/about.ts` in the bundle). Read from the program, not seen yet
  (it needs a sign-in): sent as a prompt, it answers without asking the model (`input_tokens: 0`) with "Auth Type", "Tier" (the plan), and also "User Email",
  which Plenipo would have to drop. It needs a conversation (`session/new`), which needs a sign-in.
  Whether it tells a Google sign-in from a key, and whether it leaves a conversation in Gemini's
  history, is for the owner's check.
- **Least privilege:** `--approval-mode plan` ("read-only mode"); the others are `default`,
  `auto_edit`, and `yolo`.
- **Resume:** `--resume` takes "latest" or a number; `--session-id` starts a new conversation
  with an ID the caller chooses. Whether `--resume` also takes that ID is for the owner's check.
- **Variables it reads for keys, accounts, and other endpoints** (from the program; none may be
  passed): `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `GOOGLE_APPLICATION_CREDENTIALS`,
  `GOOGLE_CLOUD_ACCESS_TOKEN`, `GOOGLE_CLOUD_PROJECT`, `GOOGLE_CLOUD_PROJECT_ID`,
  `GOOGLE_CLOUD_QUOTA_PROJECT`, `GOOGLE_CLOUD_LOCATION`, `GOOGLE_GENAI_USE_VERTEXAI`,
  `GOOGLE_GENAI_USE_GCA`, `GOOGLE_GEMINI_BASE_URL`, `GOOGLE_VERTEX_BASE_URL`,
  `GEMINI_DEFAULT_AUTH_TYPE`, `GEMINI_API_KEY_AUTH_MECHANISM`, `GEMINI_CLI_CUSTOM_HEADERS`,
  `GEMINI_CLI_IDE_AUTH_TOKEN`, `GEMINI_CLI_USE_COMPUTE_ADC`.

## `gemini --version`

```text
0.61.0
```

## `gemini --help`

```text
Usage: gemini [options] [command]

Gemini CLI - Defaults to interactive mode. Use -p/--prompt for non-interactive (headless) mode.

Commands:
  gemini mcp                   Manage MCP servers
  gemini extensions <command>  Manage Gemini CLI extensions.  [aliases: extension]
  gemini skills <command>      Manage agent skills.  [aliases: skill]
  gemini hooks <command>       Manage Gemini CLI hooks.  [aliases: hook]
  gemini gemma                 Manage local Gemma model routing
  gemini [query..]             Launch Gemini CLI  [default]

Positionals:
  query  Initial prompt. Runs in interactive mode by default; use -p/--prompt for non-interactive.

Options:
  -d, --debug                     Run in debug mode (open debug console with F12)  [boolean] [default: false]
  -m, --model                     Model  [string]
  -p, --prompt                    Run in non-interactive (headless) mode with the given prompt. Appended to input on stdin (if any).  [string]
  -i, --prompt-interactive        Execute the provided prompt and continue in interactive mode  [string]
      --skip-trust                Trust the current workspace for this session.  [boolean] [default: false]
  -w, --worktree                  Start Gemini in a new git worktree. If no name is provided, one is generated automatically.  [string]
  -s, --sandbox                   Run in sandbox?  [boolean]
  -y, --yolo                      Automatically accept all actions (aka YOLO mode, see https://www.youtube.com/watch?v=xvFZjo5PgG0 for more details)?  [boolean] [default: false]
      --approval-mode             Set the approval mode: default (prompt for approval), auto_edit (auto-approve edit tools), yolo (auto-approve all tools), plan (read-only mode)  [string] [choices: "default", "auto_edit", "yolo", "plan"]
      --policy                    Additional policy files or directories to load (comma-separated or multiple --policy)  [array]
      --admin-policy              Additional admin policy files or directories to load (comma-separated or multiple --admin-policy)  [array]
      --acp                       Starts the agent in ACP mode  [boolean]
      --experimental-acp          Starts the agent in ACP mode (deprecated, use --acp instead)  [boolean]
      --allowed-mcp-server-names  Allowed MCP server names  [array]
      --allowed-tools             [DEPRECATED: Use Policy Engine instead See https://geminicli.com/docs/core/policy-engine] Tools that are allowed to run without confirmation  [array]
  -e, --extensions                A list of extensions to use. If not provided, all extensions are used.  [array]
  -l, --list-extensions           List all available extensions and exit.  [boolean]
  -r, --resume                    Resume a previous session. Use "latest" for most recent or index number (e.g. --resume 5)  [string]
      --session-file              Load a session from a JSON file  [string]
      --session-id                Start a new session with a manually provided UUID.  [string]
      --list-sessions             List available sessions for the current project and exit.  [boolean]
      --delete-session            Delete a session by index number (use --list-sessions to see available sessions).  [string]
      --include-directories       Additional directories to include in the workspace (comma-separated or multiple --include-directories)  [array]
      --screen-reader             Enable screen reader mode for accessibility.  [boolean]
  -o, --output-format             The format of the CLI output.  [string] [choices: "text", "json", "stream-json"]
      --raw-output                Disable sanitization of model output (e.g. allow ANSI escape sequences). WARNING: This can be a security risk if the model output is untrusted.  [boolean]
      --accept-raw-output-risk    Suppress the security warning when using --raw-output.  [boolean]
  -v, --version                   Show version number  [boolean]
  -h, --help                      Show help  [boolean]
```

## One task, prompt on standard input, signed out

`echo "Reply with the single word OK" | gemini --output-format stream-json` — nothing on standard
output, exit code 41, and on standard error:

```text
Please set an Auth method in your <check folder>/home/.gemini/settings.json or specify one of the following environment variables before running: GEMINI_API_KEY, GOOGLE_GENAI_USE_VERTEXAI, GOOGLE_GENAI_USE_GCA
```

## ACP, signed out: `initialize`, then `session/new`

`>>` is what was sent, `<<` what Gemini answered.

```text
>> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false}}}
<< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"authMethods":[{"id":"oauth-personal","name":"Log in with Google","description":"Log in with your Google account"},{"id":"gemini-api-key","name":"Gemini API key","description":"Use an API key with Gemini Developer API","_meta":{"api-key":{"provider":"google"}}},{"id":"vertex-ai","name":"Vertex AI","description":"Use an API key with Vertex AI GenAI API"},{"id":"gateway","name":"AI API Gateway","description":"Use a custom AI API Gateway","_meta":{"gateway":{"protocol":"google","restartRequired":"false"}}}],"agentInfo":{"name":"gemini-cli","title":"Gemini CLI","version":"0.61.0"},"agentCapabilities":{"loadSession":true,"promptCapabilities":{"image":true,"audio":true,"embeddedContext":true},"mcpCapabilities":{"http":true,"sse":true}}}}
>> {"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"<check folder>/work","mcpServers":[]}}
<< {"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"Gemini API key is missing or not configured."}}
```

## ACP with a made-up key in the environment: `/about`

To see whether `/about` answers without the model, and what it says when a key (not a Google
sign-in) is in use, the same exchange was run with `GEMINI_API_KEY=not-a-real-key` (a made-up
value; nothing reached Google). `session/new` then opened a conversation (a key is only checked
when the model is asked), and `/about` answered with **0 tokens** and no request to the model. It
said **"Auth Type: " (empty)** and **"Tier: Unknown"**: "Auth Type" is the sign-in method saved in
Gemini's settings, not the key actually in use. Plenipo never passes a key variable (the contract
suite refuses them), so for Plenipo's own runs this case does not arise; what `/about` says after a
real Google sign-in is the owner's check. The script hid the email line itself.

It also left files in Gemini's own history (`~/.gemini/tmp/<folder>/chats/session-….jsonl`, about
1–2 KB each, holding Gemini's set-up text and `/about`), so a check like this one leaves a trace,
as Kimi's model check does (ADR-060 §5).

```text
>> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false}}}
<< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"authMethods":[{"id":"oauth-personal","name":"Log in with Google","description":"Log in with your Google account"},{"id":"gemini-api-key","name":"Gemini API key","description":"Use an API key with Gemini Developer API","_meta":{"api-key":{"provider":"google"}}},{"id":"vertex-ai","name":"Vertex AI","description":"Use an API key with Vertex AI GenAI API"},{"id":"gateway","name":"AI API Gateway","description":"Use a custom AI API Gateway","_meta":{"gateway":{"protocol":"google","restartRequired":"false"}}}],"agentInfo":{"name":"gemini-cli","title":"Gemini CLI","version":"0.61.0"},"agentCapabilities":{"loadSession":true,"promptCapabilities":{"image":true,"audio":true,"embeddedContext":true},"mcpCapabilities":{"http":true,"sse":true}}}}
>> {"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"<check folder>","mcpServers":[]}}
<< {"jsonrpc":"2.0","id":2,"result":{"sessionId":"b520f4c1-fc22-49ed-a563-f1697fe8edb9","modes":{"availableModes":[{"id":"default","name":"Default","description":"Prompts for approval"},{"id":"autoEdit","name":"Auto Edit","description":"Auto-approves edit tools"},{"id":"yolo","name":"YOLO","description":"Auto-approves all tools"},{"id":"plan","name":"Plan","description":"Read-only mode"}],"currentModeId":"default"},"models":{"availableModels":[{"modelId":"auto","name":"Auto","description":"Let Gemini CLI decide the best model for the task: gemini-3.1-pro-preview, gemini-3.8-flash"},{"modelId":"gemini-3.1-pro-preview","name":"gemini-3.1-pro-preview"},{"modelId":"gemini-3-flash-preview","name":"gemini-3-flash-preview"},{"modelId":"gemini-2.5-pro","name":"gemini-2.5-pro"},{"modelId":"gemini-3.8-flash","name":"gemini-3.8-flash"},{"modelId":"gemini-3.5-flash-lite","name":"gemini-3.5-flash-lite"}],"currentModelId":"auto"}}}
>> {"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"b520f4c1-fc22-49ed-a563-f1697fe8edb9","prompt":[{"type":"text","text":"/about"}]}}
<< {"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"b520f4c1-fc22-49ed-a563-f1697fe8edb9","update":{"sessionUpdate":"available_commands_update","availableCommands":[{"name":"memory","description":"Manage memory."},{"name":"memory show","description":"Shows the current memory contents."},{"name":"memory refresh","description":"Refreshes the memory from the source."},{"name":"memory list","description" … (the rest of Gemini's command list)
<< {"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"b520f4c1-fc22-49ed-a563-f1697fe8edb9","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Gemini CLI Info:\n- Version: 0.61.0\n- OS: linux\n- Sandbox: no sandbox\n- Model: auto\n- Auth Type: \n- GCP Project: \n- IDE Client: \n- User Email: <hidden>\n- Tier: Unknown"}}}}
<< {"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn","_meta":{"quota":{"token_count":{"input_tokens":0,"output_tokens":0},"model_usage":[]}}}}
-- done
```

## A one-off task needs the folder trusted, and read-only mode needs it too

Run in a folder Gemini has not been told to trust, a task with the prompt on standard input and
`--approval-mode plan` printed `Approval mode overridden to "default" because the current folder is
not trusted.` and stopped with exit code 55: "Gemini CLI is not running in a trusted directory. To
proceed, either use `--skip-trust`, set the `GEMINI_CLI_TRUST_WORKSPACE=true` environment variable,
or trust this directory in interactive mode." With `--skip-trust` added (trust this folder for this
run only), the same task started: `init` (`session_id`, `"model":"auto"`), then the user's message,
then, with the made-up key, Google's `400 API key not valid` — so the prompt did reach the model
from standard input. Gemini also wrote a full error report to a file in the system's temporary
folder (`gemini-client-error-….json`). A Gemini adapter would pass `--skip-trust` only for Plenipo's
own empty task folder, and would have to keep Gemini's error reports in mind (they can hold the
prompt).

# Google's Antigravity CLI 1.2.13, read before the owner's check

Gemini CLI no longer signs in with a personal Google plan (see
[the finding](../../ai-tools-gemini-finding.md)); Google's replacement is Antigravity CLI. At the
owner's direction (2026-09-29) it is checked in Gemini CLI's place. Recorded on 2026-09-29 (Pacific
time) on Plenipo's Linux build machine, **signed out**, with a cleared environment and an empty home
folder. The program came from Google's own release list (the one its installer reads,
`manifests/linux_amd64.json`), and its SHA-512 matched the list's. This is not the owner's step 0.

## What it shows

- **A real program on Windows.** Google's Windows list points to `cli_windows_x64.exe` (version
  1.2.13); the installer (`irm https://antigravity.google/cli/install.ps1 | iex`) puts it in
  `%LOCALAPPDATA%\agy\bin`, as the command `agy`. It is built in Go, with no Node.js.
- **The task can go in on standard input only.** `-p` always wants a value, but `-p=` (empty) with
  `--input-format stream-json` reads the task from standard input, one JSON line per message:
  `{"event":"user","message":{"role":"user","content":"…"}}`. Plain text on standard input is
  refused ("empty prompt"). So the words never need to be on the command line.
- **Structured output:** `--output-format stream-json`, one JSON object per line: `init` (the
  conversation ID, the folder, the tools, and the permission mode), `step_update` for each step,
  and a final `result` with the conversation ID, `status`, `response` or `error`, and token
  counts. Errors also print `AGY_ERROR: {…}` with a status and an error code.
- **Resume by ID:** `--conversation <ID>` continued the same conversation (the second task
  reported the first one's ID).
- **Least privilege:** `--mode plan` and `--sandbox` ("terminal restrictions enabled") are
  accepted; settings `toolPermission` (`strict`, `request-review`, …) and
  `allowNonWorkspaceAccess` (off by default) exist. What `plan` allows is for the owner's check
  and the decision record.
- **Models and effort:** `agy models` lists models; `--model` chooses one; `--effort` takes
  `low`, `medium`, `high`, `max`.
- **A possible sign-in check: `agy models`,** as `grok models` is Grok's. Signed out it stops with
  exit code 1: "Please sign in to view available models." Whether it lists models, and which, when
  signed in with a Google plan is for the owner's check.
- **A key alone does nothing.** With only `GEMINI_API_KEY` set, `agy models` still said "Please
  sign in". Key mode needs **both** `"modelProvider": "gemini"` in Antigravity's settings file and
  the key in the environment (Google's install page says the same). With both, and a made-up key,
  it listed Gemini API models, and a task reached Google and was refused: `400 API key not
valid`. Plenipo never passes key variables (the contract suite refuses them), so its tasks
  could not use key mode, even with that setting.
- **Paid credits after the plan runs out:** the settings include `useG1Credits` (`on`/`off`):
  "Enables personal AI credit consumption when quota exhausted". Plenipo must never let a task
  spend paid credits (ADR-036 §2: nothing paid before spending caps). How it is set on the owner's
  account, and whether Plenipo can hold it off for its tasks, is for the owner's check.
- **It updates itself in the background** during normal runs (its installer says so). No
  documented switch turns that off. Plenipo updates AI tools only between tasks (ADR-059, Plenipo
  keeps the AI tools up to date), so this is for the owner's check and the decision record.
- **Other names it reads** (from the program; none would be passed): `GEMINI_API_KEY`,
  `GOOGLE_GEMINI_BASE_URL` (Google's install page), `AGY_ADC_AUTH`, and `AGY_BUSINESS_PAYGO_TIER`.

## `agy --version`

```text
1.2.13
```

## `agy --help`

```text
Usage of antigravity:
  --add-dir                       Add a directory to the workspace (repeatable) (default [])
  --agent                         Agent for the current CLI session
  -c                              Short alias for --continue
  --continue                      Continue the most recent conversation
  --conversation                  Resume a previous conversation by ID
  --dangerously-skip-permissions  Auto-approve all tool permission requests without prompting
  --disable-slash-commands        Disable slash command and skill expansion in print mode
  --effort                        Reasoning effort for the current CLI session (low|medium|high|max)
  -i                              Short alias for --prompt-interactive
  --input-format                  Input format for print mode (text, stream-json). stream-json reads one NDJSON message per line from stdin and runs a turn for each; it requires --output-format stream-json (default text)
  --json-schema                   Optional JSON schema string or path to a schema file to enforce structured output (for stream-json, only applicable to the final result)
  --log-file                      Override CLI log file path
  --mode                          Set the agent execution mode for this session (accept-edits, plan)
  --model                         Model for the current CLI session
  --new-project                   Create a new project for this session
  --output-format                 Output format for print mode (text, json, stream-json) (default text)
  -p                              Short alias for --print
  --print                         Run a single prompt non-interactively and print the response
  --print-timeout                 Optional time limit for print mode; 0 waits until the turn completes (default 0s)
  --project                       Project ID or project name for the current CLI session
  --prompt                        Alias for --print
  --prompt-interactive            Run an initial prompt interactively and continue the session
  --remote-control                Create a remote connection for the CLI session on start up
  --sandbox                       Run in a sandbox with terminal restrictions enabled

Available subcommands:
  agent           List available agents
  agents          List available agents
  changelog       Show changelog and release notes
  help            Show help for subcommands
  install         Configure environment paths and shell settings
  mcp             Manage MCP servers (add, remove, list, enable, disable)
  mic-serve       Serve this machine's microphone to a CLI on another host
  models          List available models
  plugin          Manage plugins (install, uninstall, list, enable, disable)
  plugins         Alias for plugin
  remote-control  Manage the remote-control background daemon (start, status, stop)
  update          Update CLI
```

## `agy models`, signed out

```text
Fetching available models...
Error: Please sign in to view available models. Launch the CLI without arguments to sign in.
```

(exit code 1)

## One task, the words on standard input, signed out

`printf '%s\n' '{"event":"user","message":{"role":"user","content":"Reply with the single word OK."}}' | agy -p= --input-format stream-json --output-format stream-json --mode plan`:

```text
Error: authentication required. Run 'antigravity' to log in, then retry.
error: authentication failed or timed out
{"event":"result","result":{"conversation_id":"","status":"ERROR","response":"","error":"authentication failed or timed out","duration_seconds":0,"num_turns":0,"usage":{"input_tokens":0,"output_tokens":0,"thinking_tokens":0,"cache_read_tokens":0,"total_tokens":0}}}
```

(exit code 1)

# Claude's exact versions on the owner's subscription (Part A, 2026-09-29)

From the owner's PC, Claude Code **2.1.284**, Claude Max, signed in with the subscription
(`SignIn : none` on every task, so no key was used). Each name was run as a one-word task, and
Claude Code's own `init` reported the exact model it ran:

| Name     | Exact model it ran          | Ran by its exact name                                                                                                           |
| -------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `opus`   | `claude-opus-5-5`           | yes, "OK"                                                                                                                       |
| `sonnet` | `claude-sonnet-5-5`         | yes, "OK"                                                                                                                       |
| `haiku`  | `claude-haiku-4-5-20251001` | yes, "OK"                                                                                                                       |
| `fable`  | `claude-fable-5-1`          | accepted, then stopped at the plan's weekly Fable usage limit ("You've reached your Fable limit"): a usage limit, not a refusal |

Claude Code's own `/model` list showed the same four: Opus 5.5, Fable 5.1, Sonnet 5.5, and Haiku
4.5. The first run used an old `ANTHROPIC_API_KEY` setting on the PC instead of the subscription,
and every task was refused; the owner turned it off for the check. Plenipo never passes that
setting to Claude Code.

# Codex's models on the owner's ChatGPT sign-in (Part B, 2026-09-29)

From the owner's PC, Codex **0.159.0** (updated from 0.145.0), `Logged in using ChatGPT`. The list
is Codex's app server's `model/list` with `includeHidden: true`; "Ran" is a one-word task with that
model name. No email or account name is in either file.

```text
model             label             hidden efforts
-----             -----             ------ -------
gpt-6.1-sol       GPT-6.1-Sol        False low medium high xhigh max ultra
gpt-6-astra       GPT-6-Astra        False low medium high xhigh max ultra
gpt-6-sol         GPT-6-Sol          False low medium high xhigh max ultra
gpt-6-luna        GPT-6-Luna         False low medium high xhigh max
gpt-reserve       GPT-Reserve         True low medium high xhigh max
gpt-5.6-sol       GPT-5.6-Sol        False low medium high xhigh max ultra
gpt-5.6-terra     GPT-5.6-Terra      False low medium high xhigh max ultra
gpt-5.6-luna      GPT-5.6-Luna       False low medium high xhigh max
gpt-5.5           GPT-5.5            False low medium high xhigh
codex-auto-review Codex Auto Review   True low medium high xhigh max
```

```text
Model               Ran Said
-----               --- ----
gpt-reserve        True OK
codex-auto-review  True OK
gpt-5.4           False The 'gpt-5.4' model is not supported when using Codex with a ChatGPT account.
gpt-5.3-codex     False The 'gpt-5.3-codex' model is not supported when using Codex with a ChatGPT account.
gpt-5.2-codex     False The 'gpt-5.2-codex' model is not supported when using Codex with a ChatGPT account.
gpt-5.2           False The 'gpt-5.2' model is not supported when using Codex with a ChatGPT account.
gpt-5.1-codex-max False The 'gpt-5.1-codex-max' model is not supported when using Codex with a ChatGPT account.
gpt-5.1-codex     False The 'gpt-5.1-codex' model is not supported when using Codex with a ChatGPT account.
gpt-5.1           False The 'gpt-5.1' model is not supported when using Codex with a ChatGPT account.
gpt-5-codex       False The 'gpt-5-codex' model is not supported when using Codex with a ChatGPT account.
gpt-5             False The 'gpt-5' model is not supported when using Codex with a ChatGPT account.
o3                False The 'o3' model is not supported when using Codex with a ChatGPT account.
o4-mini           False The 'o4-mini' model is not supported when using Codex with a ChatGPT account.
gpt-4.1           False The 'gpt-4.1' model is not supported when using Codex with a ChatGPT account.
```

Each refusal came as `{"type":"error","status":400,"error":{"type":"invalid_request_error",
"message":"…"}}`; only the message is shown above. On Codex 0.145.0 the same twelve were refused,
and the list had no GPT-6 models.

# Antigravity CLI on the owner's PC (Part E, 2026-09-29)

From the owner's Windows PC, signed in with the owner's Google account, Antigravity CLI **1.2.13**
(`%LOCALAPPDATA%\agy\bin\agy.exe`, 200 MB, a real `.exe`). The steps ran in the older Windows
PowerShell, which saved the files as UTF-16; they are shown here as plain text, with the home
folder's path hidden.

- **`agy models`, signed in:** listed fourteen models from three companies, exit code 0:
  - `gemini-3.8-flash-high` — Gemini 3.8 Flash (High)
  - `gemini-3.8-flash-medium` — Gemini 3.8 Flash (Medium)
  - `gemini-3.8-flash-low` — Gemini 3.8 Flash (Low)
  - `gemini-3.7-flash-high` — Gemini 3.7 Flash (High)
  - `gemini-3.7-flash-medium` — Gemini 3.7 Flash (Medium)
  - `gemini-3.7-flash-low` — Gemini 3.7 Flash (Low)
  - `gemini-3.6-flash-high` — Gemini 3.6 Flash (High)
  - `gemini-3.6-flash-medium` — Gemini 3.6 Flash (Medium)
  - `gemini-3.6-flash-low` — Gemini 3.6 Flash (Low)
  - `gemini-3.1-pro-high` — Gemini 3.1 Pro (High)
  - `gemini-3.1-pro-low` — Gemini 3.1 Pro (Low)
  - `claude-sonnet-4-6` — Claude Sonnet 4.6 (Thinking)
  - `claude-opus-4-6-thinking` — Claude Opus 4.6 (Thinking)
  - `gpt-oss-120b-medium` — GPT-OSS 120B (Medium)
- **One task, the words on standard input,** `--mode plan --sandbox`: `status: SUCCESS`,
  `response: "OK\n"`, exit code 0. The answer streamed as `text_delta` in `step_update` events,
  with token counts (read, written, thinking).
- **The same conversation again** (`--conversation <ID>`): the same ID, `num_turns: 2`, and it
  answered "OK" to "Which single word did you reply with just now?", exit code 0.
- **An empty home folder did not sign it out:** with `USERPROFILE` pointed at an empty folder,
  `agy models` still listed the same fourteen models. The sign-in is kept outside the home folder
  (Windows Credential Manager, as Google's install page says it tries first), so Plenipo can give
  Antigravity a settings folder of its own and still use the owner's sign-in.
- **The owner's own settings file** held only `trustedWorkspaces`: no `useG1Credits`, no
  `modelProvider`.
- **Version before and after the checks:** 1.2.13 both times.
- **Not run:** the made-up key half of E6. It built a path with `Join-Path` and three parts, which
  the older Windows PowerShell does not accept; E9 repeats it in a form both accept.

```text
{"event":"init","conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","init":{"cwd":"<home>\\Desktop\\plenipo-checks","tools":["… 57 tools"],"permission_mode":"request-review"}}
{"event":"step_update","step_update":{"conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","step_index":0,"state":"DONE","step_type":"user_input"}}
{"event":"step_update","step_update":{"conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","step_index":1,"state":"ACTIVE","step_type":"agent_response","text_delta":"OK"}}
{"event":"step_update","step_update":{"conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","step_index":1,"state":"DONE","step_type":"agent_response","text_delta":"\n","duration_seconds":5.3100739,"usage":{"input_tokens":13685,"output_tokens":979,"thinking_tokens":978,"cache_read_tokens":0,"total_tokens":14664}}}
{"event":"result","result":{"conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","status":"SUCCESS","response":"OK\n","duration_seconds":5.4362304,"num_turns":1,"usage":{"input_tokens":13685,"output_tokens":979,"thinking_tokens":978,"cache_read_tokens":0,"total_tokens":14664}}}
exit code: 0
```

```text
{"event":"init","conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","init":{"cwd":"<home>\\Desktop\\plenipo-checks","tools":["… 57 tools"],"permission_mode":"request-review"}}
{"event":"step_update","step_update":{"conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","step_index":2,"state":"DONE","step_type":"user_input"}}
{"event":"step_update","step_update":{"conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","step_index":3,"state":"DONE","step_type":"system_message","duration_seconds":0.0049038}}
{"event":"step_update","step_update":{"conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","step_index":4,"state":"ACTIVE","step_type":"agent_response","text_delta":"OK"}}
{"event":"step_update","step_update":{"conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","step_index":4,"state":"DONE","step_type":"agent_response","text_delta":"\n","duration_seconds":5.6966892,"usage":{"input_tokens":15888,"output_tokens":126,"thinking_tokens":125,"cache_read_tokens":0,"total_tokens":16014}}}
{"event":"result","result":{"conversation_id":"b76723ac-06b7-48d2-9c0b-1f81db003074","status":"SUCCESS","response":"OK\n","duration_seconds":14.7742882,"num_turns":2,"usage":{"input_tokens":29573,"output_tokens":1105,"thinking_tokens":1103,"cache_read_tokens":0,"total_tokens":30678}}}
exit code: 0
```

## Paid credits and the plan (E4 and E8, the owner's screenshots)

The owner sent two screenshots from Antigravity itself (not kept here: they show the owner's
email address). What they show:

- `/credits`: **"AI Credits not enabled"**.
- Its settings screen: **"Use AI Credits" off**.
- The plan: **"Antigravity Starter Quota"**.

## Found on the build machine after Part E

- **Self-updates can be turned off:** the program reads `AGY_CLI_DISABLE_AUTO_UPDATE` (not in
  Google's documentation). With it set to `true`, a start logged no "Spawned background update
  process"; without it, or with `1`, `TRUE`, or `yes`, it did. Plenipo would set
  `AGY_CLI_DISABLE_AUTO_UPDATE=true` for its tasks, as it sets `GROK_DISABLE_AUTOUPDATER=1`.
- **The paid-credits setting takes `true` or `false`,** not the `on`/`off` Google's settings page
  lists: `"useG1Credits": "off"` (and `"false"`, `"never"`, `"disabled"`) was refused ("failed to
  load cli settings, using defaults: invalid settings: useG1Credits: invalid value "off"").
  `"useG1Credits": false` loaded with no error, next to `"toolPermission": "strict"`.

# Antigravity run the way Plenipo would (Part E9, the owner's PC, 2026-09-29)

From the owner's Windows PC, Antigravity CLI 1.2.13, with `USERPROFILE` pointed at a settings folder
of Plenipo's own (`{"useG1Credits": false, "toolPermission": "strict"}`) and
`AGY_CLI_DISABLE_AUTO_UPDATE=true`, in an empty work folder. The home folder's path is hidden.

- **The sign-in check with Plenipo's own settings folder:** `agy models` listed the same fourteen
  models, exit code 0. The owner's sign-in still works from another home folder.
- **With a made-up key in `GEMINI_API_KEY`** (and no `modelProvider` setting): the same fourteen
  models, exit code 0. A key variable alone does not switch it to pay-per-use.
- **Asked to write a file, read-only (`--mode plan --sandbox`):** `init` reported
  `"permission_mode":"strict"` (Plenipo's settings were read). It tried to run a program
  (`run_command`, `Get-ChildItem`); one-task mode refused it ("a tool required the "command"
  permission that headless mode cannot prompt for, so it was auto-denied"). The task ended
  `SUCCESS` with an empty answer and `denied_actions: [{"action":"command"}]`, exit code 0, and
  `proof.txt written: False`.

```text
{"event":"init","conversation_id":"c875ec1e-6329-46e0-b6f6-5136322c6a92","init":{"cwd":"<home>\\Desktop\\plenipo-checks\\agy-work","tools":["… 57 tools, including run_command, view_file, write_to_file, read_url_content, search_web, and browser tools"],"permission_mode":"strict"}}
{"event":"step_update","step_update":{"conversation_id":"c875ec1e-6329-46e0-b6f6-5136322c6a92","step_index":2,"state":"ACTIVE","step_type":"tool","tool_name":"run_command","tool_info":{"name":"run_command","parameters":{"CommandLine":"Get-ChildItem"}}}}
{"event":"step_update","step_update":{"conversation_id":"c875ec1e-6329-46e0-b6f6-5136322c6a92","step_index":2,"state":"ERROR","step_type":"tool","tool_name":"run_command","duration_seconds":0.0813935,"tool_info":{"name":"run_command","parameters":{"CommandLine":"Get-ChildItem"},"error":{"type":"TOOL_ERROR","message":"permission check failed for command \"Get-ChildItem\": user denied permission to run command: …"}}}}
{"event":"result","result":{"conversation_id":"c875ec1e-6329-46e0-b6f6-5136322c6a92","status":"SUCCESS","response":"","duration_seconds":3.0492578,"num_turns":1,"usage":{"input_tokens":13772,"output_tokens":478,"thinking_tokens":407,"cache_read_tokens":0,"total_tokens":14250},"denied_actions":[{"action":"command","display_name":"RunCommand"}]}}
exit code: 0
proof.txt written: False
```

## Found on the build machine while building (2026-09-29)

Signed out, Antigravity CLI 1.2.13, a settings folder of Plenipo's own, `env -i` (a cleared
environment).

- **It rewrites its settings file after reading it.** It drops permission rules of a kind it does
  not know and settings left at their default; a top-level setting it does not know is kept. Of
  `permissions.deny: ["command(*)", "read_url(*)", "url(*)", "mcp(*)", "file(*)"]` it kept
  `command(*)`, `read_url(*)`, and `mcp(*)`. The rule kinds its program names are `command`,
  `read_url`, `read_file`, `write_file`, and `mcp`; with those five, all five were kept, and its
  log said `CLI settings initialized: permissions=&{Allow:[] Deny:[command(*) read_url(*)
read_file(*) write_file(*) mcp(*)] Ask:[]}, toolPermission=strict`.
- **Plenipo's settings make `init` report strict permissions.** With Plenipo's settings file
  (plus `"modelProvider": "gemini"` and a made-up `GEMINI_API_KEY`, only so that a signed-out task
  gets as far as `init`), `init` said `"permission_mode":"strict"`; with only
  `{"modelProvider": "gemini"}`, it said `"request-review"`. Plenipo stops any task whose `init`
  does not say `strict`.
- **`useG1Credits: false` is its default:** a rewrite drops it (`{"useG1Credits": false}` became
  `{}`), and keeps `{"useG1Credits": true}`. Plenipo writes `false` before every run anyway.
- **A setting it does not know is kept, and does not stop the rest from loading**
  (`"bogusKey": 1` next to `"toolPermission": "strict"` gave `toolPermission=strict`).
- **`agy update` works with self-updates off:** with and without `AGY_CLI_DISABLE_AUTO_UPDATE=true`,
  signed out, standard input closed: "Checking for updates... (current version 1.2.13) — You are
  already on the latest version.", exit code 0. It has no check-only form (`agy update --help`
  lists no options).
- **A Gemini API key lists only Gemini's models.** With `"modelProvider": "gemini"` and a made-up
  `GEMINI_API_KEY` (an earlier check), `agy models` listed the eleven Gemini models and no
  Anthropic or OpenAI model; signed in to Google (the owner's PC), it lists all fourteen.
- **Where it keeps things:** its settings, conversations, and logs under
  `<home>/.gemini/antigravity-cli/`, and shared settings under `<home>/.gemini/config/`, so a home
  folder of Plenipo's own holds none of the owner's hooks, add-ons, or conversations.
- **`excludeTools`** in the settings file (listing `search_web`, `read_url_content`,
  `open_browser_url`, `browser_subagent`, `run_command`, `generate_image`) removed nothing: in a
  task run with a made-up Gemini API key, which gets as far as `init` before Google refuses the
  key, `init` still listed all 57 tools. Plenipo does not rely on it.
- **Leaving it, and slash commands.** Its built-in help says: "Exit: `Ctrl+D Ctrl+D` (or `/exit` or
  `/quit`)." In one-task mode, a task whose words start with `/` is not sent to the model: `/help`
  ended with "/help is answered by the CLI itself and is unavailable with --input-format
  stream-json", and `/logout` with "/logout is not available in print mode (it clears stored
  credentials, an effect that outlives the run)". Its `--disable-slash-commands` option would send
  such words to the model instead, but it also turns read-only mode off: "--mode plan has no effect
  while slash command expansion is disabled", so Plenipo does not use it.
- **Plenipo's settings, as Antigravity writes them back** (signed out, twice in a row, the same
  bytes both times): keys in alphabetical order, two spaces, a last line, and no `useG1Credits`
  line (off is the default):

```json
{
  "permissions": {
    "deny": ["command(*)", "read_url(*)", "read_file(*)", "write_file(*)", "mcp(*)"]
  },
  "toolPermission": "strict"
}
```

(Shown here on fewer lines; the file has one rule per line.) Plenipo writes exactly the file
Antigravity writes, so it replaces the file only when something changed it.
