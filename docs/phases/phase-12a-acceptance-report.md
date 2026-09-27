# Phase 12A — Acceptance Report

|              |                                                                                                                                                                                                                                                     |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 12A — Visual Design System (UniFi-Style Console Aesthetic)                                                                                                                                                                                          |
| **Branch**   | `claude/phase-12`                                                                                                                                                                                                                                   |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings` (no diff), and the full `pnpm e2e` against the release build. GitHub CI: Rust, Frontend, E2E (Linux), Windows — see the pull request.                                      |
| **Date**     | 2026-09-27                                                                                                                                                                                                                                          |
| **Result**   | All three acceptance criteria and all six Phase 12A tests pass. Every existing page runs in the new frame and passes its earlier tests unchanged (one selector updated). Version **1.7.0**. Decision: ADR-030 (first written as ADR-029), accepted. |

Screenshots (from the end-to-end run in the real app):

- The frame: [dark](evidence/phase-12a/frame-dark.png) · [light](evidence/phase-12a/frame-light.png)
  · [the smallest window, 800 × 560](evidence/phase-12a/min-window.png)
- The Gallery, dark: [top](evidence/phase-12a/gallery-dark-top.png) ·
  [cards](evidence/phase-12a/gallery-dark-cards.png) · [table and filters](evidence/phase-12a/gallery-dark-table.png)
  · [detail page](evidence/phase-12a/gallery-dark-split.png) ·
  [map](evidence/phase-12a/gallery-dark-map.png) · [notices and controls](evidence/phase-12a/gallery-dark-notices.png)
  · [tokens](evidence/phase-12a/gallery-dark-tokens.png)
- The Gallery, light: [top](evidence/phase-12a/gallery-light-top.png) ·
  [cards](evidence/phase-12a/gallery-light-cards.png) · [table and filters](evidence/phase-12a/gallery-light-table.png)
  · [detail page](evidence/phase-12a/gallery-light-split.png) ·
  [map](evidence/phase-12a/gallery-light-map.png) · [notices and controls](evidence/phase-12a/gallery-light-notices.png)
  · [tokens](evidence/phase-12a/gallery-light-tokens.png)
- [Both themes side by side](evidence/phase-12a/gallery-both.png) ·
  [5,000 rows](evidence/phase-12a/table-5000.png) ·
  [real departments and projects with Ledger activity](evidence/phase-12a/live-cards.png)
- Every page in both themes: Organization ([dark](evidence/phase-12a/page-organization-dark.png),
  [light](evidence/phase-12a/page-organization-light.png)), Projects
  ([dark](evidence/phase-12a/page-projects-dark.png), [light](evidence/phase-12a/page-projects-light.png)),
  Workers ([dark](evidence/phase-12a/page-workers-dark.png), [light](evidence/phase-12a/page-workers-light.png)),
  Approvals ([dark](evidence/phase-12a/page-approvals-dark.png),
  [light](evidence/phase-12a/page-approvals-light.png)), AI tools
  ([dark](evidence/phase-12a/page-ai-tools-dark.png), [light](evidence/phase-12a/page-ai-tools-light.png)),
  Activity ([dark](evidence/phase-12a/page-activity-dark.png), [light](evidence/phase-12a/page-activity-light.png)),
  Settings ([dark](evidence/phase-12a/page-settings-dark.png), [light](evidence/phase-12a/page-settings-light.png)),
  Diagnostics ([dark](evidence/phase-12a/page-diagnostics-dark.png),
  [light](evidence/phase-12a/page-diagnostics-light.png)).

Test totals: **765 Rust** · **415 frontend** (240 design system + 175 app) · **68 end to end** against the real release binary (section 5).

On screen the plan's words become plain ones ([word list](../design/vocabulary.md)): the scope
selector is **Showing**, the notification badge is the **bell**, banners are **notices**, the
component gallery is the **Gallery**, facets are **Filters**, column customization is
**Columns**, page size is **Rows per page**, and the card/list toggle is **Cards / List**.

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                          | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| --- | ---------------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Every component in the library renders correctly in both themes with real and empty data.            | **Pass** | The Gallery renders every component with sample data, empty, loading, and error (the filters, the detail page, its property list, and its timeline included), in either theme or both side by side (`Gallery.test.tsx`: every section in both themes, every state, plain captions, unique ids). The end-to-end test compares every sample's computed styles in both themes with the checked-in look snapshots, and shows real departments and projects with activity counted from real Ledger events ([screenshot](evidence/phase-12a/live-cards.png)). |
| 2   | Phase 12 screens can be assembled entirely from this library without introducing new one-off styles. | **Pass** | The library covers the frame, cards and grid, table, filters, the detail split view, the map, status, controls, notices, and states (`docs/design/design-system.md`). The Gallery page itself is assembled only from library components. The existing pages now sit in the library's frame and notices and take every color from the tokens; they keep their own layout rules (their buttons, badges, and cards in `apps/desktop/src/styles.css`) until Phase 12 rebuilds each one from the library, as ADR-030 §8 set out.                             |
| 3   | No feature code contains raw color values.                                                           | **Pass** | `pnpm lint` runs ESLint's raw-color rule on every `.ts`/`.tsx` file in `apps/desktop/src` and `packages/ui/src` (only `tokens.ts` holds colors) and `scripts/check-colors.mjs` on every stylesheet; one shared definition (`scripts/colors.mjs`) refuses hex (also `%23…` in a data: image), `rgb()` to `oklch()`, and every CSS named color in styles, style properties, and SVG color attributes. `scripts/colors.test.mjs` proves it on every `pnpm lint`. The app's stylesheet had about 100 raw colors; each now uses a token.                     |

## 2. Required Phase 12A tests → evidence

| Plan test                                                                            | Test                                                                                                                         | What it shows                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| visual regression snapshots of the gallery in dark and light themes                  | E2E `the Gallery matches its look snapshot in both themes`; `tests/e2e/snapshots/gallery-dark.json`, `gallery-light.json`    | Every Gallery sample's computed colors, gradients, sizes, spacing, and borders, in both themes, against the snapshot (colors compared within rounding): status, strips, cards, the filtered table and grid (with rows), the list view, the detail page with its timeline, the map, notices, controls, and every loading, error, and empty state. The frame is also checked not to move when the page jumps to a section. Screenshots are saved for people to look at. Pixel images are not compared: fonts differ between computers (ADR-030 §9). |
| token contrast check: all text/background pairs meet WCAG AA                         | `packages/ui/src/tokens.test.ts` (152 pair checks)                                                                           | Every text token (including links and status words) on every surface; each status word on its own tint; body text on every tint; text on the map, in both themes: 4.5:1. Status marks, control outlines, and map lines: 3:1.                                                                                                                                                                                                                                                                                                                      |
| status is distinguishable without color (dot plus label present in DOM)              | `status.test.tsx` `status without color`; E2E `status reads without color`                                                   | Each status has its mark and its word in the page, and its own mark shape (● ▲ ◆ ○ ◐). In the real app, every status mark on the Gallery has its word.                                                                                                                                                                                                                                                                                                                                                                                            |
| keyboard navigation and focus-visible styling across rail, filters, table, and cards | `shell.test.tsx`, `facets.test.tsx`, `table.test.tsx`, `cards.test.tsx`, `controls.test.tsx`; E2E `works from the keyboard…` | Tab reaches the strip's sections in order and Enter opens them; the search box filters 5,000 rows; Tab alone moves on through a filter group, its checkboxes (Space ticks and unticks), and both range handles (arrow keys move them), out of the filters to the table's Columns button, select all, every sortable header (Enter sorts), and a row's checkbox; each card's title is a stop. In the real app, each shows its focus outline (the card through its outline layer). Tabs move with arrow keys.                                       |
| virtualized table performance with 5,000 rows                                        | `table.test.tsx` `5,000 rows`; E2E `keeps a 5,000-row table responsive`                                                      | Fewer than 80 rows are drawn; scrolling to the end draws row 5,000; sorting 5,000 rows both ways; in the real app, scrolling to the end and sorting each take well under a second.                                                                                                                                                                                                                                                                                                                                                                |
| responsive behavior at the minimum supported window size                             | E2E `fits the smallest window (800 × 560)`                                                                                   | Nothing spills sideways; the top bar's buttons and the strip's names stay on screen; the card grid drops to one or two columns ([screenshot](evidence/phase-12a/min-window.png)).                                                                                                                                                                                                                                                                                                                                                                 |

Also: the Ledger's activity query (`crates/ledger/src/activity.rs`: fixed buckets, a week in 96
buckets with nothing dropped, problems and approvals, every scope following the organization,
refused requests, the time index used), the desktop command over IPC, the frame's own tests in
the app (`App.test.tsx`: light/dark remembered, names under the icons, the bell opens Approvals,
**Showing** opens a project or a department, the Gallery with real data), and the light/dark
choice surviving a restart in the real app.

## 3. The owner's decisions → evidence

| Decision (2026-09-27)                                        | Evidence                                                                                                                    |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| ADR-030 accepted: every screen from the same building blocks | `packages/ui`; the existing pages in the new frame; the raw-color checks.                                                   |
| Names under the icons on the left strip (view options later) | `IconRail`; E2E checks every name is shown, and none is cut off, at 1440 px and at 800 px.                                  |
| Smaller, tighter text: 13 px                                 | `font-md` 13 px is the base (`html`); E2E checks the computed size. Every older font size now maps onto the 11–20 px scale. |
| Settings → Servers works exactly as before                   | Its page, form, and tests are unchanged; the Phase 11 end-to-end test passes in the new frame (section 5).                  |

## 4. Defects found and fixed during Phase 12A

- **A test browser could not read the strip's names.** WebDriver treats text in a clipped box as
  hidden, so the Phase 1 end-to-end test saw empty buttons. The names fit the strip, so they are
  no longer clipped.
- **The strip was too narrow for "Organization"** on Linux fonts; it is now 88 px.
- **A stale token file** was caught by its own drift test, as designed.

After the first push, two reviews ran on the finished work: a code review in five areas and a
look at every screenshot in both themes. Each finding was checked again by a second reviewer
before it was fixed; 53 of 55 code findings and every look finding but a few held up. Fixed:

- **The window could scroll**, pushing the top bar off screen and leaving an empty band under
  the footer. Hidden screen-reader labels were placed against the whole page. The page area and
  the frame now contain them, the window cannot scroll, and the E2E checks the frame stays put.
- **Scroll and pages:** a card grid or table that came back after loading or an error could
  open on a blank area; a filtered table stayed on a later page. Both start at the top now.
- **Filters:** the range slider could get stuck with both handles at the top; the scale labels
  overlapped; ticked options could vanish; the panel scrolled sideways.
- **Activity counts:** a task blocked while it waits for handoff replies was counted as a
  problem, so ordinary delegation turned strips red. It is normal work now. A department no
  longer counts another department's work below it, and interrupted runs count as problems.
  The query reads the Ledger once per request.
- **Not color alone:** strips now use height (work fills half, waiting three quarters, a
  problem the whole bar); timeline events use the status mark shapes; the map's oversight
  switch shows a filled, outlined button when on.
- **Contrast:** checkboxes, radio buttons, and dropdowns drawn by the system (white squares on
  the dark theme) now use the tokens; placeholders, field and search outlines, disabled
  buttons, and map lines reach their contrast (the map line has its own 3:1 check now).
- **Layout:** card footers line up; the property list fits its card; long map names end in
  "…"; notices sit clear of the edges; spacing under headings is even; the table's number
  header lines up with its numbers.
- **The app:** **Showing** lasts for the session and resets when you leave by the strip or
  the bell; choosing the same project again opens it again; rank names follow the chosen title
  set on cards; the sign window follows the light/dark choice.
- **Gaps against the plan:** the filters, the detail page, its property list, and its timeline
  had no loading, error, or empty state; the raw-color lint missed named colors in TypeScript
  and newer color formats; the look snapshots skipped the filters, filled tables, the detail
  page, and notices; the keyboard test jumped between areas by script. All closed (sections 1
  and 2). Test names that promised more than they checked now check it.
- **The keyboard test failed in CI** (it passed alone). The Phase 10 test's browser outlived
  the app, because the test harness stopped the app before Plenipo could close it, and that
  browser window kept the keyboard focus on the test display; a window without focus draws no
  focus outlines. The harness now stops a leftover Plenipo browser after each run, and the
  keyboard test first checks that its window has the focus, with a plain message if not.
- **Numbering:** ADR-029 (workers try a CAPTCHA three times) reached main first, so this
  phase's decision is ADR-030 and the proposed terminal panel is ADR-031. Nothing in them
  changed.

## 5. Test totals

- **765 Rust** tests (Linux, after merging main), including 8 Ledger activity tests and the
  `get_activity` IPC checks (it answers the main window and is refused to every other window and
  to remote pages).
- **415 frontend** tests: **240** in the design system (`packages/ui`: 152 contrast pair checks,
  tokens, status, activity, cards, table, filters, the detail page and map, the frame, controls,
  and the Gallery) and **175** in the app (5 new for the frame and the Gallery). Plus 3 checks of
  the raw-color patterns (`scripts/colors.test.mjs`, run by `pnpm lint`).
- **68 end to end** against the real release binary, in 12 groups: 6 Phase 1, 6 Phase 2, 10
  Phase 3, 5 Phase 4, 5 Phase 5, 5 Phase 6, 4 Phase 7, 4 Phase 8, 6 Phase 10, 3 switches and
  learning, 5 Phase 11, and **9 Phase 12A**.

## 6. Notes

- **The terminal panel** (Phase 12) is proposed in ADR-031 and not built. It needs the owner's OK.
- **The look snapshots are computed styles, not pictures.** They catch a changed color, size,
  spacing, or border in either theme; they do not catch a layout that moved while every style
  stayed the same. The screenshots cover that, for people.
- **Windows:** the build, installer, and launch smoke test run on Windows in CI. The owner's
  Windows check for Phase 11 (a real server) is still to do.
