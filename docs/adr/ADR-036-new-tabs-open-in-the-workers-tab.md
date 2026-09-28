# ADR-036: New tabs open in the worker's own tab

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 10 (follow-up)
- **Amends:** [ADR-020 (Plenipo's browser and computer use)](ADR-020-browser-and-computer-use.md),
  section 2 (Plenipo's browser) and section 4 (what always waits for the owner)

## Context

Each worker's step gets its own tab in Plenipo's browser (ADR-020). That tab is where all of
Plenipo's checks live: the page helper that reads the page for the worker, the network gate that
checks every page against the owner's website lists and holds what the page sends (ADR-035), the
watch for live connections, and the sign that says a worker is using the browser.

A page can ask the browser for a second tab: a link with `target="_blank"`, a script calling
`window.open`, or a form aimed at a new window. Until now the browser simply opened it. That new
tab shared the profile's cookies and sign-ins but had none of the checks: no helper, no gate, no
website check, no sign. The worker could not see it either, since its tools work on its own tab.
Whatever that tab loaded or sent went unchecked and unnoticed, and the tab stayed open.

## Decision

1. **A page never gets a second tab.** When a page in a worker's tab opens a new tab, Plenipo
   stops that tab before any of its script runs or any request leaves, and closes it. Then:
   - If it happened during the worker's action (a click, a key press, an option chosen) and the
     new tab's address passes the tab's website check, the worker's own tab goes to that address
     instead, the same way a page the worker opens goes: through the network gate, watched by
     the action. The worker's result says: "The link opened a new tab; Plenipo opened it here
     instead."
   - If the address does not pass the check, the worker's tab stays where it was, and the result
     carries the gate's own words after "The link opened a new tab.": a blocked website never
     loads, and a website on neither list is stopped, with the worker pointed at `browser_open`,
     where the owner is asked.
   - If it happened outside an action (a pop-up on the page's own, from a timer), the new tab is
     just closed, and the worker's next result says: "The page tried to open a new tab on its
     own; Plenipo closed it."
2. **How.** Right after the browser starts, before any tab exists, Plenipo tells the browser to
   attach to every new tab and hold it paused before it runs (`Target.setAutoAttach` with
   `waitForDebuggerOnStart`, told to the browser as a whole: that is where new windows arrive; a
   tab's own session only hears of its frames and workers). Plenipo's own tabs arrive the same
   way and are let run at once; so are new tabs of a tab the owner has (one opened from Settings,
   or a worker's tab the owner took over). A worker's tab that is stopped, or handed to the owner
   to solve a check that a person is using the site (a CAPTCHA), is still the worker's, and its
   page still runs (the owner's own click is what a pop-up needs to get past the browser's rules):
   a new tab its page opens then is closed like one opened on the page's own, and the worker is
   told with its next result. A new tab whose opener is a worker's tab is the page's doing.
   - The paused tab has no address of its own yet, so the address comes from the tab's own event
     about the ask (`Page.windowOpen`), which arrives a moment earlier.
   - A new tab the page cannot reach (a link, a form) is closed while still paused: nothing in it
     ever runs.
   - A new tab the page's script can reach (`window.open` keeps a handle on it) is different: the
     browser holds the page's own script until the new tab runs or closes, and closing it while
     paused leaves the page unable to take clicks (checked on Chromium 141, headless and with a
     window). So Plenipo first tells the new tab to hold every request, lets it run, refuses each
     request it tries, and closes it once it has been quiet for a moment (about 150 ms, 2 seconds
     at most). Its page never loads. What the opener's script sends through the new window goes
     through the opener's own gate, so it is held or stopped like the page's own sends (ADR-035).
   - A browser that cannot be told to attach this way is not used, like one that cannot be told
     to refuse downloads (ADR-037).
3. **Nothing else changes** for the worker's own tab: its helper, its gate, its live-connection
   watch, and its sign are as before. The browser's own pop-up rules stay too: a pop-up with no
   click behind it never appears at all.

## Consequences

- Websites that open things in new tabs still work for workers: the page opens in the worker's
  tab, where Plenipo checks it, and the worker reads it there and can go back with `browser_back`.
- Flows that need two windows at once (a sign-in window that talks back to the page that opened
  it) do not work in a worker's tab: the new window is closed. Signing in is the owner's anyway
  (ADR-020, section 5).
- The owner's own tabs are not affected: a tab opened from Settings, or one the owner took over,
  opens new tabs as any browser does.
- A page that opens a new tab on its own between actions sees it closed; the worker is told and
  can open the address with `browser_open` if it needs it.

**Known limits:**

- For the moment a new tab the page's script can reach is let run before it is closed, the page's
  own script runs on. What it sends through the new window is checked by the opener's gate; what
  else it does with the window is not seen, and the window closes.
- Plenipo keeps refusing that new tab's requests until the browser says the tab is closed (a
  request left waiting when a tab closes would go out); what the browser does after that answer
  is out of Plenipo's sight.
- A form aimed at a new window is not sent: the worker's tab opens the form's address as a plain
  page instead, and the worker is told the link opened a new tab. Sending a form needs the
  owner's approval anyway; a worker that needs such a form sent says so in its answer.

## Alternatives considered

- **The browser's switch that blocks all new windows (`--block-new-web-contents`).** Not chosen:
  it does not stop a link with `target="_blank"` (checked on Chromium 141), and where it does
  stop a `window.open`, the click just does nothing and nobody is told.
- **Letting the new tab run under its own checks (a helper, a gate, and a sign of its own).** Not
  chosen: the worker's tools work on one tab per step, and a second checked tab would need every
  tool to say which tab it means, for a page the worker can read as well in its own tab.
- **Closing every new tab while paused, whether the page can reach it or not.** Not chosen: the
  browser holds the opener's script until the new tab runs or is closed, and closing it while
  paused leaves the worker's page unable to take clicks, so the worker's tab would be lost after
  any `window.open`.
- **Asking the owner about each new tab.** Not chosen: the new tab is a page like any the worker
  opens, and the tab's website check already decides that (a website on neither list asks
  through `browser_open`, as before); a note keeps the worker informed without another approval.
