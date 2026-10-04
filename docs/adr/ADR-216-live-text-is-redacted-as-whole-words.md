# ADR-216: Live text is redacted as whole words

- **Status:** Proposed (the Development Coordinator and the independent security reviewer settled
  the design on 2026-10-04; the owner accepts it at review)
- **Date:** 2026-10-04
- **Phase:** none (security hardening: a gap found on 2026-10-04 while checking how the text an
  agent streams is hidden, after [ADR-214](ADR-214-files-at-the-moment-of-use-and-scripts-follow-the-lists.md);
  not in the 2026-10-02 report)
- **Builds on:** [ADR-013 (Guard and the capability broker)](ADR-013-guard-capability-broker.md),
  whose secret filter hides the owner's stored secrets and recognizable key shapes in what is
  shown and recorded; [ADR-200 (a live chat with each agent)](ADR-200-a-live-chat-with-each-agent.md),
  which shows an agent's words and thinking as they stream. No earlier decision is changed.
- **Made by:** 8 West Ventures, LLC, for Plenipo.

## In short

An agent's words and thinking reach the screen as they are written, in small pieces (Claude Code
hands over a few letters at a time; an AI tool over ACP, two halves). Each piece passed the secret
filter on its own. But the filter recognizes a secret by its whole shape — an `sk-…` key, a
`-----BEGIN … PRIVATE KEY-----` block, a stored password — and a key written across two pieces has
no whole shape in either: both passed, the screen joined them, and the key stood there in plain
sight. The stored copy of the message was whole and hidden; the live screen was not.

Now the runtime holds back the word being written. A piece goes on only up to its newest space (or
other whitespace); the rest waits for the next piece, so the filter always sees whole words. A
private key is held from its `-----BEGIN` to its `-----END`. The start of a stored secret that has
spaces in it waits until the rest has come. Whatever is held goes on before anything of another
kind (a message, a tool call, a sign) and when the turn ends, so the screen keeps the order.

Accepting this record means: live words appear one word later than before (the last word of a
line shows when the next one begins, or when the block ends); a secret written over several pieces
shows as `[hidden by Plenipo: …]`, never half of it; and a key block that never ends holds the
rest of that block's words until the block ends, where the stored copy hides them too.

## Context

What the code did at `0269bbeb`:

- `crates/runtime/src/agent/service.rs`: `TurnContext::event` passed every event through
  `filtered`, which redacted a `TextDelta` (words) or `Reasoning` (thinking) piece on its own with
  the broker's filter, `Arc<dyn Fn(&str) -> String>`, and sent it on. Nothing was held: a piece of
  `sk-ant-api03-abcd` followed by `efghijklmnopqrstuvwxyz` was two harmless-looking pieces.
- The screens (the live chat, Watch, the phone page) join the pieces. The in-memory activity kept
  for a reload (`buffer`) joins them too, so a reload showed the same joined text.
- The stored copies were safe: `TextDelta` and `Reasoning` are live only (no Ledger kind), and the
  `Message` recorded at the turn's end is redacted whole.
- `crates/guard/src/redact.rs`: `Redactor::redact` hides stored values (six characters or more)
  and the recognizable shapes: private keys, `sk-`, `sk_live_`, `ghp_`, `github_pat_`, `AKIA`,
  `AIza`, `xox`, `xai-`, `npm_`, JWTs, `bearer ` tokens, `user:pass@` in addresses, `PASSWORD=`
  and `"apiKey": "…"` settings (the last four hide the value after the prefix).

Measured with the fake agent (`plenipo-fake-agent`), which streams a Claude Code answer in
16-byte pieces: a stored key in the answer reached the screens whole, piece by piece.

## Decision

1. **The word being written is held back** (`crates/runtime/src/agent/live_text.rs`,
   `LiveText`). A streamed piece joins what is held; what goes on is everything up to and
   including the newest whitespace, as the agent wrote it; the rest stays held until the next
   piece, or the flush below. Nothing that is held is shown.
2. **The filter sees the line already shown in front of the held word.** The text sent on is
   redacted with the current line of already-shown text (up to 200 characters) in front of it, so
   prefixes that were shown earlier (`bearer `, `export MY_DATABASE_PASSWORD=`, `"apiKey": "`)
   still make their value match. Only the new text is changed: `Redactor::redact_from(text,
start)` hides every secret whose end lies at or after `start`, hiding one that began before
   `start` from `start` on, and returns the first `start` bytes byte for byte. A marker placed
   earlier is never matched again. If a filter ever broke the byte-for-byte promise, the new text
   would be redacted on its own rather than shown wrong.
3. **A private key is held from BEGIN to END** (reviewer's S3). From `-----BEGIN`, wherever it
   lies in the held text (a word may carry it: `key="-----BEGIN`), nothing of the key goes on
   until the `-----END … -----` line closes it; then the text before the key, one
   `[hidden by Plenipo: private key]`, and what follows up to its newest whitespace go on. A
   block that ends inside the key (a message follows, or the turn ends) sends the marker and the
   key is over for the live view; the stored copy hides from BEGIN to the message's end.
4. **A stored secret with spaces in it is held from its first word** (reviewer's C4). Guard's
   `stored_secret_start` finds the earliest point where the end of the shown-and-held text is the
   beginning (or the whole) of a stored value containing whitespace; what comes before that point
   goes on, the rest waits until the secret is whole and followed by something, or turns out not
   to be one ("use my friend" goes on once "friend" shows it is not "my secret phrase").
5. **What is held is bounded** (reviewer's C2): past 64 KB of held text, it goes on anyway,
   redacted as far as it can be; a key still open sends its marker and stays open, each further
   64 KB sending another marker, as the stored copy hides the key to the message's end.
6. **Words and thinking are held apart** (`LiveKind::Words`, `LiveKind::Thinking`), each with its
   own held word and its own context. A piece of one kind sends on what the other kind holds
   first. **Everything of another kind ends the blocks** (`ends_blocks`; the amendment adding the
   "Thinking" and "Starting" signs): a message, a tool call or its result, a notice, a sign, a
   session bound, memory shortened — all send the held text on first, so the screen keeps the
   order. A wait for the AI company (a retry can land mid-word), token counts and plans pass by
   without ending a block. The turn's end (`complete`) sends everything on before its result is
   recorded.
7. **The filter is a trait.** `TextFilter` is now `Arc<dyn TextRedaction>` with `redact`,
   `redact_from`, `hidden` (the marker for a kind), and `stored_secret_start`
   (`crates/runtime/src/agent/tools.rs`); the broker's `Redaction` implements it over Guard's
   redactor under its lock (`crates/capabilities/src/broker.rs`). Call sites that only redact
   (`ai_tools.rs`, `paid/mod.rs`, the desktop's log filter in `orgs.rs` and its diagnostics file in
   `upkeep_commands.rs`) call `.redact(…)`; the runtime's tests use Guard's real redactor
   (`plenipo-guard` is a dev-dependency of `plenipo-runtime`).

## What a person sees

- Words and thinking arrive a word later: the last word of a line appears when the next word
  begins, or when the block ends (a tool call, a message, the end of the turn). Nothing else
  changes on an honest turn.
- A key, a password, or a stored secret the agent writes shows as `[hidden by Plenipo: API key]`
  (or `private key`, `secret setting`, the secret's name) — the same marker the stored copy shows —
  however the AI tool cut the pieces.
- A private key block shows as its marker once it ends; a block that never ends holds the rest of
  that block's words until the block ends, and shows the marker then.

## Known gaps

- **A pattern's value with spaces shows its first words.** For a value the filter knows only by
  its shape (`"apiKey": "two words"`, `PASSWORD="two words"`), each word but the last goes on as
  it is; the marker replaces the value once it is whole. A stored secret with spaces is held from
  its first word (decision 4), so storing such a value hides it whole.
- **A common first word waits a word.** When a stored secret with spaces begins with an everyday
  word ("my"), every "my " at the end of the shown text waits for the next word. One word's delay,
  nothing hidden wrongly.
- **Text without whitespace is held to its line's end** (a long address, text in a language
  written without spaces): it goes on at the next whitespace or newline, at the block's end, or at
  64 KB.
- **The context is one line, 200 characters.** A prefix on an earlier line, or more than 200
  characters back, is not seen; the value then matches only if it has a shape of its own.
- **A key's BEGIN with no END** holds the rest of that block's words (decision 3). Live, that is
  what the stored copy hides anyway.
- **The joined live text equals the stored message** only when the pieces are joined as the
  screen joins them; a screen that drops a piece reads differently, as before.

## Alternatives considered

- **Redact each piece on its own**, as before. Rejected: that is the gap.
- **No live text when a filter is on; show the message at the end.** Rejected: the live chat
  (ADR-200) is the owner's window on the work, and the fix costs one word's delay.
- **Hold a fixed number of characters** instead of the word. Rejected: a secret longer than the
  window still splits, and a shorter one is delayed for nothing.
- **Hold the whole line.** Rejected: the delay is visible on long lines, and a word is enough for
  the filter's shapes; the stored secret and the key block, which can span words and lines, get
  their own rules (decisions 3 and 4).
- **Redact the whole joined text again on each piece and send the difference.** Rejected: a
  marker placed earlier cannot be taken back from the screen; hiding only from where the new text
  starts (decision 2) keeps what was shown fixed.

## How to check

- `crates/runtime/src/agent/live_text.rs` (unit tests, Guard's real redactor as the filter): a
  key split anywhere, a key split character by character equals the whole redaction, a private
  key across pieces and a split or glued BEGIN, a stored secret with spaces held whole, prefix
  patterns kept working by the context and nothing shown twice, words and thinking held apart, the
  flush at a block's end and the key marker at a flush, the 64 KB cap, no filter means no holding,
  and `ends_blocks` for each event kind.
- `crates/guard/src/redact.rs`, `redaction_from_a_point_on`: `redact_from` hides from the point
  on and keeps the earlier bytes, never re-matches a marker, never panics inside a character or
  past the end; `stored_secret_start` finds a beginning and ignores a whole value with text after
  it.
- `crates/runtime/tests/agents.rs`, `a_key_split_across_live_pieces_never_shows`: the fake agent
  streams an answer holding a stored key in 16-byte pieces; no live piece shows eight or more
  characters of the key, the recorded message is redacted, and the live pieces joined equal the
  recorded message.

Ran on the owner's PC on 2026-10-04: `cargo fmt --all -- --check`, `cargo clippy --workspace
--all-targets --locked -- -D warnings`, `cargo test --locked -p` for `plenipo-guard`,
`plenipo-capabilities` and `plenipo-runtime`. `cargo test --workspace --locked` and the real-app
tests: not run (CI).
