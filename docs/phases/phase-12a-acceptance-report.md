# Phase 12A — Acceptance Report

|              |                                                                                                                                                                                                                          |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Phase**    | 12A — Visual Design System (UniFi-Style Console Aesthetic)                                                                                                                                                               |
| **Branch**   | `claude/phase-12`                                                                                                                                                                                                        |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings` (no diff), and the full `pnpm e2e` against the release build. GitHub CI: Rust, Frontend, E2E (Linux), Windows — see the pull request.           |
| **Date**     | 2026-09-27                                                                                                                                                                                                               |
| **Result**   | All three acceptance criteria and all six Phase 12A tests pass. Every existing page runs in the new frame and passes its earlier tests unchanged (one selector updated). Version **1.7.0**. Decision: ADR-029, accepted. |

Screenshots (from the end-to-end run in the real app):

- The frame: [dark](evidence/phase-12a/frame-dark.png) · [light](evidence/phase-12a/frame-light.png)
  · [the smallest window, 800 × 560](evidence/phase-12a/min-window.png)
- The Gallery, dark: [top](evidence/phase-12a/gallery-dark-top.png) ·
  [cards](evidence/phase-12a/gallery-dark-cards.png) · [table and filters](evidence/phase-12a/gallery-dark-table.png)
  · [detail split view](evidence/phase-12a/gallery-dark-split.png) ·
  [map](evidence/phase-12a/gallery-dark-map.png) · [notices and controls](evidence/phase-12a/gallery-dark-notices.png)
  · [tokens](evidence/phase-12a/gallery-dark-tokens.png)
- The Gallery, light: [top](evidence/phase-12a/gallery-light-top.png) ·
  [cards](evidence/phase-12a/gallery-light-cards.png) · [table and filters](evidence/phase-12a/gallery-light-table.png)
  · [detail split view](evidence/phase-12a/gallery-light-split.png) ·
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

Test totals: see section 5.

On screen the plan's words become plain ones ([word list](../design/vocabulary.md)): the scope
selector is **Showing**, the notification badge is the **bell**, banners are **notices**, the
component gallery is the **Gallery**, facets are **Filters**, column customization is
**Columns**, page size is **Rows per page**, and the card/list toggle is **Cards / List**.

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                          | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| --- | ---------------------------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Every component in the library renders correctly in both themes with real and empty data.            | **Pass** | The Gallery renders every component with sample data, empty, loading, and error, in either theme or both side by side (`Gallery.test.tsx`: every section in both themes, unique ids). The end-to-end test compares every sample's computed styles in both themes with the checked-in look snapshots, and shows real departments and projects with activity counted from real Ledger events ([screenshot](evidence/phase-12a/live-cards.png)). |
| 2   | Phase 12 screens can be assembled entirely from this library without introducing new one-off styles. | **Pass** | The library covers the frame, cards and grid, table, filters, the detail split view, the map, status, controls, notices, and states (`docs/design/design-system.md`). The Gallery page itself is assembled only from library components. The existing pages now sit in the library's frame and notices.                                                                                                                                       |
| 3   | No feature code contains raw color values.                                                           | **Pass** | `pnpm lint` runs ESLint's raw-color rule on every `.ts`/`.tsx` file in `apps/desktop/src` and `packages/ui/src` (only `tokens.ts` holds colors) and `scripts/check-colors.mjs` on every stylesheet; both were shown to catch `#fff`, `rgba()`, and named colors. The app's stylesheet had about 100 raw colors; each now uses a token.                                                                                                        |

## 2. Required Phase 12A tests → evidence

| Plan test                                                                            | Test                                                                                                                         | What it shows                                                                                                                                                                                                                                                                                                                                            |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| visual regression snapshots of the gallery in dark and light themes                  | E2E `the Gallery matches its look snapshot in both themes`; `tests/e2e/snapshots/gallery-dark.json`, `gallery-light.json`    | Every Gallery sample's computed colors, sizes, spacing, and borders, in both themes, against the snapshot (colors compared within rounding). Screenshots are saved for people to look at. Pixel images are not compared: fonts differ between computers (ADR-029 §9).                                                                                    |
| token contrast check: all text/background pairs meet WCAG AA                         | `packages/ui/src/tokens.test.ts` (137 checks)                                                                                | Every text token (including links and status words) on every surface, tint, and the map, in both themes: 4.5:1; status marks and control outlines: 3:1.                                                                                                                                                                                                  |
| status is distinguishable without color (dot plus label present in DOM)              | `status.test.tsx` `status without color`; E2E `status reads without color`                                                   | Each status has its mark and its word in the page, and its own mark shape (● ▲ ◆ ○ ◐). In the real app, every status mark on the Gallery has its word.                                                                                                                                                                                                   |
| keyboard navigation and focus-visible styling across rail, filters, table, and cards | `shell.test.tsx`, `facets.test.tsx`, `table.test.tsx`, `cards.test.tsx`, `controls.test.tsx`; E2E `works from the keyboard…` | Tab reaches the strip's sections in order and Enter opens them; the search box filters 5,000 rows; Tab reaches the filters' buttons, the Columns button, select all, and each sortable header (Enter sorts); each card's title is a stop. In the real app, each shows its focus outline (the card through its outline layer). Tabs move with arrow keys. |
| virtualized table performance with 5,000 rows                                        | `table.test.tsx` `5,000 rows`; E2E `keeps a 5,000-row table responsive`                                                      | Fewer than 80 rows are drawn; scrolling to the end draws row 5,000; sorting 5,000 rows both ways; in the real app, scrolling to the end and sorting each take well under a second.                                                                                                                                                                       |
| responsive behavior at the minimum supported window size                             | E2E `fits the smallest window (800 × 560)`                                                                                   | Nothing spills sideways; the top bar's buttons and the strip's names stay on screen; the card grid drops to one or two columns ([screenshot](evidence/phase-12a/min-window.png)).                                                                                                                                                                        |

Also: the Ledger's activity query (`crates/ledger/src/activity.rs`: fixed buckets, a week in 96
buckets with nothing dropped, problems and approvals, every scope following the organization,
refused requests, the time index used), the desktop command over IPC, the frame's own tests in
the app (`App.test.tsx`: light/dark remembered, names under the icons, the bell opens Approvals,
**Showing** opens a project or a department, the Gallery with real data), and the light/dark
choice surviving a restart in the real app.

## 3. The owner's decisions → evidence

| Decision (2026-09-27)                                        | Evidence                                                                                                                    |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| ADR-029 accepted: every screen from the same building blocks | `packages/ui`; the existing pages in the new frame; the raw-color checks.                                                   |
| Names under the icons on the left strip (view options later) | `IconRail`; E2E checks every name is shown, and none is cut off, at 1440 px and at 800 px.                                  |
| Smaller, tighter text: 13 px                                 | `font-md` 13 px is the base (`html`); E2E checks the computed size. Every older font size now maps onto the 11–20 px scale. |
| Settings → Servers works exactly as before                   | Its page, form, and tests are unchanged; the Phase 11 end-to-end test passes in the new frame (section 5).                  |

## 4. Defects found and fixed during Phase 12A

- **A test browser could not read the strip's names.** WebDriver treats text in a clipped box as
  hidden, so the Phase 1 end-to-end test saw empty buttons. The names fit the strip, so they are
  no longer clipped.
- **The strip was too narrow for "Organization"** on Linux fonts; it is now 88 px.
- **A stale token file** was caught by its own drift test, as designed.

## 5. Test totals

To be filled in with the final run.

## 6. Notes

- **The terminal panel** (Phase 12) is proposed in ADR-030 and not built. It needs the owner's OK.
- **The look snapshots are computed styles, not pictures.** They catch a changed color, size,
  spacing, or border in either theme; they do not catch a layout that moved while every style
  stayed the same. The screenshots cover that, for people.
- **Windows:** the build, installer, and launch smoke test run on Windows in CI. The owner's
  Windows check for Phase 11 (a real server) is still to do.
