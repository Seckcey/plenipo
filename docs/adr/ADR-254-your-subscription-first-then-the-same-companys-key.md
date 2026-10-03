# ADR-254: Your subscription first, then the same model on the same company's key

- **Status:** Accepted (the owner, second list, kept in Phase 25: "Plenipo is not using
  subscription tokens before API tokens like it's supposed to."; item 4.4 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), accepted 2026-10-03).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 4 (item 4.4)
- **Number:** after Phase 25's block (ADR-190 to ADR-199), like ADR-250.
- **Amends:** [ADR-085 (paid AI keys with spending caps)](ADR-085-paid-ai-keys-with-spending-caps.md)
  §6 ("A paid route is used only where the owner listed it") and §8 ("Nothing turns subscription
  use into paid use silently"), as ADR-190's table planned.

> **On screen** (ADR-010, plain words and rank names): a worker's **Why** says "Claude Sonnet 5.5
> (Anthropic) is Website Supervisor's first choice, on your Anthropic key: Claude Code reached its
> usage limit (resets in about 2 hours), so the same model runs on your key until then. It costs
> money: …". Claude Code's card says "Its work moves to your Anthropic key while it waits (paid
> per use, within your spending caps)." when the key can take it.

## In short

You choose a model, such as Claude Sonnet, for a job. Before, choosing it meant one AI tool
(Claude Code) and nothing else: when its plan ran out, the work waited, even with a working
Anthropic key saved. Plenipo already knew the two were the same model, but never used it.

**Accepting this record means:**

1. **Choosing a model means: on your subscription first, then on the same company's key.**
   When Claude Code reaches its usage limit, the same model runs on your Anthropic key; when Codex
   does, on your OpenAI key.
2. **Only when all three hold:** paid AI keys are switched on (Settings → Switches), a key is saved
   for that company, and the spending caps covering the work have room. Otherwise the work waits,
   as before (or moves on, as you chose in Settings → AI models).
3. **Never silently.** The worker's reason says it runs on your key and why, and that it costs
   money. It is recorded with the worker in the Ledger, like every routing choice. The AI tool's
   card says where its work goes while it waits.

## Decision

- **Which models are the same.** Each AI tool's model list already names a model the same on
  every AI tool that runs it (`same`, ADR-036 §4). Claude Code's Fable, Opus, Sonnet, and Haiku
  are now linked to the same models on the Anthropic key, and Codex's GPT-6.1 Sol, GPT-6 Astra,
  and GPT-6 Luna to the same models on the OpenAI key. A model with no link (or "its own
  choice") is not moved: the key could run a different model.
- **Which key.** The paid AI tool of the same company (Claude Code and the Anthropic key are both
  Anthropic's). OpenRouter is not used for this: it is another company.
- **When.** Only when the subscription's limit is what stops the model, at the moment the work is
  placed. A full-time agent keeps its conversation on its AI tool; its new objectives wait for the
  reset (ADR-253, when a plan runs out).
- **Every check a listed paid route gets** still applies to the key: the switch, the key, a price,
  the room under the caps, the project's allowed AI tools, and "never use" companies. "Wait for
  the reset instead of moving to another AI company" does not stop it: it is the same company.
- **Recorded** with the decision (`onKeyFor` names the subscription it stands in for), which the
  Ledger keeps with the worker (`org.worker_spawned`), so the worker's Why and the Activity trail
  say so. When the key cannot take it (no room under the caps, not priced, the project does not
  allow it), the model's note says why ("…, and your Anthropic key can't take it: …").

## Consequences

- **Easier:** with a key saved and paid keys on, work on your chosen model does not stop when your
  plan runs out.
- **Harder:** it spends money while the plan is out. The caps bound it, and the reason says so.
- **Watch for:** a full-time agent's existing conversation never moves to the key (its memory is on
  the AI tool); only new workers do.

## Alternatives considered

- **A separate switch for this.** Not needed now: "Let workers use paid AI keys" is already the
  owner's yes to paid work, and the caps bound it. Item 4.5 (stepping down) adds the switch the
  owner asked for there.
- **Move to any company's key.** Rejected: the owner asked for the same company's key, and another
  company's would change who made the model (ADR-081, cross-company review).
