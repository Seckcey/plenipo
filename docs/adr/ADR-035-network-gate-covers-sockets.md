# ADR-035: The network gate covers beacons, sends on the page's own, and live connections

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 10 (follow-up)
- **Amends:** [ADR-020 (Plenipo's browser and computer use)](ADR-020-browser-and-computer-use.md),
  section 4 (what always waits for the owner) and its known limits

## Context

Plenipo promises that sending anything from a website waits for the owner's approval. ADR-020
keeps that promise in two ways: it reads the control a worker is about to click or press and
asks first when it looks like sending, buying, or signing in; and its network gate holds what
the page sends right after the action, until the owner decides.

A review of the gate found four gaps between the promise and the code:

1. **Enter in a text box asked only inside a form.** Many websites (chats, comment boxes,
   messaging apps) use a text box with no form around it: a `contenteditable` or a
   `role=textbox`. Enter there sends the message through the page's own script, and Plenipo
   did not count it as sending.
2. **The gate watched only pages, forms, and script requests** (`Document`, `XHR`, `Fetch`). A
   beacon (`navigator.sendBeacon`, which pages use to send data as they go) and other request
   kinds went by unseen.
3. **A script's send outside the watch window went ahead.** Plenipo watches a page for about a
   second after a worker's action. A page that sends later on its own (a timer, an autosave, a
   "send after a pause") sent freely, with no approval. Only a form the page submitted by
   itself was stopped.
4. **Live connections were invisible.** A page with a WebSocket (a connection that stays open
   and sends as you type or click) sends its frames outside any request the gate can hold. A
   harmless-looking button on such a page could send a message with no approval.

## Decision

1. **Enter in any text box asks.** A single-line field, a `contenteditable`, or a
   `role=textbox` control, inside a form or not: Enter sends what it holds, so it asks the
   owner as "Sending or publishing outside this computer" (or as buying or signing in when the
   form looks like that). Enter in a multi-line box (a textarea) stays a new line. Space keeps
   its behavior (it presses a focused button).
2. **The gate holds beacons and other request kinds too.** It intercepts `Document`, `XHR`,
   `Fetch`, `Ping`, and `Other` requests. (Chromium's filter refuses `EventSource`, which only
   receives, and `WebSocket`, covered by point 4.) During a worker's action, any of them that
   is not a plain read (GET, HEAD, OPTIONS) is held until the owner approves.
3. **What a page sends on its own is stopped, and the worker is told.** A send that arrives
   while no worker action runs (a form the page submits by itself, a script's POST on a timer,
   a beacon) is stopped, and the worker's next result says: "The page tried to send data to
   {site} on its own, outside your action; Plenipo stopped it. To send it, act on the page
   (click or press a key): the owner is asked then." That note also comes with `browser_read`.
   It is stopped rather than held because nobody could decide it: an approval belongs to a
   worker's tool call, and a request held between calls would wait for an action it had
   nothing to do with, and would be described to the owner as that action's.
4. **A page with a live connection asks before any click, Enter, or Space.** The tab listens
   for the browser's "a WebSocket opened" event (and forgets it on the next page). When the
   worker is about to click, press Enter, or press Space on such a page, and the control does
   not already look sensitive, the action asks the owner as sending, with the reason "this page
   has a live connection (a WebSocket) that sends as you type or click". Typing alone does not
   ask.

## Consequences

- The owner sees more approvals on chat-like websites: every Enter in a composer, and every
  click on a page with a live connection. That is the promise: nothing is sent without asking.
- Pages that send on their own (autosave, analytics beacons, a "typing" signal) have those
  sends stopped while a worker uses them. The worker is told and can act on the page to send
  with approval. Some such pages may show an error or wait; the owner can take over the tab and
  use it freely, as before.
- Plenipo still cannot see inside a live connection. Once the owner approves a click or Enter
  on a live page, whatever the page sends through the connection after it goes unseen. The
  approval card says why it asks.
- Plain page loads (a link that changes something with a GET) stay a known limit of ADR-020.

## Alternatives considered

- **Holding sends outside an action until the worker's next action or a short timer.** Not
  chosen: the approval would name an action the send had nothing to do with, a request held
  between calls could wait for minutes, and a timer would need the tab to raise approvals with
  no tool call behind them. Stopping is simpler, and the worker knows what to do.
- **Holding WebSocket handshakes.** The browser's request filter does not intercept them, and
  holding the handshake would only delay the page's connection, not the frames sent later.
  Asking before the action covers the frames.
- **Asking on every keystroke on a live page.** Too many approvals for what is typed into a
  box; Enter and clicks are where a page sends.
