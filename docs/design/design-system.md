# Plenipo's design system

**Decision:** ADR-030 (one design system for every screen), accepted by the owner on 2026-09-27.
**Plan:** `ROLLOUT_PLAN.md`, Phase 12A. **Package:** `packages/ui` (`@plenipo/ui`).
**See it:** Diagnostics → **Open the gallery** shows every component, in every state, in either
theme or both side by side.

Plenipo looks like one dense, dark operations console (the reference is the UniFi Network
controller, in [`reference/`](reference/)). Every screen is built from the same tokens and
components, so a new page needs no styles of its own.

## Rules

1. **Tokens only.** Every color, size, and timing comes from a token: `var(--ui-…)` in CSS,
   `colorVar("accent")` in the rare inline style. Colors are written only in
   `packages/ui/src/tokens.ts`. `pnpm lint` refuses a raw color (`#…`, `rgb()`, `hsl()`, or a named
   color) anywhere else in the desktop app or the library: ESLint checks TypeScript, and
   `scripts/check-colors.mjs` checks CSS.
2. **Components first.** A screen composes `@plenipo/ui` components. It may lay out a page (grid
   areas, gaps) with tokens, but draws nothing the library already draws.
3. **One accent.** Electric blue (`accent`) means selection, links, focus, and the primary action.
   It never means status.
4. **Status is a mark and a word.** Never color alone. Each status also has its own mark shape.
5. **Dense by default.** 13 px text, 28 px table rows, small padding. Show more rows rather than
   more whitespace.
6. **Plain words.** Everything on screen follows [`vocabulary.md`](vocabulary.md).
7. **Both themes.** Dark is the default; light maps the same tokens. A new component is checked
   in both (the Gallery's "Both side by side").

## Tokens

The source is `packages/ui/src/tokens.ts`. `pnpm tokens` writes `src/generated/tokens.css` (CSS
custom properties) and `src/generated/tokens.json` (for the Rust side and exports); a test fails
when either is out of date. `<html data-theme="dark|light">` picks the theme; any element with
`data-theme` can show the other one (the Gallery does).

### Color

| Token                                            | Use                                                      |
| ------------------------------------------------ | -------------------------------------------------------- |
| `bg`                                             | Application background, behind everything                |
| `surface`                                        | Panels, cards, tables                                    |
| `surface-raised`                                 | Hover, selected, and floating surfaces above a panel     |
| `surface-sunken`                                 | Wells: logs, terminal output, inputs                     |
| `surface-overlay`                                | Translucent panels over the organization map             |
| `border` / `border-strong`                       | Thin dividers / dividers that must stand out             |
| `control-border`                                 | Outline of inputs, checkboxes, and switches (3:1)        |
| `text-primary` / `text-secondary` / `text-muted` | Titles and body / labels / timestamps and hints (all AA) |
| `accent` / `accent-strong`                       | Links, selection, focus / on hover                       |
| `accent-fill` / `on-accent`                      | Primary button background / its text                     |
| `accent-soft`                                    | Tint behind selected rows and the current section        |
| `ok`, `warn`, `error`, `offline`, `pending`      | The status ramp (marks, strips, bars)                    |
| `…-text`                                         | A status word in its color (AA on every surface)         |
| `…-soft`                                         | Tint behind a notice or pill of that status              |
| `on-error`                                       | Text on a solid error background (PRODUCTION, Stop all)  |
| `scrim`, `shadow`                                | Behind dialogs; elevation shadows                        |
| `skeleton-base`, `skeleton-shine`                | Loading placeholders                                     |
| `map-…`, `rank-…`, `role-…`                      | The organization map, rank colors, oversight roles       |
| `brand-mark`, `brand-stroke`                     | Plenipo's mark                                           |

**Status ramp.** `ok` working, online, succeeded · `warn` needs attention, blocked · `error`
failed, refused, production · `offline` idle, not running · `pending` waiting (for you, or in a
queue).

**Contrast.** A unit test checks every text/background pair in both themes against WCAG AA: 4.5:1
for text (text, links, and status words on every surface; each status word on its own tint; body
text on every tint), 3:1 for status marks, control outlines, and lines that carry meaning (map
connectors).

### Type, spacing, shape, motion

| Group     | Tokens                                                                                                                                     |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Type      | `font-xs` 11 · `font-sm` 12 · `font-md` 13 (base) · `font-lg` 15 · `font-xl` 17 · `font-2xl` 20 px                                         |
| Weight    | `weight-regular` 400 · `weight-medium` 500 · `weight-semibold` 600                                                                         |
| Families  | `font-sans` (Segoe UI Variable first) · `font-mono` (Cascadia Mono first)                                                                  |
| Spacing   | `space-1` 2 · `space-2` 4 · `space-3` 6 · `space-4` 8 · `space-5` 12 · `space-6` 16 · `space-7` 20 · `space-8` 24 · `space-9` 32 px        |
| Radius    | `radius-sm` 4 · `radius-md` 6 · `radius-lg` 8 px · `radius-pill`                                                                           |
| Sizes     | `row-height` 28 · `control-height` 26 · `rail-width` 88 · `topbar-height` 44 · `card-width` 240 · `card-height` 164 · `facet-width` 232 px |
| Elevation | `elevation-1`, `-2`, `-3` (geometry + the theme's `shadow`)                                                                                |
| Motion    | `motion-fast` 100 · `motion-base` 160 · `motion-slow` 240 ms · `ease`                                                                      |

Every number shown as a metric uses tabular figures (`.ui-num`, and every table's number
columns), so columns line up.

## Components

All live in `packages/ui/src`, with tests next to them. Each has empty, loading, and error states.

| Component                                                                                               | What it is                                                                                                                                                                                                  |
| ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AppShell`, `IconRail`, `TopBar`, `ScopeSelector`, `ThemeToggle`, `NotificationBell`                    | The frame: the left strip (icons with names under them, the owner's choice; a marker on the current section; count badges; tooltips), and the top bar (where you are, the page title, light/dark, the bell) |
| `Banner`, `BannerSlot`                                                                                  | Notices above the page: advisories and things you must do, with their button and **Dismiss**                                                                                                                |
| `StatusDot`, `StatusPill`, `CountBadge`, `HealthBar`, `Sparkline`, `ActivityStrip`, `Tag`               | Status primitives. The strip is the Site Manager card's 24-hour strip, with a time axis and a **Now** marker; a `Tag` is a plain name ("Run programs"), with no status mark                                 |
| `EntityCard`, `CardGrid`                                                                                | A department, project, or worker as a card (status, subtype, activity strip, who fills it, permission icons); the grid fits as many columns as the window allows and switches to a list                     |
| `DataTable`, `statusColumn`, `CellLink`                                                                 | The dense table: sortable columns, a status column, number columns, links to parent entities, row checkboxes, choosing columns, rows per page, and "1–100 of 5,000 records"                                 |
| `FacetPanel`, `useFacets`                                                                               | Filters: search, grouped checkboxes with counts, two-handle range sliders, **Clear filters**; collapses to a strip. `useFacets` binds it to a table or grid                                                 |
| `DetailSplitView`, `PropertyList`, `TimelineScrubber`                                                   | The detail page: properties and switches on the left, a timeline you can scrub (with **Live**) in the middle, a map on the right, a table below                                                             |
| `TopologyMap`, `layoutMap`                                                                              | Relationship map: tiles in their status color, labeled lines, a caption under each tile                                                                                                                     |
| `Button`, `IconButton`, `Switch`, `Checkbox`, `SearchField`, `TextField`, `Select`, `Segmented`, `Tabs` | Controls. `Switch` shows the words On/Off; `Tabs` move with the arrow keys                                                                                                                                  |
| `EmptyState`, `LoadingState`, `Skeleton`, `ErrorState`                                                  | The states every component shares. `EmptyState` and `ErrorState` can show Pip beside their words (`pip`), except in the compact size                                                                        |
| `PageHeader`, `Panel`, `RowList`, `StatGrid`, `Hero` (Phase 12)                                         | The parts of every page: its heading (with **Back** when another page opened it), titled panels, short lists of things to open, number tiles, and Pip's greeting on Home                                    |
| `MenuButton`, `ResizeHandle`, `LogView`, `terminalTheme` (Phase 12)                                     | A button that opens a menu (**New terminal**), an edge that resizes a panel with the mouse or the arrow keys, a read-only log (watch tabs), and the terminal's colors from the tokens                       |
| `PlenipoMark`, `PlenipoLogo`, `Pip` (Phase 12)                                                          | The owner's brand kit (`docs/brand/pip-brand-kit`): the three-rail P, the logo (the P, "lenipo", and Pip on the n, colored by four brand tokens so it follows the theme), and Pip in any of his 15 poses    |
| `Icon`                                                                                                  | Plenipo's own line icons (no UniFi artwork)                                                                                                                                                                 |

## Density guidelines

- Text 13 px; secondary 12 px; captions and axis labels 11 px. Headings: page 17 px, section 15 px.
- Table rows 28 px, controls 26 px, small buttons 22 px.
- Padding: 8–12 px inside panels and cards; 16–20 px around a page.
- One line per row: long text is cut with an ellipsis, and the full text is in a tooltip or the
  detail view.
- Prefer a table over cards once there are more than about 30 items; cards for fewer, richer
  items (departments, projects).

## Large lists

`DataTable` and `CardGrid` draw only what is on screen (a small windowing helper,
`useVirtualWindow`; no extra library). 5,000 table rows and hundreds of cards scroll smoothly:
unit tests scroll both, and the end-to-end test scrolls and sorts 5,000 rows in the real app.

## Activity over time

Strips and timelines come from the Ledger (the Phase 2 event model):

- `Ledger::activity` (desktop command `get_activity`) counts events in fixed time buckets for a
  scope: everything, a department (its projects and every position under its head, down to the
  head of another department), a project (its tasks and its Supervisor's team, down to another
  project's Supervisor), or a position (and the positions under it).
- **Downsampling:** a series always has the same number of buckets (96 for 24 hours: 15 minutes
  each). A longer range makes each bucket wider; counts are added up, so nothing is dropped.
  `downsample()` merges buckets the same way on screen; sparklines average.
- Each bucket also counts **problems** (failures, refusals, timeouts, a changed server ID) and
  **requests for approval**. A task that is blocked while it waits for handoff replies is normal
  work, not a problem. Not color alone: work fills the lower half of the strip, waiting for
  approval three quarters (violet), and a problem the whole height (red), so problems stand up
  above the rest. Busy buckets are drawn in three steps of green relative to the busiest one.
- Migration 0008 adds an index on event times, so the query stays fast on a large Ledger.

## Testing the look

- **Contrast:** `packages/ui/src/tokens.test.ts` (both themes, every pair).
- **Look snapshots:** the end-to-end test `tests/e2e/specs/design.e2e.mjs` opens the Gallery in the
  real app, in both themes, and compares each sample's computed styles (colors, sizes, spacing,
  borders) with `tests/e2e/snapshots/gallery-dark.json` and `gallery-light.json`. After an
  intended change, run `PLENIPO_E2E_UPDATE_SNAPSHOTS=1 pnpm e2e` and check the new files in.
  Screenshots are saved for people to look at; pixel-by-pixel comparison is not used, because
  fonts render slightly differently from one computer to the next.
- **Status without color**, **keyboard use and focus outlines**, and **5,000 rows** are tested in
  the library's unit tests and in the real app. **The smallest window (800 × 560)** is tested
  only in the real app (unit tests have no layout).

## Adding a component

1. Build it in `packages/ui/src` with tokens only, and a `ui-` class prefix.
2. Give it empty, loading, and error states.
3. Add it to the Gallery (`src/gallery/Gallery.tsx`) with a `Variant` in every state: a `name` for
   the look test and a plain-words `caption` for people.
4. Test it: behaviour, keyboard, and status words.
5. Run `PLENIPO_E2E_UPDATE_SNAPSHOTS=1 pnpm e2e`, look at the Gallery in both themes, and check in
   the new look snapshot.
