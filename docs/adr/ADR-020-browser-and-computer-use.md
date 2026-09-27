# ADR-020: Plenipo's browser and computer use, through Guard

- **Status:** Accepted (by the owner, 2026-09-27)
- **Amended by:** [ADR-023 (on/off switches in Settings)](ADR-023-settings-switches.md): with
  its switches on, sending, buying, and signing in go ahead without asking on allowed websites,
  and a CAPTCHA can be handed to the owner (sections 4, 5, and 6)
- **Date:** 2026-09-27
- **Phase:** 10

## Context

Phase 10 of the rollout plan adds controlled work on websites and on the desktop "when
structured integrations are unavailable": a managed browser, browser automation, a screenshot
and vision pipeline, watching and controlling the computer, a website and application policy,
a sign that a worker is in control, and an emergency stop. The plan puts integrations in this
order: official API, then command-line tool, then browser automation, then computer use.

The owner set these rules for Phase 10:

- Submitting a form, buying anything, logging in, or sending anything always waits for the
  owner's approval.
- Every significant action goes in the Activity trail, with screenshots.
- Never bypass CAPTCHAs or site security, never collect passwords, and never type a secret the
  worker can see. If a login is needed, the owner does it.
- Flag website terms-of-service risks.

Workers already use Plenipo's tools only through Guard and the capability broker (ADR-013):
per-step grants, an MCP tool server reached through Plenipo's relay, approvals, and the Ledger.

## Decision

### 1. Four capabilities, in the plan's order

| Capability         | On screen                  | Tools                                                                                                          |
| ------------------ | -------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `browser.navigate` | Visit websites             | `browser_open`, `browser_read`, `browser_screenshot`, `browser_scroll`, `browser_back`                         |
| `browser.automate` | Use websites               | `browser_click`, `browser_type`, `browser_press`, `browser_select`                                             |
| `computer.observe` | See the screen             | `screen_view`                                                                                                  |
| `computer.control` | Use the mouse and keyboard | `screen_take_control`, `screen_click`, `screen_type`, `screen_keys`, `screen_scroll`, `screen_release_control` |

- New built-in permission sets: **Researcher** (visit websites), **Web assistant** (visit and
  use websites), and **Screen, mouse, and keyboard** (see the screen, use the mouse and
  keyboard; `computer-use` in the code). No built-in role gets it. The owner gives it to a role of their own when nothing else
  will do.
- Workers are told the order: official connections and programs first, the browser for
  screens those don't cover, and the mouse and keyboard last. The tool descriptions and role
  instructions say the same (ADR-019).

### 2. Plenipo's browser

- Plenipo uses **Microsoft Edge or Google Chrome** already on the computer (`PLENIPO_BROWSER`
  can name another Chromium browser). It installs nothing.
- It starts the browser as a supervised program with **its own profile folder** in Plenipo's
  data folder, never the owner's profile, cookies, sign-ins, or saved passwords. That profile
  never offers to save passwords, addresses, or cards. It has no sync and no extensions, and it
  shows the browser's own "controlled by automated software" bar.
- The browser is **always visible**. There is no hidden control.
- Plenipo controls it through the **Chrome DevTools Protocol**, on a random port on this
  computer only. Its page helpers run in an isolated world that the website's scripts cannot
  see or call.
- Each worker's step gets **its own tab**, which closes when the step ends (unless the owner
  took it over).
- The owner can open Plenipo's browser from **Settings → Permissions → Websites** to sign in to
  a website. Workers then use that sign-in until it expires.

### 3. Website lists (the plan's "domain policy")

**Settings → Permissions → Websites** has an **Allowed** list (open without asking), a
**Blocked** list (never open), and **Other websites**, which is **Ask me** (the default) or
**Blocked**.

- An entry is a website (it covers its subdomains), an IP address, or `localhost`, with an
  optional port.
- Addresses on this computer or the local network open only when the Allowed list names them.
- Every page the tab loads is checked, including redirects. Blocked ones are stopped before
  they load.
- A website on neither list asks the first time, then stays open to that worker for the rest
  of its step.
- **Blocked to start with**, because their terms forbid automated use: linkedin.com,
  facebook.com, instagram.com, x.com, twitter.com, tiktok.com, and amazon.com. The Websites
  section warns: check a website's terms before you allow it.

### 4. What always waits for the owner

- **Clicks and key presses** are classified before they happen. Plenipo reads the control's
  words, its link, and its form, and asks the owner for anything that:
  - submits a form (**sending**),
  - looks like buying or paying, or leads to a checkout (**buying**),
  - signs in or sends a password form (**signing in**, a new sensitive kind),
  - looks like sending or changing something ("Send", "Post", "Delete", …).
- **What the page sends** is held too. While a worker's action runs, any request that is not a
  plain page read (a form post, or a script's POST, PUT, or DELETE) is held until the owner
  answers. A form the page sends by itself, with no worker action, is stopped.
- These sensitive kinds can be set to **Ask** or **Blocked**, never allowed without asking.
- The approval card shows exactly what will happen, the page's address, and a **screenshot of
  the page** as it is when the worker asks.

### 5. Passwords, secrets, CAPTCHAs

- Workers **never type into password, one-time-code, or card fields**. The tool refuses.
- They **never type text that contains a secret** from the Vault. Secrets are hidden from
  workers, so they cannot type one anyway.
- **Sign-ins are the owner's:** the worker stops, and the owner signs in in Plenipo's browser.
- **CAPTCHA widgets are recognized**, and clicking or typing into them is refused. The worker
  is told to stop and say so. Plenipo never solves, avoids, or works around a CAPTCHA or other
  site security.
- Page text given to the worker is marked as the website's content, never as instructions.

### 6. Screenshots and vision

- **Every significant action** (open, click, type, press, choose, back, and every screen
  action) takes a screenshot afterwards, and every approval request takes one before.
- Each screenshot is kept as a **Ledger artifact** (type `screenshot`, with its SHA-256) and
  linked from the `capability.used` or approval event.
- The Activity trail and approval cards show them (`get_screenshot`).
- The worker gets each screenshot as an **image** (MCP image content), plus a **description in
  words** for models that cannot see images.

### 7. Computer use (the last resort)

- `screen_view` returns a screenshot of the screen.
  - Windows: `xcap`.
  - Linux: X11.
- A worker must first call `screen_take_control` with **its reason**, and the owner is
  **always asked**, every time (**taking control of your mouse and keyboard**, a new sensitive
  kind).
- Then it can click, type, press keys, and scroll, at coordinates on its last screenshot.
  Input goes through `enigo` (SendInput on Windows).
- **Enter asks again** before it sends anything. The Windows key is refused, and so is text
  containing a secret.
- While a worker holds the mouse and keyboard, a **small window stays above all others** with
  its name, **Take over**, and **Stop**.

### 8. The sign, Stop, and Take over

- **The sign:** whenever a worker uses the browser or the desktop, every page of the app shows
  a banner naming it, with its page and last action. The footer and the tray menu say the same.
  In Plenipo's browser, the worker's page has a colored frame and a label: blue "{worker} is
  using this browser for Plenipo" with **Take over**, green "You have control" after a take
  over, and red "Stopped by you" after a stop. The page's own scripts cannot see or change the
  label.
- **Emergency Stop:** **Stop all** in the banner, **Stop all browser and desktop control** in
  the Windows system tray, and **Stop** in the desktop window. It:
  - halts every session at once (tabs are released, and held mouse buttons and keys are let
    go);
  - ends those workers' permissions for the rest of their step, refusing what they were
    waiting for;
  - keeps any worker from taking control until the owner presses **Allow again**.
- **Take over:** the owner presses **Take over**, or simply clicks or types in the worker's
  page (on the desktop, moves the mouse).
  - That worker stops. Its next call is refused, and what it was waiting for is refused.
  - The tab stays open for the owner.
- Every change is recorded: `control.started`, `control.taken_over`, `control.stopped`,
  `control.allowed`, and `control.ended`.

### 9. Time limits and crashes

- A page gets 30 seconds to load; the worker may ask for 5–120 seconds.
- Each command to the browser gets 20 seconds.
- If the browser dies, the worker's next call says its tab is gone, and the next page it opens
  starts the browser again (`browser.started`, restarted).

### 10. No browser extension

The plan lists an optional supported browser extension. It is **not built**. The DevTools
Protocol gives everything Phase 10 needs without installing anything into a browser, and an
extension in the owner's own browser would reach the owner's sign-ins, which this design keeps
separate.

## Consequences

- Work on websites without an API is possible, and every step is visible, recorded with a
  picture, and stoppable at once.
- The owner answers more approvals: every submit, purchase, sign-in, and send.

**Known limits** (said plainly to the owner):

- **Plain page loads are not held.** A link that changes something through an ordinary page
  load (a badly built "delete" link) is not held by the network gate. It is caught only when
  its words look like sending or changing something.
- **Only requests during a worker's action are held.** Requests a page makes on its own
  between actions (background syncing) are not held. Forms it sends by itself are stopped.
- **Website lists go by name.** An allowed website whose name points at a local address (DNS
  rebinding) is not detected.
- **Frames inside a page are not checked.** Content an allowed page embeds from another site
  (a map, a video) loads with it; workers read and act only on the page itself.
- **The browser's control port is reachable only from this computer**, while the browser runs.
  Another program running as the owner could connect to it. This is the same trust boundary as
  the owner's account.
- **On the desktop, only the owner's mouse is noticed**, not their keyboard. Use **Take over**
  or **Stop**.
- **Screenshots cannot be blurred.** They show whatever was on the page or screen, and stay in
  Plenipo's data folder on this computer.
- **Images for Codex and Grok are unverified.** Whether Codex and Grok pass MCP images to their
  models is not yet checked with the real tools. Every screenshot comes with a description in
  words.
- **Ollama cannot use Plenipo's tools** (ADR-017), so a worker on Ollama has no browser. It is
  told so, with the reason, and the owner can pick another AI tool for its role.
- **Terms of use:** many websites forbid automated use in their terms, and Plenipo cannot
  judge a site's terms. The starting blocked list is not complete; the owner must check each
  site before allowing it. Google and Bing forbid automated searches, so searching should use
  an official search API (a later integration, following the plan's order).

## Alternatives considered

- **A browser extension in the owner's browser.** Rejected: it would act with the owner's own
  sign-ins and history, and needs installing and updating in every browser.
- **Playwright or Puppeteer.** Rejected: they would bundle Node.js and browser downloads. The
  DevTools Protocol from Rust is smaller and uses the browser already on the computer.
- **A headless browser.** Rejected: hidden browser control is out of scope, and the owner must
  be able to see and take over. Headless is used only by the automated tests.
- **Allowing submits on allowed websites.** Rejected: the owner's rule is that submitting,
  buying, signing in, and sending always wait for approval.
