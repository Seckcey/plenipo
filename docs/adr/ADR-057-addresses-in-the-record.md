# ADR-057: Web addresses in the record keep the page and the names of its fields

- **Status:** Accepted (by the owner, 2026-09-28)
- **Date:** 2026-09-28
- **Phase:** 10 (follow-up; security hardening, Group C)
- **Amends:** [ADR-020 (browser automation and computer use)](ADR-020-browser-and-computer-use.md),
  section 4 ("the approval card shows exactly what will happen, the page's address")

## Context

A web address often carries more than where it goes. What follows `?` or `#` can hold search
terms, a sign-in token, or a session key that lets whoever has it act as the signed-in person.
Plenipo kept every address in full wherever it wrote one down: approval cards (kept in the
Ledger), the record of each tool call a worker made or was refused (`capability.used`,
`guard.denied`), the control center's notes, and the records of the screenshots it keeps. Those
records last for good and are shown in the Activity trail, backed up, and exported. The secrets
filter hides only what it recognizes (stored secrets and common key formats), so most session
keys stayed.

## Decision

1. **Plenipo records a web address as its website and page** (`plenipo_guard::websites::
safe_address`). What follows `?` keeps only the names of its fields (`?to=…&amount=…`), what
   follows `#` shows as `#…`, a page path's own settings (`;jsessionid=…`) keep only their name,
   and a user name or password in the address is dropped. The address is cut as written, never
   rebuilt, and cleaned before secrets are hidden, so a hidden secret cannot end an address
   early.
2. **Where:** approval cards, `capability.used`, `guard.denied`, and `guard.approvals_limited`
   for the browser tools; the control center's notes; screenshot records; and
   `browser.opened_by_owner`. A program's command line or a file's text is kept as it is (the
   owner must see exactly what a program would run).
3. **What does not change:** the worker still reads the page's address with its `?` part (it
   needs it to work), Guard still checks every address in full against the website lists, and
   what an AI tool itself reports (its tool calls and its answers, `agent.*`) is kept as it said
   it, with secrets hidden.
4. **The approval card for opening a page** shows the website, the page, and the names of the
   fields the address carries, not their values. This amends ADR-020 section 4.

## Consequences

- A session key or sign-in token in an address no longer lasts in the Ledger, its backups, or its
  exports through Plenipo's own records.
- The owner decides on an approval card without the values after `?`. The field names still say
  what kind of data the address carries (`?to=…&amount=…` reads differently from `?page=…`), and
  the card's screenshot shows the page. An owner who needs the exact address can open it in
  Plenipo's browser themselves.
- The AI tool's own activity can still hold a full address it repeated; the Activity trail shows
  it as the tool said it.

## Alternatives considered

- **Hide the whole query (`?…`).** Simpler, but the owner loses what kind of data the address
  carries. Rejected.
- **Keep the full address on the live card and clean it only when the approval is answered.**
  The card is written to the Ledger when it is made, so the full address would be kept anyway
  until then, and a crash would keep it for good. Rejected.
- **Clean the AI tools' own activity too.** It needs a second filter in the runtime for tool
  calls only, and a worker's answer can repeat an address in any words. Left for later.
