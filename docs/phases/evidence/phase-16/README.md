# Phase 16 evidence: Google's Gemini CLI 0.61.0, read before the owner's check

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
