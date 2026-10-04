# ADR-215: An approved click sends only where the card said

- **Status:** Proposed (the Development Coordinator and the independent security reviewer settled
  the design on 2026-10-04; the owner accepts it at review)
- **Date:** 2026-10-04
- **Phase:** none (security hardening: finding P-BROWSER-1, found by the independent reviewer on
  2026-10-03 while reviewing pull request #179; not in the 2026-10-02 report)
- **Builds on:** [ADR-013 (Guard and the capability broker)](ADR-013-guard-capability-broker.md),
  [ADR-032 (the CAPTCHA checkbox and verdict; a click glides and holds as a person's does)](ADR-032-captcha-checkbox-and-verdict.md),
  [ADR-035 (the network gate holds what a page sends, and covers sockets)](ADR-035-network-gate-covers-sockets.md),
  [ADR-057 (addresses in the record keep the page)](ADR-057-addresses-in-the-record.md). No earlier
  decision is changed.
- **Made by:** 8 West Ventures, LLC, for Plenipo.

## In short

When the owner approves a worker's click on a web page ("click "Send message" on shop.test"),
Plenipo moves the pointer to the button like a person would, presses, holds for a moment, and
lets go. The page sees the pointer coming and the button going down, and a hostile page could use
those moments: re-aim its form at another website the instant the button is pressed, or slide a
different button ("Delete account") under the pointer. Until now the approval released whatever
the page then sent, to any website, and the click landed wherever the pointer was.

Now the card says where a form sends ("click "Send message" on shop.test, sends to shop.test/send"),
and the approval covers only that: a form sent to the place the card named, by the method it
named, and the page's own website for what the button's script sends; and only what the page sends
once the button is actually down, not while the pointer is still on its way. Anything else the
page sends gets its own card, as a send with no approval does today, with a line saying why it is
not covered. And while the button is held, Plenipo asks the page what is under the pointer: if it
is no longer the approved control, the pointer is moved to an empty spot before the button is let
go, so no click lands anywhere, and the worker is told.

Accepting this record means: an honest page that sends to a different website than it shows
(app.example.com → api.example.com, or a third-party form target) now shows one more card, naming
that website; a page that re-aims, swaps, or pre-sends gets nothing under the approval; the Enter
key and typing with "send the form" carry the same binding for the field's form.

## Context

What the code did at `b3c02a0a`:

- `crates/capabilities/src/broker/operate.rs`: `Action::BrowserClick` reads the control's facts
  (its form's method and action among them) and puts "click {what} on {host}" on the card; the
  owner's approval sets `CallContext::approved`. At carry-out, `same_control` reads the control
  again by its reference and refuses if it changed (`classify::changed`); then `tab.click` runs,
  and `decide_held` releases **every** held request when `approved`, whatever its site.
- `crates/capabilities/src/browser/tab.rs`, `click`: the pointer glides in twelve steps of 14 ms,
  the button is pressed, held 70 ms, released. Nothing is read between the glide and the release.
  Every request the page sends while the action runs (`acting`) is held, from the first glide step
  to the end of the settling wait.
- The test pages `/swap` and `/turncoat` (pull request #179) cover a form re-aimed while the owner
  decides — caught by `same_control` — but not one re-aimed on `mousedown`, nor a decoy moved
  under the pointer during the glide.
- The reviewer's first proposal, to turn the page's scripts off for the 70 ms of the press
  (`Emulation.setScriptExecutionDisabled`), was withdrawn: it would also stop every honest
  button whose own script sends the message (most modern pages), and the events are not replayed
  when scripts come back on.

Measured on the owner's PC on 2026-10-04, with the test site and Edge:

- A form re-aimed at a website that is **not on the allowed list** never reaches the network gate:
  the website lists stop the navigation first ("The page tried to open other.test, which is not on
  the owner's allowed websites list, so Plenipo stopped it"). The binding below matters for an
  **allowed** website the card did not name, and for **another page of the same website**.
- While a form submission the gate holds is pending, the page's script does not answer:
  `Runtime.evaluate` in the page times out (20 s). So once something is held, the click must not
  ask the page anything more.
- The browser sends a mouse-move of its own when a page loads under a resting pointer: a page
  whose button sits where the last click was gets `mouseover` as it loads. A form it submits then
  is sent "by itself" and stopped (ADR-035), before any click begins.

## Decision

1. **The card names a form's destination** (B2, B3). For a submit button, for Enter in a field,
   and for typing with "send the form", the card says "sends to {host}{path}" (the form's action
   without its query; the page's host when the form has none). When the form's website differs
   from the page's, both appear on the card, and both are bound.
2. **The approval is bound to what the card named** (`Destinations`, in `operate.rs`). A held
   form submission (a `Document` request) is covered only when its method and its address
   (origin and path, query ignored) are the ones the card named. Anything else the page sends
   (`XHR`, `Fetch`, a beacon) is covered only when it goes to a website the card named: the
   page's own, or the form's.
3. **Only what the click itself sends is covered** (B4). A request is marked `after_press` when
   it is held once the button is down (or the key struck, or the option chosen); one held while
   the pointer is still gliding is never covered by the approval, however well it matches.
4. **What is not covered takes the existing path** for a send with no approval: the owner's
   "sending" rule (blocked: stopped), the "send without asking on allowed websites" switch, or a
   card: "let the page send data to other.test after clicking "Send message"", with the reason
   ending "not covered by your approval of the click: it goes to other.test, which the card did
   not name" — or "the form went to shop.test/delete-account, which the card did not name", or
   "the page sent it before the click landed, while the pointer was still on its way". What the
   owner approved on the first card is released after that decision, not before.
5. **The click is checked under the held button** (B1, `tab.click`). After the glide, the control
   is read again by its reference where the pointer now is: if it changed, or something else lies
   at its middle (`clear` is false), the click is given up before the button goes down. The press
   lands at the re-read's middle. 25 ms into the 70 ms hold, the page is asked what lies under the
   pointer (`__plenipo.under`): if it is not the approved control or part of it, the pointer is
   moved to a point with nothing under it (`__plenipo.safePoint`: a corner or edge of the page
   where `elementFromPoint` finds only the page itself) and the button is let go there. The
   browser then fires its `click` on the common ancestor of the press and the release, never on
   the decoy, so no button is activated. The worker reads: "The click did not happen: the page
   moved another control under the pointer after the check, so the button was let go away from
   it and nothing was clicked." Data the page sent meanwhile is decided as unapproved.
6. **Enter and typing with "send the form"** carry the focused field's form as the click carries
   the button's (the facts Plenipo reads for `same_focus` and `same_control` hold the form).
   Choosing an option binds the page's website only.
7. **A page that sends while the pointer is still on its way, or the moment the button goes
   down, is not asked anything more.** Something is held then, and the page's script would not
   answer (above). The click is given up: before the press, nothing more happens; after the press,
   the pointer is moved to the page's corner (0, 0) and the button let go there, with no call into
   the page. What was held is decided as not covered, and the card says "the page sent it before
   the click landed" or "the click was given up". The worker reads "The click did not happen: the
   page sent data while the pointer was still on its way to the control, so nothing was clicked"
   (or "… the moment the button went down …").
8. **An action that fails outright** (the browser did not answer) **fails everything it set
   off**, rather than leaving a held request with no one to decide it.

## What a person sees

- An approval card for a submit button, Enter in a form, or typing with "send the form" now ends
  with where the form sends: "click "Send message" on shop.test, sends to shop.test/send". When
  that is another website, the card names it: "…, sends to pay.example/checkout".
- On an honest page, nothing else changes — **except** a page whose approved button sends to a
  website the card did not name (its API on another host, a third-party form target). That now
  shows one more card, naming that website, as a send with no approval does today. The owner's
  "send without asking on allowed websites" switch still lets an allowed website through.
- On a hostile page, the worker reads "Not sent: …" or "The click did not happen: …", and the
  owner sees a card only for what the page tried that the first card did not cover; its reason
  ends with why: "Not covered by your approval of clicking "Send message": the form went to
  shop.test/delete-account, which the card did not name", or "… it goes to pay.test, which the
  card did not name", or "… the page sent it before the click landed, while the pointer was still
  on its way".

## Known gaps

- **The same address, other contents.** A page can change a form's hidden fields, or its query,
  between the card and the press; the form still goes where the card said, by the method it said,
  and is covered. The owner approved sending to that address; what the page puts in the form is
  the page's.
- **A decoy that comes and goes.** A control moved under the pointer after the post-glide read
  and moved away again before the 25 ms check gets a `mousedown`, and the release lands on the
  approved control: the browser's `click` then fires on the common ancestor of the two, never on
  the decoy, so nothing is activated; the decoy saw a `mousedown` and nothing more.
- **A page that re-aims on `click`** (after the release) and sends with its own script: that send
  is held after the press, and covered only if it goes to a website the card named; a form it
  submits from script to another address is a `Document` request that does not match and is
  asked about.
- **A link** (`<a href>`) opens a page by `GET`, which is never held; the website lists govern
  where it may go, as before.
- The hold check reads the page once, 25 ms in; a change in the last 45 ms before the release is
  not seen. The browser's common-ancestor rule covers the press-to-release swap (above); a page
  that moves the approved control itself away in that window loses the click.
- **A page that submits its own form on `mousedown`** gets a card, not the approval, even when the
  form is the one the card named: the click is given up before the page can be asked anything
  (decision 7). Honest pages submit on `click`, after the release, which is covered.

## Alternatives considered

- **Turn scripts off for the press** (the reviewer's first layer 2). Withdrawn: honest pages whose
  button's script sends the message would stop working, and the events are not replayed.
- **Bind the approval to the site only.** Rejected: a form re-aimed from `/send` to
  `/delete-account` on the same site would stay covered. The method and the address (origin and
  path) are what the card can show and what the page cannot change without being asked.
- **Cover every request held during the action**, as before. Rejected: a form the page submits on
  `mouseover`, while the pointer is still on its way, would ride on an approval the owner gave for
  a click that had not happened.
- **Refuse any click whose button's form sends to another website.** Rejected: honest pages do
  that (payment providers, form services); the card names it and the owner decides.

## How to check

`crates/capabilities/tests/browser.rs`: `an_approved_click_sends_only_where_the_card_said`
(`/re-aim` to a website that is not allowed, `/re-aim-allowed` to the allowed `pay.test`,
`/re-aim-path` to another page of the same website, `/eager`, `/js-send`, and the honest
cross-website form `/pay`) and `a_control_moved_under_the_pointer_is_not_clicked` (`/decoy`,
`/decoy-late`, `/decoy-press`); the pages are in `tests/support/site.rs`, and the test harness now
allows `pay.test` beside `shop.test`. The earlier
`a_control_that_changed_while_the_owner_decided_is_left_alone` and the rest of the browser tests
still pass. Both new tests ran on the owner's PC (Windows, Edge) on 2026-10-04.
