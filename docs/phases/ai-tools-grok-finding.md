# Finding: Grok (xAI) does not meet the bar yet

Plenipo does not add Grok as an AI tool for now. Grok's official CLI cannot take the prompt on
standard input in its one-task mode, which ADR-014 (adding AI tools ahead of Phase 15) requires
of every AI tool. Following ADR-014 §4, this branch merges this finding instead of an adapter.
Grok can be tried again when the answer changes (see below).

## Tool and version checked

- **Grok Build**, xAI's official CLI (`grok`), `grok 1.0.41 (4220f3b224a6)`, the stable channel.
  The standard-input checks were repeated on the alpha channel's `grok 1.0.42 (4651fbdf9f13)`.
- Installed with xAI's own installer (`https://x.ai/cli/install.sh`; on Windows,
  `irm https://x.ai/cli/install.ps1 | iex`) and run on Linux without signing in, on 2026-09-26.
- The captured output is in [`evidence/ai-tools-grok/`](evidence/ai-tools-grok/README.md).

## Bar item that failed

ADR-014, bar item 1:

> **An official CLI with a non-interactive mode.** The CLI comes from the AI company (or, for
> Copilot, GitHub). It runs one task and exits by itself, reads the prompt from stdin, and never
> stops to ask a question.

Grok has an official CLI and a one-task mode (`grok -p`), but that mode **does not read the
prompt from standard input**. The same rule comes from ADR-007 (how Plenipo runs Claude Code and
Codex): the prompt goes to the program's standard input, never on its command line.

## Evidence

Each command ran in an empty folder with a short text piped to it, or with empty input.

| Command                                  | Result                                                                                                                                                                                                               |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `grok -p --output-format json`           | `error: a value is required for '--single <PROMPT>' but none was supplied`                                                                                                                                           |
| `grok -p - --output-format json`         | With **empty** input it still goes on to the sign-in check (`Not signed in…`), while `grok -p ""` is refused with `--single: prompt is empty`. So `-` is taken as the prompt text "-", not as "read standard input". |
| `grok --prompt-file - --output-format …` | `Failed to read '-': No such file or directory`                                                                                                                                                                      |
| `grok --prompt-json - --output-format …` | `--prompt-json: Invalid JSON: EOF while parsing a value`                                                                                                                                                             |
| `grok --output-format json` (no `-p`)    | `No such device or address`: without `-p` it tries to open the interactive screen.                                                                                                                                   |
| `grok --prompt-file /dev/stdin …`        | Reads standard input, but only because Linux and macOS expose it as a file. Windows has no such file.                                                                                                                |
| xAI's guide shipped with the CLI         | `~/.grok/docs/user-guide/14-headless-mode.md`: "Headless mode does not read piped stdin into the prompt. Pass external content through command substitution or `--prompt-file`."                                     |

The rest of the bar, for the record (checked signed out; nothing was signed in):

| Bar item                             | What was seen                                                                                                                                                                                                                                                                                    |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 2. Structured or streaming output    | Met on paper: `--output-format json`, `streaming-json` (one event per line) and `streaming-messages-json`, each with a session ID, the answer, and errors.                                                                                                                                       |
| 3. Subscription sign-in only         | Not checked signed in. `grok login` signs in with an X account; xAI lists SuperGrok and X Premium Plus. `GROK_DISABLE_API_KEY_AUTH=1` makes Grok refuse an API key (tested with a fake key). A key set on a model in `~/.grok/config.toml` wins over the sign-in, by xAI's own credential order. |
| 4. Status check: subscription or key | No status command. The first line of `grok models` names the credential when signed out ("You are not authenticated.") or using a key ("You are using XAI_API_KEY."). The signed-in wording was not captured.                                                                                    |
| 5. Stable execution                  | `--version`; `-s <UUID>` starts a conversation with a chosen ID and `-r <ID>` resumes it; documented exit codes (0, 1, 130, 143). Resume was not checked signed in.                                                                                                                              |

## What would change the answer

Either of these:

- **xAI lets the one-task mode read the prompt from standard input**, for example `grok -p -` or
  `--prompt-file -`. Then the rest of the bar can be checked on the owner's Windows machine
  (sign-in wording, subscription, resume), and the adapter can be built as ADR-014 describes.
  Grok's `/feedback` command sends requests to xAI.
- **A new decision record accepts a two-way route.** `grok agent stdio` takes everything,
  including the prompt, on standard input over ACP (Agent Client Protocol, JSON-RPC, one message
  per line). It answers correctly signed out (see `acp-signed-out.*.jsonl` in the evidence).
  Google's Gemini CLI speaks the same protocol (`gemini --acp`). But ACP is a conversation in
  both directions during a task, which ADR-014's adapter contract (prompt in, events out, one
  program per task) does not cover. ADR-014 §7 says that needs its own decision record.

## Not done

- **The prompt on the command line** (`grok -p "<prompt>"`): ruled out by ADR-007 and ADR-014.
  It shows the prompt in program listings, is length-limited on Windows, and invites quoting
  mistakes.
- **The prompt in a file** (`--prompt-file` with a file in the conversation's folder): not
  standard input. The owner ruled it out.
- **`--prompt-file /dev/stdin`**: works on Linux and macOS only; Windows is Plenipo's target.
- **ACP through `grok agent stdio`**: needs a new decision record first (above).
- **An xAI API key** (`XAI_API_KEY`): pay-per-use billing, out of scope under ADR-014 §7.
- **Unofficial Grok CLIs** (for example the community `superagent-ai/grok-cli`, which describes
  itself as a coding agent for the Grok API): out of scope under ADR-014 §7. Not installed.
- **Driving Grok's interactive screen**: out of scope under ADR-014 §7.
