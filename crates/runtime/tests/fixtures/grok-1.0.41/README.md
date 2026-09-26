# Grok CLI 1.0.41: signed-out captures

Real output from xAI's official Grok CLI ("Grok Build", `grok 1.0.41 (4220f3b224a6)`, stable
channel, `linux-x86_64`), installed with `https://x.ai/cli/install.sh` on 2026-09-26. Nothing
here was signed in: there was no `~/.grok/auth.json`, and no real API key. These files are for
the Grok adapter's tests and fake persona; the adapter itself waits for the shared AI-tool
groundwork (the `claude/ai-tools` branch) to reach `main`.

**Redacted:** working-directory paths (now `/work`), the host name, the agent and instance IDs,
and the conversation ID Grok created (now `00000000-0000-7000-8000-000000000001`). Slash
commands Grok picked up from the capture machine's `~/.claude/skills` were removed from
`available_commands`. `grok inspect --json` is cut down to its version and login policy, because
the rest listed the capture machine's own skills. "Fake key" means `XAI_API_KEY` or
`GROK_CODE_XAI_API_KEY` set to `xai-FAKE-not-a-real-key`; the key never appears in the output.

Every task below ran in an empty folder with standard input closed or piped, and a time limit.

| File                                            | Command                                                                                       | Exit |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------- | ---- |
| `version.txt`                                   | `grok --version`                                                                              | 0    |
| `version.jsonl`                                 | `grok version --json`                                                                         | 0    |
| `update-check.jsonl`                            | `grok update --check --json`                                                                  | 0    |
| `help/*.txt`                                    | `grok [subcommand] --help` for every subcommand                                               | 0    |
| `inspect-login-policy.json`                     | `GROK_DISABLE_API_KEY_AUTH=1 grok inspect --json` (cut down)                                  | 0    |
| `models-signed-out.txt`                         | `grok models`                                                                                 | 0    |
| `models-api-key.txt`                            | `XAI_API_KEY=<fake> grok models` (same text with `GROK_CODE_XAI_API_KEY`)                     | 0    |
| `turn-signed-out.streaming-json.jsonl`          | `grok -p - --output-format streaming-json` (stdout; stderr in `turn-signed-out.stderr.txt`)   | 1    |
| `turn-signed-out.json.jsonl`                    | `grok -m not-a-model -p hi --output-format json`                                              | 1    |
| `turn-signed-out.streaming-messages-json.jsonl` | `grok -p "Reply with ok." --output-format streaming-messages-json --no-auto-update`           | 1    |
| `turn-api-key-rejected.streaming-json.jsonl`    | `XAI_API_KEY=<fake> grok -p "Reply with ok." --output-format streaming-json --no-auto-update` | 1    |
| `turn-api-key-refused.streaming-json.jsonl`     | the same with `GROK_DISABLE_API_KEY_AUTH=1`                                                   | 1    |
| `acp-signed-out.client.jsonl`                   | what was sent to `grok agent stdio`: `initialize`, then `session/new` (never `authenticate`)  | —    |
| `acp-signed-out.agent.jsonl`                    | what `grok agent stdio` answered                                                              | 0    |

## What these show

- **The prompt cannot go in on standard input with `-p` on Windows.** `-p` needs a value, and
  `-p -` is the literal prompt "-": with empty standard input it still reaches the sign-in
  check, while `-p ""` is refused as empty. `--prompt-file -` and `--prompt-json -` read the
  argument literally too. `--prompt-file /dev/stdin` reads standard input on Linux and macOS
  only. The bundled guide (`~/.grok/docs/user-guide/14-headless-mode.md`) says: "Headless mode
  does not read piped stdin into the prompt." A task with no `-p` and piped input fails with
  "No such device or address".
- **`grok agent stdio` takes everything on standard input** (ACP, JSON-RPC 2.0, one message per
  line). `initialize` works signed out and returns the model menu with each model's effort
  levels, and one sign-in method (`grok.com`, "Sign in with Grok"). `session/new` signed out
  fails with `-32000 Authentication required`.
- **Sign-in status:** there is no `auth status` command. `grok models` prints the credential in
  use on its first line ("You are not authenticated." / "You are using XAI_API_KEY."). The
  signed-in text is still to be captured on the owner's machine.
- **`GROK_DISABLE_API_KEY_AUTH=1`** makes a task with an API key stop at "Not signed in"
  instead of calling xAI (compare the two `turn-api-key-*` files). `grok models` still says
  "You are using XAI_API_KEY." with it set. `grok inspect --json` shows it under
  `loginPolicy`.
- **`streaming-messages-json`** opens with a `system`/`init` line whose `apiKeySource` is
  documented as `user` for an API key and `oauth` otherwise. Signed out with no key at all it
  still says `user`, so only `oauth` can count as a subscription sign-in; the signed-in value is
  still to be captured on the owner's machine.
- **Models and effort (signed out):** `grok-4.6` (default; `xhigh`, `high`, `medium`, `low`;
  default `high`) and `grok-4.5` (`high`, `medium`, `low`; default `high`). xAI's web docs name
  `grok-4.7`; this version does not list it signed out.
- **Credential order** (bundled `02-authentication.md`): a per-model `api_key`/`env_key` in
  `config.toml` wins, then the signed-in session, then `XAI_API_KEY`. `GROK_CODE_XAI_API_KEY`
  is read the same way as `XAI_API_KEY`.
