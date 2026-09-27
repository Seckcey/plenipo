# Ollama 0.34.4: real CLI and local-service output (cloud model)

Captured by the owner on Windows 11 on 2026-09-26, signed in to ollama.com on the **free plan**
(`ollama signin`), with no `OLLAMA_*` variables set. The model is a cloud model,
`gpt-oss:120b-cloud`: it runs on Ollama's servers, reached through the Ollama service on the PC.
Step 0 of adding Ollama (see [the Ollama checklist](../../../../../docs/phases/ai-tools-ollama-checklist.md)).
The Windows user name in paths is replaced with `<user>`.

| File                                  | What it shows                                                                                                                                           |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `where.txt`, `version.txt`            | A native `ollama.exe` in `%LOCALAPPDATA%\Programs\Ollama`, version 0.34.4.                                                                              |
| `help/`                               | `ollama --help`, `run --help`, `signin --help`. There is `signin`/`signout` but no sign-in status command.                                              |
| `cli/show-gpt-oss-120b-cloud.txt`     | The cloud model: 117B parameters, 131,072-token context, capabilities completion, tools, thinking (low, medium, high; default medium).                  |
| `cli/run-stdin*.txt`                  | `ollama run <model>` reads the prompt from stdin and exits by itself; the output is plain text with the thinking and terminal control codes on stderr.  |
| `api/chat-stream.jsonl`               | `POST 127.0.0.1:11434/api/chat` with `stream: true`: one JSON object per line (`thinking`, `content`), then `done` with `done_reason` and token counts. |
| `api/chat-history.jsonl`              | The same endpoint with earlier messages: the model answered from the history Plenipo sent (Ollama keeps no conversation itself).                        |
| `api/version.jsonl`, `api/tags.jsonl` | The service's version, and the cloud model listed with `remote_host: https://ollama.com` and its capabilities.                                          |
| `listening.txt`                       | The service listens on `127.0.0.1:11434` only.                                                                                                          |
