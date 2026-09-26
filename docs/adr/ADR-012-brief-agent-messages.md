# ADR-012: Brief messages between agents

- **Status:** Accepted (by the owner, 2026-09-26)
- **Date:** 2026-09-26
- **Phase:** 5 (follow-up, v0.6.1)

## Context

The owner's Phase 5 check on Windows passed, but its team round trip (step 7: a supervisor asks
the Senior Developer to write a function and the Code Reviewer to review it) took much longer
than it should. The owner saw the agents write long, over-cautious requests, answer with the
whole context of the previous reply, and add more instructions for the next agent, so the
messages grew with every hand-on. Owner direction: messages between agents must be "brief, to
the point, and VERY concise."

Plenipo's own instructions invited this (ADR-008, how workers hand work to each other):

- The request template asked for the sub-task "stated completely" and showed the requester's
  whole answer (`{"kind": "answer"}`) as its context.
- The worker was told to "answer normally", and nothing asked for a short reply.
- The limits were generous: a request up to 4,000 characters, acceptance criteria up to 2,000,
  the requester's answer up to 12 KiB as context, and a reply up to 16 KiB.

## Decision

1. **Instructions ask for brevity.** The protocol and its format (`plenipo-liaison/1`) are
   unchanged; the wording changes:
   - **Requests** read like a short note to a colleague: the task and the result needed, in a
     few short sentences; no greetings, background, caveats, or restated rules. The template
     shows only `to` and `objective`.
   - **Context** carries only what the other worker needs; a short excerpt (for example the code
     to review) is preferred to the requester's whole answer.
   - **Replies** are brief: only the result asked for (for code, the code and a sentence or two),
     without repeating the request or the context and without instructions for other workers.
     Workers are told that long replies are cut off.
   - **Continuing** after replies, a worker takes only what it needs from them and does not copy
     them in full into its answer or into new requests.
   - **Team leads** give each member of their team a short, specific task.
2. **Limits back the instructions up** (they change the numbers in ADR-008 §1 and §4):

   | Limit                                    | Before      | Now         |
   | ---------------------------------------- | ----------- | ----------- |
   | Request (`objective`)                    | 4,000 chars | 1,000 chars |
   | `acceptanceCriteria`                     | 2,000 chars | 500 chars   |
   | The requester's answer passed as context | 12 KiB      | 6 KiB       |
   | Reply text passed back to the requester  | 16 KiB      | 8 KiB       |

   Excerpts (8,000 characters each, 24 KiB in total) and task results passed as context (8 KiB)
   are unchanged. A request over a limit is refused with the reason ("… keep it short"), and the
   requester can send a shorter one in its next round. A cut reply ends with "…"; the worker's
   full answer stays with its own task in the Ledger.

## Consequences

- Team round trips should be shorter and use less of the owner's subscriptions. Only the real
  AI tools can confirm it: the owner re-runs step 7 of the Phase 5 check.
- A request over 1,000 characters costs a round (refused, then sent again shorter). The limit
  is stated in the instructions, so this should be rare.
- A reply over 8 KiB loses its end, which could cut a large piece of code. Workers are asked to
  keep replies short; if real use shows the limit is too tight, it is one setting
  (`LiaisonConfig::reply_text_bytes`).

## Alternatives considered

- **Instructions only, no new limits.** Models do not reliably follow length guidance; the limits
  turn it into a rule.
- **Summarize each reply with another model call.** It adds an agent turn to every hand-on, the
  opposite of the goal.
- **Shorten an over-long request instead of refusing it.** It could silently drop the part of
  the request that matters.
