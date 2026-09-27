# Phase 12A — Implementation Checklist

**Status:** in progress on `claude/phase-12`.

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

- [ ] color: background, surface, surface-raised, border, text-primary, text-secondary,
      text-muted, accent, and status ramp (ok / warn / error / offline / pending)
- [ ] typography scale (roughly 11–20px), tabular numerals for all metrics
- [ ] spacing, radius (small, 4–8px), elevation, and motion tokens
- [ ] light theme mapping of the same tokens; dark is the default

### Core layout shell

- [ ] icon rail with active-section indicator and tooltips (names under the icons, the
      owner's choice)
- [ ] collapsible left facet/filter panel (search box, grouped checkbox filters with counts,
      range sliders, "Clear Filters")
- [ ] top bar: scope selector (org / department / project), title, theme toggle,
      notification badge
- [ ] global banner slot for advisories and required actions, with an inline call-to-action
      button and dismiss

### Component library

- [ ] Entity card: title, status dot and subtype line, 24h activity strip with time axis
      labels and a "Now" marker, a provider/owner row, and a footer row of small
      capability/resource icons
- [ ] Card grid with responsive column count and a card/list view toggle
- [ ] Dense data table: sortable columns, status dot column, tabular numeric columns, inline
      links to parent entities, per-row selection checkboxes, column customization, page-size
      control, and a records counter
- [ ] Facet filter panel bound to the table and grid
- [ ] Detail split view: properties/toggles panel, live timeline scrubber, topology map, table
      below
- [ ] Topology / relationship map: node tiles with status color fill, labeled connectors,
      per-node metric captions
- [ ] Status primitives: dot, pill, activity strip, sparkline, health bar, count badge
- [ ] Empty, loading (skeleton), and error states for every component above

### Documentation

- [ ] `docs/design/design-system.md`: tokens, components, usage rules, density guidelines
- [ ] Gallery page in the desktop app: every component, all states, both themes

## Technical implementation (plan)

- [ ] Tokens as CSS custom properties, generated from a single TypeScript source
- [ ] Components in a shared `packages/ui` workspace package; no screen-level ad-hoc styling
- [ ] No hardcoded color literals in feature code; a lint rule enforces token usage
- [ ] Virtualized tables and card grids (1,000+ rows, 100+ cards stay responsive)
- [ ] Activity strips and timelines driven by the Phase 2 event model, with a defined
      downsampling strategy

## Existing screens

- [ ] Every page moves onto the tokens and the new frame; what each page does stays the same
- [ ] Settings → Servers works exactly as before (Phase 11 owner check still to come)
- [ ] Existing unit and end-to-end tests pass unchanged, or with selector updates only

## Tests (plan)

- [ ] visual regression snapshots of the gallery in dark and light themes
- [ ] token contrast check: all text/background pairs meet WCAG AA
- [ ] status is distinguishable without color (dot plus label present in DOM)
- [ ] keyboard navigation and focus-visible styling across rail, filters, table, and cards
- [ ] virtualized table performance with 5,000 rows
- [ ] responsive behavior at the minimum supported window size

## Acceptance criteria (plan)

- [ ] Every component in the library renders correctly in both themes with real and empty data.
- [ ] Phase 12 screens can be assembled entirely from this library without introducing new
      one-off styles.
- [ ] No feature code contains raw color values.

## Before pushing

- [ ] `pnpm check`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `pnpm bindings`, then no diff in `packages/types/src/generated`
- [ ] `pnpm e2e` against the release build
