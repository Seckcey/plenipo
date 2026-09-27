# Choosing Plenipo's browser — Checklist

**Status:** built and tested with a real Chromium on Linux, and with Google Chrome on the CI
runner; the owner's Windows check is below. ADR-028 (choosing Plenipo's browser) was
**accepted** by the owner on 2026-09-27.

While PR 23 (Kimi) waited for its end-to-end tests, the owner asked: "Can we have plenipo use
chrome instead of edge? Or let users choose a browser in their settings?" They picked **a choice
in Settings**. This is not in the rollout plan. It is recorded in **ADR-028 (choosing Plenipo's
browser)**, which amends ADR-020 (Plenipo's browser and computer use).

## Guard settings

- [x] `BrowserChoice`: Automatic (the default, as before), Edge, or Chrome; kept as
      `browserChoice` in the Ledger setting `guard`; settings stored before read as Automatic
- [x] `Guard::set_browser_choice`, recorded as `guard.browser_chosen` only when the choice changes

## Plenipo's browser (`crates/capabilities/src/browser/`)

- [x] Finds Edge and Chrome, Edge first (Windows: Program Files and the user's own folder;
      Linux: `microsoft-edge`, `google-chrome`, and Chromium for Chrome; macOS: the apps)
- [x] Starts the chosen browser: Automatic takes Edge, then Chrome; Edge or Chrome takes only
      that one, and never the other on its own
- [x] Each browser has its own profile folder: Edge (and `PLENIPO_BROWSER`) `browser-profile`,
      Chrome `browser-profile-chrome`
- [x] A new choice is used from the next start; an open browser stays open, and Settings names
      the next one
- [x] `PLENIPO_BROWSER` still wins; Settings turns the menu off and says why
- [x] A missing browser is named in plain words, with what to do
- [x] `PLENIPO_BROWSER_START_SECONDS` (5 to 300, test computers only); the owner's limit stays
      30 seconds
- [x] The broker reads the choice from Guard before it starts the browser, opens it for the
      owner, or shows its status

## App and screens

- [x] Command `set_browser_choice` (registered, allowed for the main window only, and refusing
      any other value)
- [x] Settings → Permissions → Websites → Plenipo's browser: the **Browser** menu (only
      installed browsers), the note on sign-ins after switching, and the next browser
- [x] Activity trail: "Plenipo's browser set to Google Chrome"

## Tests

- [x] Unit: profile folders, a configured browser winning, the start time setting, plain words
      for a missing browser, and Guard keeping and recording the choice
- [x] IPC: the default, a change to Chrome, and an unknown browser refused
- [x] Screens: choosing Chrome, "(not installed)" options, and the menu turned off by
      `PLENIPO_BROWSER`
- [x] End to end: the browser test chooses Google Chrome in Settings where it is installed (the
      CI runner), then does all its browser work in Chrome; CI gives the browser 90 seconds to
      start
- [x] All pre-push checks from `CLAUDE.md`

## Owner's check on Windows (about 5 minutes)

1. Install the build from this PR's **Windows** check (or the next release).
2. Open **Settings → Permissions → Websites**. Under **Plenipo's browser**, the **Browser** menu
   shows **Automatic**, and the line under it says **Microsoft Edge, with its own profile**.
3. Choose **Google Chrome**. The line now says **Google Chrome, with its own profile**. If Chrome
   is not installed, it shows **(not installed)** and cannot be chosen.
4. Press **Open Plenipo's browser**. Chrome opens with the "controlled by automated test
   software" bar. Close it.
5. Choose **Automatic** again. Press **Open Plenipo's browser**: Edge opens, still signed in to
   any website you signed in to before.
6. **Activity** shows "Plenipo's browser set to Google Chrome" and "… set to Automatic".

## Found on the CI runner

- Chrome starts faster than Edge on GitHub's Linux test computers: the browser test that starts
  it took 23 seconds with Chrome (PR 32), against 40 to 52 seconds with Edge, which sometimes
  went past the old 30-second limit.
