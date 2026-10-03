# ADR-202: Prompt caching for Anthropic models on your key and through OpenRouter

- **Status:** Accepted (the owner, first list, kept in Phase 25: "Make sure we are using prompt
  caching for all Anthropic models."; item 4.1 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), accepted 2026-10-03).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 4 (item 4.1)
- **Number:** after Phase 25's block (ADR-190 to ADR-199), like ADR-200.
- **Amends:** [ADR-085 (paid AI keys with spending caps)](ADR-085-paid-ai-keys-with-spending-caps.md)
  §3.5: "Plenipo's helpers never ask a service to store a conversation for reuse."

> **On screen** (ADR-010, plain words and rank names): an AI tool's Usage tab has **Saved by
> caching**: "400 of 1,000 read came from the cache (40%)", today, this week, and last week.

## In short

Claude Code caches by itself. Your paid Anthropic key and OpenRouter did not, because Plenipo's
helper sent no cache marks: ADR-085 §3.5 said never to ask a service to store a conversation. You
have now asked for caching on every Anthropic model.

**Accepting this record means:**

1. **For Anthropic's models** (on your Anthropic key, and "anthropic/…" models through OpenRouter),
   each request marks its reusable start for Anthropic's prompt cache: its instructions, and its
   latest message, so the next task in the conversation reads everything before it from the cache.
   Other companies' requests are unchanged.
2. **The cost is counted right.** Input stored in the cache costs 1.25 times the input price
   (Anthropic's five-minute cache), and input read back costs the cached price (about a tenth).
   Anthropic's price rows get a cache-write price; through OpenRouter, the cache-write price comes
   from OpenRouter's own list. The spending gate sets aside the most a step could cost: all of its
   input as stored.
3. **Saved by caching** shows on each AI tool's Usage tab, from the reused tokens Plenipo already
   counts.

## Decision

- Two of the four cache marks Anthropic allows are used. A request shorter than Anthropic's
  smallest cached size is simply not cached; nothing fails.
- Bills stay rounded up, and every fresh input token is counted at the cache-write price, so
  Plenipo never counts less than was spent (ADR-085 §3).
- The prices are checked on the owner's PC against Anthropic's own page, per ADR-081 §8.

## Consequences

- Long conversations on a paid key cost much less after their first task.
- The first task of a conversation costs up to a quarter more for its input.

## Alternatives considered

- **Keep caching off.** Rejected by the owner.
- **Cache through OpenRouter's automatic caching only.** Rejected: OpenRouter caches Anthropic's
  models only where the request marks them.
