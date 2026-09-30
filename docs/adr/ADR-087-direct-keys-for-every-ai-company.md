# ADR-087: Direct keys for every AI company whose models take one — each company's own service through Plenipo's helper, with dated prices

- **Status:** Proposed (2026-09-30), built at the owner's direction with the owner's choices (the
  [Wave 3 checklist](../phases/phase-16-wave-3-checklist.md#the-owners-choices-2026-09-30)):
  choice 5 (direct keys run through Plenipo's own helper) and choice 14, as the owner widened it:
  "We should be able to add api keys to any of the models that support api keys."
- **Date:** 2026-09-30
- **Phase:** 16, Wave 3, part 3
- **Follows:** [ADR-085 (paid AI keys with spending caps)](ADR-085-paid-ai-keys-with-spending-caps.md)
  and [ADR-086 (OpenRouter through a Plenipo helper)](ADR-086-openrouter-through-a-plenipo-helper.md),
  whose helper, key handling, and spending gate this reuses.

> **On screen** (ADR-010, plain words and rank names): an AI tool per company, named by the
> company (**Anthropic**, beside **Claude Code**), that **comes with Plenipo**; the same key form as OpenRouter's;
> and on each card, **Plenipo has not checked … with a real key yet** until the owner has. Never
> "API", "endpoint", "BYOK", or "direct integration".

## In short

Each AI company that sells pay-per-use keys gets its own paid AI tool: Anthropic, OpenAI, xAI,
Moonshot AI, Google, DeepSeek, Z.ai, MiniMax, Mistral, and Alibaba Cloud. All ten run through the
same helper as OpenRouter (ADR-086), reach only the company's own fixed address, get the key on
the helper's standard input, and are fenced by the spending caps (ADR-085). Their models and
prices come from each company's own pages, read on 2026-09-30, because the companies' own model
lists carry no prices. NVIDIA sells no pay-per-use key and has no row.

## Context

The owner has keys for Anthropic, OpenAI, xAI, and Kimi (Moonshot AI), and asked for keys to work
with every model that takes one. The companies' own programs (Claude Code, Codex, Grok, Kimi)
already run here on the owner's subscriptions, and ADR-007 §4 refuses an API-key sign-in for them
on purpose: a subscription AI tool never turns into paid use. So direct keys need their own way
in, and choice 5 chose Plenipo's helper: one program Plenipo already supervises, that the key
reaches only on its standard input, and whose every request is priced before it goes out.

Checked on 2026-09-30 against each company's own documentation:

- Every company but Anthropic offers OpenAI-style chat completions (Google's under its own base,
  `/v1beta/openai`); Anthropic has its own messages. The helper already speaks both.
- Each lists its models at a fixed address that costs nothing and needs the key, so the list is
  the key check. Google's list is its own (`/v1beta/models`), outside its OpenAI-style base.
- None of those lists gives prices, and none but OpenRouter sends a bill with the answer. Prices
  come from each company's own pricing page.
- Some companies price by prompt size (xAI and Google double at 200,000 prompt tokens; OpenAI
  charges more above 272,000), by time of day (DeepSeek's busy hours cost twice its off-peak), or
  store input for reuse by themselves and charge more for that (OpenAI's newest models, 1.25
  times the input price; MiniMax M2.7).
- NVIDIA's key comes with trial credits and no published price per use.

## Decision

### 1. A paid AI tool per company, one adapter for all

`crates/runtime/src/agent/direct.rs` holds one row per company: its paid AI tool's ID (also the
helper's service), its name, who makes its models, where the owner makes a key, and its models
with their prices. One adapter (`Direct`) serves every row. `plenipo_guard::PaidService` holds the
network half of each row: the only host, the base address, the way of talking, how the key is
carried, the key check, and the request's field for the longest answer.

| Company       | Paid AI tool    | Address (fixed)                                          | Talks           | Key carried             |
| ------------- | --------------- | -------------------------------------------------------- | --------------- | ----------------------- |
| Anthropic     | `anthropic-key` | `https://api.anthropic.com/v1`                           | Anthropic's own | `x-api-key`             |
| OpenAI        | `openai-key`    | `https://api.openai.com/v1`                              | OpenAI-style    | `Authorization: Bearer` |
| xAI           | `xai-key`       | `https://api.x.ai/v1`                                    | OpenAI-style    | `Authorization: Bearer` |
| Moonshot AI   | `moonshot-key`  | `https://api.moonshot.ai/v1`                             | OpenAI-style    | `Authorization: Bearer` |
| Google        | `google-key`    | `https://generativelanguage.googleapis.com/v1beta`       | OpenAI-style    | `x-goog-api-key`        |
| DeepSeek      | `deepseek-key`  | `https://api.deepseek.com`                               | OpenAI-style    | `Authorization: Bearer` |
| Z.ai          | `zai-key`       | `https://api.z.ai/api/paas/v4`                           | OpenAI-style    | `Authorization: Bearer` |
| MiniMax       | `minimax-key`   | `https://api.minimax.io/v1`                              | OpenAI-style    | `Authorization: Bearer` |
| Mistral       | `mistral-key`   | `https://api.mistral.ai/v1`                              | OpenAI-style    | `Authorization: Bearer` |
| Alibaba Cloud | `alibaba-key`   | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` | OpenAI-style    | `Authorization: Bearer` |

The international address where a company has more than one. Alibaba Cloud's Singapore address is
its older shared one, still offered; a key made in another region does not work there.

### 2. What each is asked

The helper sends each company what it takes: `max_completion_tokens` to OpenAI, Moonshot AI, and
MiniMax (whose reasoning models refuse `max_tokens`), `max_tokens` elsewhere; the counts at the
end of the answer (`stream_options`) except to Mistral, which refuses fields it does not know and
sends them by itself; the effort level as `reasoning_effort`, or Anthropic's `output_config`;
MiniMax's thinking apart from the answer (`reasoning_split`). The counts are read as each company
names them (DeepSeek's cached tokens, and the thinking of a company that counts it only in the
total).

### 3. Prices, dated, and never less than was spent

Each model's price per million tokens (input, cached input, output) is in its row, from the
company's own pricing page on 2026-09-30:

| Company       | Model                    | Input  | Cached | Output | Notes                                           |
| ------------- | ------------------------ | ------ | ------ | ------ | ----------------------------------------------- |
| Anthropic     | Claude Sonnet 5.5        | $2.00  | $0.20  | $10.00 |                                                 |
| Anthropic     | Claude Opus 5.5          | $4.00  | $0.20  | $20.00 |                                                 |
| Anthropic     | Claude Fable 5.1         | $10.00 | $0.25  | $50.00 |                                                 |
| Anthropic     | Claude Haiku 4.5         | $1.00  | $0.10  | $5.00  |                                                 |
| OpenAI        | GPT-6.1 Sol              | $2.00  | $0.10  | $10.00 | storing for reuse $2.50; words under 272K       |
| OpenAI        | GPT-6 Astra              | $10.00 | $1.00  | $50.00 | storing for reuse $12.50; words under 272K      |
| OpenAI        | GPT-6 Luna               | $0.10  | $0.01  | $0.50  | storing for reuse $0.125; words under 272K      |
| xAI           | Grok 4.7                 | $2.00  | $0.50  | $6.00  | words under 200K                                |
| xAI           | Grok 4.3                 | $1.25  | $0.20  | $2.50  | words under 200K                                |
| xAI           | Grok Build 0.1 (coding)  | $1.00  | $0.20  | $2.00  | words under 200K                                |
| Moonshot AI   | Kimi K3                  | $3.00  | $0.30  | $15.00 |                                                 |
| Moonshot AI   | Kimi K2.7 Code           | $0.95  | $0.19  | $4.00  |                                                 |
| Moonshot AI   | Kimi K2.6                | $0.95  | $0.16  | $4.00  |                                                 |
| Google        | Gemini 3.8 Flash         | $1.50  | $0.15  | $7.50  | half until 2026-12-31; the regular price counts |
| Google        | Gemini 3.5 Flash-Lite    | $0.30  | $0.03  | $2.50  |                                                 |
| Google        | Gemini 3.1 Pro (preview) | $2.00  | $0.20  | $12.00 | words under 200K                                |
| DeepSeek      | DeepSeek V4.1 Flash      | $0.30  | $0.006 | $1.20  | busy-hours price (off-peak is half)             |
| DeepSeek      | DeepSeek V4 Pro          | $1.32  | $0.044 | $3.96  | busy-hours price (off-peak is half)             |
| Z.ai          | GLM-5.3                  | $1.40  | $0.26  | $4.40  |                                                 |
| Z.ai          | GLM-5.3 Flash            | $0.15  | $0.03  | $0.50  |                                                 |
| MiniMax       | MiniMax M3               | $0.30  | $0.06  | $1.20  |                                                 |
| MiniMax       | MiniMax M2.7             | $0.30  | $0.06  | $1.20  | storing for reuse $0.375                        |
| Mistral       | Mistral Medium 3.5       | $1.50  | $0.15  | $7.50  |                                                 |
| Alibaba Cloud | Qwen3.8 Flash            | $0.15  | —      | $0.47  | Singapore prices                                |
| Alibaba Cloud | Qwen3.8 Max              | $2.00  | —      | $6.00  | Singapore prices                                |

1. **A model not in the row is not priced**, so it never runs on a key (ADR-085 §3.4). The
   company's own list still shows it on the card, marked "new — not checked yet".
2. **A price that steps up with the prompt's size never applies:** that model's words are held
   under the step (a token is at least one byte, so 190,000 bytes stay under 200,000 tokens).
3. **A price by time of day** is counted at its dearest.
4. **Input a company stores for reuse by itself**, at a dearer price, is counted at that price for
   every fresh input token (`Price::cache_write`), since the counts do not say which were stored.
5. **The bill** is the step's token counts at the row's prices; a company that sent a bill would
   be read first, as OpenRouter's is. Without both counts, the step is "not priced yet" and counts
   at the most it could have cost.

### 4. Not checked with a real key yet

Until the owner types each company's key into its card, nothing here has met the real service.
Each card says so, with where to make a key ("Plenipo has not checked Anthropic's service with a
real key yet…"), and the checklist lists each company's check. The owner has keys for Anthropic,
OpenAI, xAI, and Moonshot AI now; the others follow.

### 5. The same model, more than one way

Kimi K3 on Moonshot AI's own service joins Kimi K3 on Kimi Code, Ollama, and OpenRouter; GLM-5.3,
MiniMax M3, and DeepSeek V4 Pro link with Ollama's; Qwen3.8 Flash and Mistral Medium 3.5 link with
OpenRouter's (ADR-036 §4).

### 6. NVIDIA has no row

NVIDIA's key comes with trial credits and a request limit, and NVIDIA publishes no price per use:
more use means deploying models elsewhere. With no price, no paid key (ADR-085 §3.4). NVIDIA's
Nemotron runs on Ollama's cloud (ADR-017); its other models are reached through Ollama or
OpenRouter.

## Consequences

- **Every model worth having is reachable with the owner's own key**, fenced by the caps, as a
  route the owner lists (ADR-085 §6).
- **Prices are dated and can go stale.** A company that raises a price makes Plenipo count less
  than it spent until an update; one that lowers it makes Plenipo count more. The date is on every
  card, and a later update checks them again.
- **Ten more cards** on the AI tools page, each with its key form.
- **Privacy:** a Google key on Google's free tier lets Google use what it is sent to improve its
  products (Google's own terms); the setup guide says so. ADR-036's "to watch" stands.

## Alternatives considered

- **The companies' own programs signed in with a key** (Claude Code with `ANTHROPIC_API_KEY`).
  Rejected: ADR-007 §4 keeps those programs on subscriptions only, and a program holding the key
  could make requests Plenipo cannot bound or price.
- **Prices from each company's list at run time.** None gives prices; OpenRouter alone does.
- **NVIDIA with a price of zero.** Rejected: trial credits run out, and "never counted as zero"
  (ADR-085 §3.4) would be broken the day they do.

## As built

**Part 3 (2026-09-30):**

- `crates/guard/src/paid.rs`: `PaidService` with eleven services, `PaidAuth`, each service's
  address, key check, and answer-length field; Guard's rules allow each only its own host.
- `crates/runtime/src/agent/direct.rs`: the companies' rows and the `Direct` adapter;
  `crates/runtime/src/pricing.rs`: `Price::cache_write`.
- `crates/capabilities/src/paid/helper.rs`: each company's key header, request fields, Google's
  model list, "too busy" (503, 529) as not billed, and each company's counts.
- The card's note (`AiToolState.paid_note`).
- Tests: every company's address and no other's; each company asked in its own words; the row's
  prices only; a model held under its price step; storing for reuse at its price; the contract
  suite runs a task on every company's service.
