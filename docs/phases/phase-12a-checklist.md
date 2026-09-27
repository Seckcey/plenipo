# Phase 12A — Implementation Checklist

**Status:** complete on `claude/phase-12`. See the [acceptance report](phase-12a-acceptance-report.md).

Source: `ROLLOUT_PLAN.md`, Phase 12A — Visual Design System (UniFi-Style Console Aesthetic).
Built on v1.6.0 (Phase 11, servers over SSH) and the plan change for the terminal panel
([Seckcey/plenipo#33](https://github.com/Seckcey/plenipo/pull/33)).

This checklist keeps the plan's words where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md).

**Goal (plan):** "Establish the Plenipo visual language and shared component library before the
Phase 12 screens are built, so every operator surface reads as one dense, dark, professional
network-operations console rather than a set of separately styled pages."

## Owner decisions (2026-09-27)

- ADR-029 (one design system for every screen): **accepted.** Every screen uses the same
  building blocks; no screen gets its own one-off style.
- The left strip shows **names under the icons** (UniFi shows icons only). View options come
  later.
- **Smaller, tighter text:** 13 px instead of 15 px.
- Version **1.7.0**. Phase 12 follows as 1.8.0.

## Design decisions

Details are in ADR-029.

- One package, `@plenipo/ui` (`packages/ui`), for tokens, components, and their styles.
- Tokens written once in `packages/ui/src/tokens.ts`; `tokens.css` and `tokens.json` are
  generated from it and checked by a test (`pnpm tokens` rewrites them).
- Dark by default, light as a choice; the choice is remembered on this computer.
- `pnpm lint` refuses raw colors in the desktop app's code and styles.
- Tables and card grids draw only what is on screen (a small helper, no new dependency).
- Activity strips come from the Ledger: fixed buckets (96 for 24 hours), counts added up for
  longer ranges; migration 0008 adds an index on event times.
- The Gallery page opens from Diagnostics.

## Deliverables (plan)

### Design tokens

- [x] color: background, surface, surface-raised, border, text-primary, text-secondary,
      text-muted, accent, and status ramp (ok / warn / error / offline / pending)
- [x] typography scale (roughly 11–20px), tabular numerals for all metrics
- [x] spacing, radius (small, 4–8px), elevation, and motion tokens
- [x] light theme mapping of the same tokens; dark is the default

### Core layout shell

- [x] icon rail with active-section indicator and tooltips (names under the icons, the
      owner's choice)
- [x] collapsible left facet/filter panel (search box, grouped checkbox filters with counts,
      range sliders, "Clear Filters")
- [x] top bar: scope selector (org / department / project), title, theme toggle,
      notification badge
- [x] global banner slot for advisories and required actions, with an inline call-to-action
      button and dismiss

### Component library

- [x] Entity card: title, status dot and subtype line, 24h activity strip with time axis
      labels and a "Now" marker, a provider/owner row, and a footer row of small
      capability/resource icons
- [x] Card grid with responsive column count and a card/list view toggle
- [x] Dense data table: sortable columns, status dot column, tabular numeric columns, inline
      links to parent entities, per-row selection checkboxes, column customization, page-size
      control, and a records counter
- [x] Facet filter panel bound to the table and grid
- [x] Detail split view: properties/toggles panel, live timeline scrubber, topology map, table
      below
- [x] Topology / relationship map: node tiles with status color fill, labeled connectors,
      per-node metric captions
- [x] Status primitives: dot, pill, activity strip, sparkline, health bar, count badge
- [x] Empty, loading (skeleton), and error states for every component above

### Documentation

- [x] `docs/design/design-system.md`: tokens, components, usage rules, density guidelines
- [x] Gallery page in the desktop app: every component, all states, both themes

## Technical implementation (plan)

- [x] Tokens as CSS custom properties, generated from a single TypeScript source
- [x] Components in a shared `packages/ui` workspace package; no screen-level ad-hoc styling
- [x] No hardcoded color literals in feature code; a lint rule enforces token usage
- [x] Virtualized tables and card grids (1,000+ rows, 100+ cards stay responsive)
- [x] Activity strips and timelines driven by the Phase 2 event model, with a defined
      downsampling strategy

## Existing screens

- [x] Every page moves onto the tokens and the new frame; what each page does stays the same
- [x] Settings → Servers works exactly as before (Phase 11 owner check still to come)
- [x] Existing unit and end-to-end tests pass unchanged, or with selector updates only

## Tests (plan)

- [x] visual regression snapshots of the gallery in dark and light themes
- [x] token contrast check: all text/background pairs meet WCAG AA
- [x] status is distinguishable without color (dot plus label present in DOM)
- [x] keyboard navigation and focus-visible styling across rail, filters, table, and cards
- [x] virtualized table performance with 5,000 rows
- [x] responsive behavior at the minimum supported window size

## Acceptance criteria (plan)

- [x] Every component in the library renders correctly in both themes with real and empty data.
- [x] Phase 12 screens can be assembled entirely from this library without introducing new
      one-off styles.
- [x] No feature code contains raw color values.

## Before pushing

- [x] `pnpm check`
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [x] `cargo test --workspace --locked`
- [x] `pnpm bindings`, then no diff in `packages/types/src/generated`
- [x] `pnpm e2e` against the release build
