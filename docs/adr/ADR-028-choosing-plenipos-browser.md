# ADR-028: Choosing Plenipo's browser

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 10 (follow-up)
- **Amends:** ADR-020 (Plenipo's browser and computer use), section 2

## Context

ADR-020 section 2 says Plenipo uses the Microsoft Edge or Google Chrome already on the computer,
Edge first, and `PLENIPO_BROWSER` can name another Chromium browser. There is no setting: a
computer with both always gets Edge.

The owner asked for Chrome instead of Edge, or a choice in Settings, and picked a choice in
Settings. It came up because the end-to-end browser tests kept failing on GitHub's Linux test
computers: Edge there often takes close to 30 seconds to start, which is the limit Plenipo gives
a browser. That is a test computer problem. On Windows, Edge starts in a few seconds.

## Decision

### 1. A Browser menu in Settings

**Settings → Permissions → Websites → Plenipo's browser** gets a **Browser** menu:

| Choice (on screen)                                      | What Plenipo starts                                           |
| ------------------------------------------------------- | ------------------------------------------------------------- |
| Automatic (Microsoft Edge, or Google Chrome without it) | Edge; Chrome when Edge is not installed (as before)           |
| Microsoft Edge                                          | Edge only                                                     |
| Google Chrome                                           | Chrome only (on Linux, Chromium when Chrome is not installed) |

- Only browsers on this computer can be chosen. The others show "(not installed)".
- The choice is kept in Guard's settings (`browserChoice` in the Ledger setting `guard`). Each
  change is recorded as `guard.browser_chosen` and shows in the Activity trail as "Plenipo's
  browser set to …".
- Settings stored before this record have no choice and read as **Automatic**. Nothing changes
  for an owner who never opens the menu.

### 2. Each browser has its own profile folder

- Edge, and a browser named by `PLENIPO_BROWSER`, keep Plenipo's usual folder,
  `browser-profile`. So an owner on Edge keeps every sign-in.
- Chrome gets `browser-profile-chrome`, next to it.
- Two browsers never share a folder. Edge and Chrome protect saved sign-ins with their own keys,
  so a shared folder would lose sign-ins anyway, and can damage the profile.
- After switching, the owner signs in to their websites again in the new browser. Settings says
  so.
- Everything ADR-020 says about the profile applies to both folders: Plenipo's own, never the
  owner's; it never saves passwords, addresses, or cards; no sync; no extensions.

### 3. A new choice is used from the browser's next start

- A browser that is open stays open, so no worker loses the page it is on.
- Settings says which browser comes next ("Plenipo switches to Google Chrome the next time its
  browser starts. Close its window to switch now.").

### 4. What is not chosen here

- `PLENIPO_BROWSER` still wins over the menu (tests, and other Chromium browsers). The menu is
  then turned off and says why.
- If the chosen browser is not installed, workers cannot use websites, and Settings says so in
  plain words, for example "Google Chrome was not found on this computer, so workers cannot use
  websites. Choose Automatic or Microsoft Edge, or install Chrome." Plenipo never switches to the
  other browser on its own, because the owner chose.

### 5. Test computers only: more time to start

- `PLENIPO_BROWSER_START_SECONDS` (5 to 300) lets a slow test computer give the browser longer
  to start.
- The CI end-to-end job sets it to 90 seconds and chooses Google Chrome in the new menu, which
  also tests the menu.
- On the owner's computer the limit stays 30 seconds. A browser that takes longer there has a
  problem the owner should see.

## Consequences

- The owner can pick Chrome, and nothing changes until they do.
- Switching browsers means signing in to websites again in the new one.
- A computer that has Chrome but no Edge (rare on Windows 11) used Chrome in the usual folder
  before this record. It now uses `browser-profile-chrome`, so the owner signs in to websites
  once more.
- The end-to-end browser tests get 90 seconds to start the browser, which removes the failures
  seen on PR 23. Chrome is also faster there: on PR 32's first green run, the browser test that
  starts it took 23 seconds with Chrome, against 40 to 52 seconds with Edge (or a failure).

## Alternatives considered

- **Always prefer Chrome.** Every Windows PC has Edge, but not every one has Chrome. The owner
  preferred a choice.
- **One profile folder for both browsers.** Rejected: see section 2.
- **Firefox, Brave, or Opera in the menu.** Plenipo controls its browser through the Chrome
  DevTools Protocol, which Firefox does not fully support. Brave and Opera are Chromium browsers
  and can be named with `PLENIPO_BROWSER`, but they are not checked, so the menu does not offer
  them.
- **A longer start limit for everyone.** Rejected: see section 5.
