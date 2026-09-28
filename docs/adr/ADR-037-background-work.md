# ADR-037: Background work — Plenipo lives in the tray, and the window comes and goes

- **Status:** Accepted (by the owner, 2026-09-27, as recommended: option D)
- **Date:** 2026-09-27
- **Phase:** 13
- **Number:** ADR-034 and ADR-035 are taken by pull request #69 (the security fixes) and
  ADR-036 by pull request #71 (every AI model), so this is ADR-037. The owner approved it as
  "ADR-035" before the renumbering.

## Context

Phase 13 of `ROLLOUT_PLAN.md` asks for a "background service/daemon" and says: "Separate UI
lifecycle from long-running task lifecycle. Closing the main window must not accidentally kill
authorized long-running work unless the user configured that behavior." It also asks for defined
recovery after a window crash, a background crash, a Windows restart, an AI tool's crash, an
unfinished task, and an interrupted database migration.

How Plenipo runs today (1.8.0):

- **One Plenipo program** (`Plenipo.exe`, running as you, never as administrator) holds the
  Ledger, Guard, the AI tool programs it starts, your terminals, and Plenipo's browser.
- **The window's page runs in its own programs.** Windows draws Plenipo's window with WebView2
  (the Microsoft Edge engine), which runs the page in separate `msedgewebview2.exe` programs. If
  the page crashes, the Plenipo program keeps running.
- **Closing the window while work runs hides Plenipo to the tray.** With nothing running,
  closing the window quits.
- **AI tool programs end with Plenipo.** They are kept in a Windows "job" (a group of programs
  that ends together), so a crash never leaves an AI tool working unseen.
- Nothing starts Plenipo when you sign in, a second launch starts a second Plenipo, and nothing
  tells you afterwards what a crash or a restart stopped.

Three facts limit the choice:

1. **Your AI tools are signed in under your Windows user** (their sign-ins live in your user
   folder), and **your secrets live in your Windows Credential Manager**, which only programs
   running as you can open.
2. **Plenipo never runs as administrator**, and its installer is per user (it needs no
   administrator either).
3. **Workers use your screen** (Plenipo's browser, and the mouse and keyboard when you allow it),
   which only a program in your own sign-in can reach.

## Decision

**Recommended: option D below.** The Plenipo program is the background part. It lives in the
tray, and the window is one part of it that can close, crash, and open again without touching
the work.

1. **One Plenipo per Windows user, living in the tray.** The Plenipo program keeps the Ledger and
   all the work. The window is only a view of it.
2. **Closing the window never stops approved work, unless you choose that.** A new setting,
   **Settings → Start and close → When I close the window**:
   - **Keep Plenipo in the tray while work is going** (the starting choice, and how 1.8.0
     works): with work going, the window hides and the work goes on; with nothing going, Plenipo
     quits.
   - **Always keep Plenipo in the tray**: closing only hides the window. **Quit Plenipo** in the
     tray menu ends it.
   - **Quit Plenipo and stop its work**: closing the window is the same as **Quit Plenipo**.
     Work stops cleanly and is recorded as stopped.
3. **A crashed window is reopened.** While the window is open, its page tells the Plenipo
   program every few seconds that it is alive. If it goes quiet (a WebView2 program ended, or
   the page froze), Plenipo reloads the page, and if that does not bring it back, closes the
   window and opens a new one. It records this in the Ledger and says: "The window stopped
   unexpectedly at 3:14 PM and was reopened. Your work kept running." Your terminals end with the
   page that showed them, as they already do when the page reloads (ADR-031, the terminal
   panel). (WebView2's own "crashed" signal would need low-level code the project does not
   allow, so the page's "I'm alive" signal is used instead.)
4. **Start with Windows** is a switch in **Settings → Start and close**, off until you turn it
   on. On, Windows starts Plenipo in the tray when you sign in, with no window. It uses your own
   "Run at sign-in" list (`HKEY_CURRENT_USER`), so no administrator is needed, and Windows'
   **Settings → Apps → Startup** page can turn it off too.
5. **One Plenipo at a time.** Opening Plenipo again (Start menu, shortcut) shows the Plenipo
   that is already running and does not start a second one.
6. **If the Plenipo program itself crashes or is ended**, its AI tool programs end with it (as
   today; nothing works unseen). The next time Plenipo starts (you open it, or Windows starts it
   at sign-in), it says in plain words what happened and which tasks were stopped, and offers
   **Run again** or **Leave stopped** for each. **Nothing runs again until you say so**: a task
   that was stopped halfway may have done part of its work, and only you can judge whether to
   run it again. Plenipo does not restart itself after a crash: Windows' "restart after a crash"
   registration needs low-level code the project does not allow (`unsafe_code = "forbid"`), and
   a restart helper that watches Plenipo could restart it in a loop when you meant to end it.
7. **When Windows restarts, shuts down, or signs you out** while Plenipo runs, Windows closes
   Plenipo, as it closes every program, and does not wait for its work to stop the normal way.
   Plenipo notes it in its "running" note as it is closed; the work that was going stops with it.
   When you sign in again it starts (if **Start with Windows** is on, or when you open it) and
   tells you: "Windows closed Plenipo at 3:04 AM (a restart, a shutdown, or signing out)", with
   the task that was running and **Run again**.
8. **Plenipo tells a crash from Windows closing it** with a small "Plenipo is running" note it
   keeps in its own folder (the version, when it started, a heartbeat every 30 seconds, and the
   step it is on; never task text or secrets). If the note is still there at the next start, the
   last run did not end cleanly; if Plenipo noted that Windows was closing it, or Windows started
   after the last heartbeat (a restart or a power cut), it was Windows; otherwise a crash.

**Deviation from the plan:** there is no separate service or daemon program. The plan's
"background service/daemon" is the Plenipo program in the tray, and "separate UI lifecycle from
task lifecycle" is the window coming and going while that program keeps the work.

## Consequences

- **Everything that works today keeps working.** AI tool sign-ins, Credential Manager, Plenipo's
  browser, the screen, and Guard all run as you, exactly as before. No administrator, no new
  install step, and nothing new listening for connections on the PC.
- **A window crash cannot stop work.** The page already runs in WebView2's own programs; this
  decision adds noticing the crash, reopening the window, and saying so.
- **A crash in the Plenipo program itself still stops the work.** That is also true of any
  design: in option C below, a crash of the background program stops the work just the same.
  What this decision adds is that Plenipo tells you what stopped, when it starts again, and lets
  you run it again.
- **The window's memory is not given back while Plenipo is in the tray.** Closing hides the
  window rather than destroying it, so your terminals and the page you were on are kept.
- **If crashes in the Plenipo program become a real problem**, option C can still be built later
  on top of this: the tray, the single instance, Start with Windows, and the recovery messages
  all stay.

## Alternatives considered

- **A. A Windows Service running as SYSTEM.** The usual "background service". Rejected: SYSTEM is
  not you, so it cannot use your AI tools' sign-ins or open your Credential Manager; it runs
  where there is no screen, so Plenipo's browser and the mouse and keyboard cannot work;
  installing a service needs administrator, and Plenipo never runs as administrator; and a
  program running as SYSTEM that takes orders from a window would be a new, very powerful target
  on the PC.
- **B. A Windows Service running as your Windows account.** Fixes the sign-ins, but installing it
  still needs administrator, Windows must keep your Windows password to start it (which does not
  work with a PIN, Windows Hello, or a Microsoft account sign-in the usual way), and it still
  runs where there is no screen. Rejected.
- **C. A second Plenipo program for the work, started at sign-in, with the window as a separate
  program.** The textbook "daemon": a background program running as you holds the Ledger and the
  work, and the window program talks to it through a private channel on the PC. It keeps the
  sign-ins and Credential Manager. Not recommended now:
  - every one of the window's 100-plus commands, and every live update (the Ledger's events,
    terminal output), would move to a new private channel that must be locked to you and
    checked, which is a new way in that Guard does not cover today;
  - two programs must be installed, updated, and kept at the same version;
  - it is weeks of rework across the whole app, with the risk that brings;
  - and a crash of the background program still stops the work, which is the case the owner
    most needs covered.
- **D. The Plenipo program in the tray, with the window coming and going.** Chosen, above.
