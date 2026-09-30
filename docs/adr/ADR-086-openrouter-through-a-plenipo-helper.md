# ADR-086: OpenRouter through a Plenipo helper — one fixed address, the key on standard input, and a price before every task

- **Status:** Proposed (2026-09-30), built at the owner's direction with the owner's choices (the
  [Wave 3 checklist](../phases/phase-16-wave-3-checklist.md#the-owners-choices-2026-09-30));
  choice 7 (a short checked list plus any model named exactly) and choice 8 ("No limits" on
  which hosting companies OpenRouter uses).
- **Date:** 2026-09-30
- **Phase:** 16, Wave 3, part 2
- **Follows:** [ADR-036 (every AI model worth having)](ADR-036-every-ai-model.md) §4 (more than
  one route to a model) and [ADR-085 (paid AI keys with spending caps)](ADR-085-paid-ai-keys-with-spending-caps.md)
  §5 to §7.
- **Built like:** [ADR-017 (Ollama's cloud models through the Ollama service on this PC)](ADR-017-ollama-cloud-models.md):
  a helper per task, supervised like any AI tool.

> **On screen** (ADR-010, plain words and rank names): an AI tool **OpenRouter**, "Comes with
> Plenipo", **Paid per use with your key, within your spending caps**, **Save and check**,
> **Replace key**, **Remove key**, **No key yet**, **Your key works**, and a model's price as
> "$3.00 in, $15.00 out per million tokens". Never "API", "endpoint", "BYOK", "bearer token", or
> "metered".

## In short

OpenRouter is a service that sells many companies' models behind one key. Plenipo reaches it
through **its own helper**: Plenipo runs itself in a helper mode for each task, the way it runs
the Ollama helper. The helper:

1. talks only to `https://openrouter.ai/api/v1`, follows no redirects, and checks the address with
   Guard's rules before it connects (the app checks it before the helper starts, too);
2. gets the owner's key on the **first line of its standard input**, never as an argument or an
   environment variable;
3. sends at most a fixed amount of the conversation and asks for at most a fixed number of answer
   tokens, so the most a task can cost is known before it starts and set aside under the caps;
4. reads OpenRouter's own bill for the request when it gives one, or prices the token counts from
   the price list it read before the task.

## Context

ADR-036 asked for OpenRouter as a way to reach models Plenipo has no AI tool for (Qwen, Mistral,
Meta's Llama) and a third way to reach Kimi K3. OpenRouter has no program to install: it is a web
service with an OpenAI-style chat interface. Checked on 2026-09-30 against OpenRouter's own
documentation:

- `GET /api/v1/key` answers with the key's own details (a cheap check that costs nothing);
  `401` means the key is not accepted.
- `GET /api/v1/models` lists every model with its prices per token, without a key.
- `POST /api/v1/chat/completions` streams the answer as server-sent events. With
  `usage: {include: true}` its last event carries the token counts and the request's own cost
  (`usage.cost`, in dollars).
- `402` means the account is out of credit; `429` means a usage limit.

Plenipo already runs every AI tool as a supervised program with a time limit, cancel, a Ledger
record, and restart handling. ADR-017 showed a helper mode of Plenipo itself fits that model for a
web service, without loading anything into Plenipo while it runs (ADR-014).

## Decision

### 1. A helper per task, inside Plenipo's own program

`plenipo-desktop --plenipo-paid openrouter check|chat …` is Plenipo run in a helper mode
(`crates/capabilities/src/paid/helper.rs`). The runtime starts it as the task's program, with the
same supervision as every AI tool. On screen it is an AI tool, **OpenRouter**, that **comes with
Plenipo**: nothing to install, and it is updated when Plenipo is.

- `check`: the key works (`/key`), and the models with their prices (`/models`, capped at 1,000
  models and 8 MB), on one line the runtime reads up to 1 MB of, within 75 seconds. The runtime
  runs it before every turn, so the prices are from right before the task.
- `chat --model M --session ID [--resume] --max-input-bytes N --max-output-tokens N [--effort L]`:
  one request, streamed. The helper writes one JSON object per line: `session`, `thinking`,
  `text`, `answer`, `done` (token counts and OpenRouter's bill), `notice`, or `error`.

### 2. One fixed address, checked twice

Guard gains a purpose for paid AI services (`Purpose::PaidAi(PaidService)`) with each service's own
hosts: for OpenRouter, only `openrouter.ai`, only over `https`. The app checks the address before
the helper starts; the helper checks every request again with the same rules before it connects.
Redirects are never followed. The connection uses Windows' own TLS through the `reqwest` library
(`native-tls`), inside the helper only; the app itself makes no such request. Test builds may point
a paid service at a stand-in on this PC (`paid_test_port`), never a release.

### 3. The key

The key comes from the Vault on the helper's first line of standard input (`{"key":"…"}`, at most
4 KB) and nowhere else. The helper never writes it to a file, prints it, or puts it in an error:
anything OpenRouter echoes back is hidden before it leaves the helper, and Plenipo's secret filter
hides it everywhere else too (ADR-085 §5).

### 4. The most a task can cost

The helper keeps each conversation in the session's own folder
(`.plenipo-paid-openrouter-<ID>.json`) and sends at most `--max-input-bytes` of it (400,000 bytes),
dropping the oldest messages first; it asks for at most `--max-output-tokens` (16,000 by default,
32,000 at most). The runtime turns those limits into the most the step could cost at the model's
price and sets that aside under the caps before the helper starts (ADR-085 §2.5). A task that does
not finish leaves the conversation unchanged. The file keeps only each message's role and words,
and only its newest part (twice what a task sends), so what is sent is always what was counted.
An answer silent for five minutes ends, and an answer counts as finished only when OpenRouter says
it is done.

### 5. The bill

The last event's `usage.cost` is OpenRouter's own bill and is recorded exactly (ADR-085 §3.3),
with what the AI company billed the owner's own key on OpenRouter added when the account uses one
(`cost_details.upstream_inference_cost`). Without a bill, both token counts are priced from the
price list read before the task. Without either, the task counts at the most it could have cost
("not priced yet"), never as zero.

Each request tells OpenRouter the most it may cost: the price set aside for, per million tokens,
and no fee per request (`provider.max_price`), so OpenRouter sends it to no company that charges
more. A model whose price list names a fee beyond its token prices (per request, per web search,
dearer thinking, or other prices for some requests) is not priced, so it never runs on the key.

### 6. Errors, in plain words

| OpenRouter says               | Plenipo says                                                        |
| ----------------------------- | ------------------------------------------------------------------- |
| `401`                         | OpenRouter refused the key: it needs a new key                      |
| `403`                         | OpenRouter refused this request (a guardrail); not billed           |
| `402`                         | The account is out of credit                                        |
| `429`                         | A usage limit, until its reset time (`X-RateLimit-Reset`)           |
| `400`, `404`, `413`, or `422` | It did not take this request (it never reached a model); not billed |
| No connection                 | OpenRouter could not be reached; not billed                         |
| Any other answer              | OpenRouter answered with that status, and its own words             |

### 7. Models

A short list Plenipo checked on OpenRouter on 2026-09-30, each with the company that made it
(ADR-081): Qwen3.8 Max Prime and Qwen3.8 Flash (Alibaba), Mistral Medium 3.5 and Devstral
(Mistral), Llama 4 Maverick and Llama 4 Scout (Meta), and Kimi K3 (Moonshot AI). Any other model
OpenRouter lists can be named exactly and is marked "new — not checked yet". A model without a
price on OpenRouter's list is never run on the key (ADR-085 §3.4). Kimi K3 is linked with Kimi K3
on Kimi Code and on Ollama, so the owner sees one model with three ways to reach it.

### 8. What a worker on OpenRouter can do

Answer in text: write, review, and explain. It reads no files and runs nothing in this wave
(choice 6); the Router's reason says "A worker on it answers in text only."

### 9. Privacy

No limit on which hosting companies OpenRouter sends a request to (choice 8). ADR-036's "to
watch" stands: where prompts are processed, and 8 West IT client data, which reaches OpenRouter
only through a position the owner put it on.

## Consequences

- **Models Plenipo had no AI tool for** (Qwen, Mistral, Llama) and a third way to Kimi K3, paid per
  use and fenced by the caps.
- **No new program to install or keep up to date:** the helper is Plenipo.
- **A second network client** (`reqwest`) exists, but only in the helper mode, reaching only fixed
  addresses Guard allows.
- **A price can change between the check and the bill.** OpenRouter's own bill is what is
  recorded; a bill over what was set aside is recorded as it was and stops that task's paid work
  (ADR-085 §2.6).

## Alternatives considered

- **OpenRouter's own programs or an OpenAI-compatible command-line tool.** Rejected: a third-party
  program would hold the key and make requests Plenipo cannot bound or price before they go out.
- **Requests from the app itself.** Rejected: a helper per task keeps the time limit, cancel, and
  records every AI tool has, and keeps the network client out of the app.
- **The key in an environment variable** (ADR-036 §2.4). Rejected for standard input (ADR-085 §5).

## As built

**Part 2 (2026-09-30):**

- `crates/guard/src/paid.rs`: `PaidService` (OpenRouter's address, paths, and way of talking),
  `PaidKeyInfo`; `crates/guard/src/outbound.rs`: `Purpose::PaidAi`, one host, `https` only.
- `crates/capabilities/src/paid/helper.rs`: the helper (`check`, `chat`, the stream reader, error
  words, the key hidden); `crates/capabilities/src/paid/mod.rs`: the gate (set aside before a step,
  settle after), and saving, checking, and removing a key.
- `crates/runtime/src/agent/paid.rs`: the OpenRouter AI tool (its models, prices, limits, and the
  reader for the helper's lines), `PaidGate`, and the conversation file.
- `crates/runtime/src/bin/plenipo-fake-agent.rs`: the `--plenipo-paid` stand-in, for the contract
  suite and the tests.
- Desktop: the helper mode in `main.rs`; `save_paid_key` and `remove_paid_key` (main window only);
  the key form on OpenRouter's card.
