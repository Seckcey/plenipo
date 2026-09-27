# ADR-029: One design system for every screen

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 12A

## Context

Phase 12A asks for one visual language and one shared component library before the Phase 12
screens are built, so that every page reads as one dense, dark operations console (the
reference is the UniFi Network controller, `docs/design/reference/`). Until now each phase
added its own styles to one 3,100-line stylesheet in the desktop app, with about a hundred raw
color values and no light theme.

The owner approved the plan on 2026-09-27: every screen uses the same building blocks, and no
screen gets its own one-off style. The owner also chose **names under the icons** on the left
strip (UniFi shows icons only; "we will add view options later") and **smaller, tighter text**
(13 px instead of 15 px).

## Decision

### 1. One package: `@plenipo/ui`

- A new workspace package, `packages/ui`, holds the design tokens, the component library, and
  their styles. The desktop app imports it; screens compose its components.
- Component class names start with `ui-`. Screen styles in the app may lay out a page, but they
  use only the tokens (section 3).

### 2. Tokens: one TypeScript source

- `packages/ui/src/tokens.ts` is the only place color values, sizes, and timings are written.
  It holds: colors (background, surface, surface-raised, border, text-primary,
  text-secondary, text-muted, accent, and the status ramp ok / warn / error / offline /
  pending, plus the few extra colors the organization map needs), the type scale (11–20 px),
  spacing, radius (4–8 px), elevation, and motion.
- Two themes map the same token names: **dark** (the default) and **light**.
- Generated from it and checked in: `packages/ui/src/generated/tokens.css` (CSS custom
  properties, `--ui-…`) and `tokens.json` (for the Rust side and exports). A test fails when
  either file is out of date; `pnpm tokens` rewrites them.
- Numbers use tabular figures everywhere a metric is shown.

### 3. No raw colors in feature code

- `pnpm lint` fails on a color literal (`#…`, `rgb()`, `hsl()`, or a named color) in the
  desktop app's code or styles. ESLint checks TypeScript; `scripts/check-colors.mjs` checks
  CSS. Only `packages/ui/src/tokens.ts` may hold color values.
- Not covered, on purpose: the marketing website (`apps/website`, out of scope for 12A), and
  the sign Plenipo draws inside web pages (`crates/capabilities/src/browser/page.js`), which runs
  inside other people's websites.

### 4. The frame around every page

- **Left strip:** a button for each section, with its icon and its name under it (owner's
  choice), an active-section marker, and a count badge where a section has one. View options
  (icons only, for example) can come later.
- **Top bar:** where you are (All of the organization, a department, or a project), the page
  title, the light/dark switch, and a bell with the number of requests waiting for you.
- **Notice area** under the top bar for advisories and things you must do, each with its
  button and, where it makes sense, **Dismiss**. The existing notices (the sign while a worker
  uses the browser or a server, approvals waiting, Ledger notices) move there unchanged.
- The theme is remembered on this computer (browser storage in the app window).

### 5. The component library

Built once, used by every screen, each with empty, loading, and error states:

- Status: dot with a word (never color alone), pill, count badge, health bar, sparkline, and
  the 24-hour activity strip with a time axis and a "Now" marker.
- Entity card (a department, project, or worker), and a card grid with a card/list switch.
- Dense table: sortable columns, a status column, tabular numbers, links, row checkboxes,
  choosing columns, page size, and a count of records.
- Filter panel: search, grouped checkboxes with counts, range sliders, **Clear filters**.
- Detail split view: properties on the left, a timeline in the middle, a map on the right, and
  a table below.
- Relationship map: tiles with status color, labeled connectors, and a caption under each tile.
- Plenipo's own simple line icons. No UniFi icons, logos, or other assets.

### 6. Large lists stay fast

- Tables and card grids draw only the rows and cards on screen (a small windowing helper in
  `packages/ui`, no new dependency), so 5,000 rows and hundreds of cards scroll smoothly.

### 7. Activity strips come from the Ledger

- A Ledger query counts events in fixed time buckets for a scope: everything, a department (its
  projects and every position under its head), a project, or a position.
- **Downsampling:** a strip always has the same number of buckets (96 for 24 hours: 15 minutes
  each). A longer range makes each bucket wider; counts are added up, so nothing is dropped.
  Each bucket also counts problems (failures, blocks, refusals) and requests for approval, so a
  strip can show red or amber where they happened.
- Migration 0008 adds one index (events by time) so the query stays fast on a large Ledger. No
  table changes.

### 8. Today's screens move onto the new look

- Every existing page uses the tokens and the new frame, with smaller text. What each page does
  does not change. Settings → Servers keeps working exactly as before (the owner's Phase 11
  Windows check is still to come).
- The **Gallery** page shows every component, in both themes, with real, empty, loading, and
  error data. It opens from Diagnostics, not from the left strip.

### 9. How the look is tested

- **Contrast:** a unit test checks every text/background pair in both themes against WCAG AA
  (4.5:1 for text, 3:1 for large text and status marks).
- **Look snapshots:** the end-to-end test opens the Gallery in the real app in both themes and
  compares each component's computed styles (colors, sizes, spacing, borders) with a snapshot
  checked into the repository. Screenshots are saved as evidence for people to look at.
  Pixel-by-pixel image comparison is not used: fonts render slightly differently from one
  computer to the next, so it would fail for no real reason.
- Status without color, keyboard use and focus outlines, 5,000 rows, and the smallest window
  size (800 × 560) are tested too.

## Consequences

- New screens are faster to build and look the same everywhere; Phase 12 builds its pages from
  this library.
- Every page gets denser. A later "view options" setting can offer larger text or icons only.
- The light theme exists everywhere at once; a new style that forgets a token fails the lint.
- The organization map keeps its own look (glows, colored ranks), expressed in tokens.
- One more workspace package to keep in step with the version check.

## Alternatives considered

- **A ready-made component kit (MUI, Fluent, Chakra):** heavy, hard to make as dense as the
  reference, and would bring its own look. Not chosen.
- **A virtualization library:** fixed-height rows need only a small helper. Not needed now.
- **Icons only on the left strip, like UniFi:** the owner chose names under the icons.
- **Pixel-by-pixel screenshot tests:** fragile across computers (section 9).
