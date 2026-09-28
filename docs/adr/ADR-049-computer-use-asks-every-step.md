# ADR-049: Computer use asks before every click and keystroke

- **Status:** Accepted (by the owner, 2026-09-28)
- **Date:** 2026-09-28
- **Phase:** 10 (follow-up)
- **Amends:** [ADR-020 (Plenipo's browser and computer use)](ADR-020-browser-and-computer-use.md),
  section 7 (computer use)

## Context

Computer use is the last resort: a worker that finds no official connection, no program, and no
web page for a job may ask to use the owner's mouse and keyboard. The owner was asked once, when
the worker took control (ADR-020). After that the worker clicked, typed, and pressed keys on its
own. Plenipo asked again only when the worker's own words for a step (its "purpose") read like
paying, signing in, or sending, or when the step pressed Enter.

On a web page Plenipo can see what a control is and what the page sends, so it can tell a "Pay"
button from a "Next" button (ADR-035). On the desktop it cannot: a screenshot is a picture, and a
click is a point on it. The only account of what a click does was the worker's own description,
and a description is not a check. A worker that called a step "continue" was never stopped,
whatever the point did.

## Decision

1. **Taking control still asks once:** may this worker touch the desktop at all, with its reason
   (ADR-020, unchanged).
2. **Then every click, typing, and key press asks the owner before it runs:** `screen_click`,
   `screen_type`, and `screen_keys`, however the worker describes them. Looking at the screen
   (`screen_view`) and scrolling (`screen_scroll`) do not ask: they change nothing.
3. **The card shows what the owner needs to decide:** the screen as it is now, with a click's
   point marked on it; the worker's own words for the step, in quotes; and, for typing, the text
   itself. Secrets are never typed (ADR-020), so the text on a card is never a secret.
4. **The worker's words still give the headline.** When the purpose reads like paying, signing
   in, or sending, that is the card's sensitive kind and its reason, as before. Otherwise the
   card's kind is "Taking control of your mouse and keyboard", and the reason says that Plenipo
   cannot see what the step does on the screen and quotes the worker.
5. **The same approval path as everything else.** Each step goes through Guard's usual asking:
   the same card, the same Ledger events, the same limits on asking (at most three requests
   waiting, ten new cards a minute). There is no second approval path for the desktop.
6. **An approved card no longer counts against the minute.** The limit of ten new cards a minute
   counts the cards of the last minute that are still unanswered or were refused or expired. An
   owner who approves each step promptly is never slowed down by their own answers; a worker
   whose cards go unanswered or refused still is. The cap of three waiting requests is unchanged.

## Consequences

- Desktop work is slower: each step is a card. That is the point. Computer use is the last
  resort, and the owner sees every step of it before it happens.
- The owner decides from the picture and the worker's words. The picture shows where the click
  lands; the words are the worker's claim about it. The picture is the check.
- While a card waits, the worker still holds the mouse and keyboard, and the owner moving the
  mouse still takes control back (ADR-020): the waiting card is refused and the worker is told.
  The cards are answered in Plenipo's window (the small window above all others offers Stop and
  Take over only), so an owner who wants the step done answers without moving the mouse, from
  the keyboard; an owner who reaches for the mouse has taken over and can do the step
  themselves. Whether the owner's hand while a card waits should count as an answer rather than
  a take-over is a question for a later record.
- Workers are told in the tool descriptions that the owner approves each click and each typing,
  so they plan fewer, well-described steps.
- Tests: the broker's unit tests for the limits on asking
  (`at_most_ten_unanswered_or_refused_cards_a_minute`,
  `approved_cards_do_not_count_against_the_minute`), the screenshot marker
  (`a_point_is_marked_for_the_owner`), and end to end
  (`every_click_and_key_on_the_desktop_asks_the_owner`; the existing
  `computer_use_asks_first_and_never_types_secrets` and `plan_user_takes_control` now approve
  each step).

## Alternatives considered

- **Keep deciding from the worker's words.** The words are the worker's, and a worker can be
  wrong or misled by what it reads on the screen. Plenipo cannot check them against the screen,
  so they cannot be the gate.
- **Read the screen to tell what a point does** (finding buttons and their labels in the
  picture). It would guess; a guess that says "Next" over a "Pay" button is worse than asking.
  It could later shorten the reason on the card, never replace the asking.
- **Ask once per program or per window.** Plenipo cannot tell windows and programs apart on a
  screenshot with any certainty, and one window holds both harmless and costly buttons.
- **A separate approval for the desktop.** One more path to check, record, and cap. Guard's
  existing path already gives the card, the Ledger events, and the limits.
- **Leave the minute limit as it was.** Ten steps a minute is a normal pace for an owner who
  approves each one; counting approved cards would have refused the eleventh step of a worker
  the owner was following closely, which punishes the owner for answering.
