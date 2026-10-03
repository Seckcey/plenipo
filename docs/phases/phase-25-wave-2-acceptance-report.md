# Phase 25 — Acceptance Report (Wave 2)

|              |                                                                                                                                                                                                       |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 25 — Fixes and a simpler Plenipo before launch, **Wave 2** ("make Plenipo simple")                                                                                                                    |
| **Branch**   | `claude/relaxed-mccarthy-uxyguw` ([PR #156](https://github.com/Seckcey/plenipo/pull/156)), commits `71b88e4` to `b1dcffd`, with the end-to-end test fix `eabac08`                                     |
| **Verified** | `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff), and `pnpm docs:check`, here. GitHub CI on the pull request, Windows and the E2E included. |
| **Date**     | 2026-10-03 (Pacific time)                                                                                                                                                                             |
| **Result**   | All nine items built. The owner's check (a new install with only the tour) is next (section 3). Plenipo is made by 8 West Ventures, LLC.                                                              |

## In short

Wave 2 made Plenipo simpler. Cards start closed with one line each, a few questions are gone, you
pick the exact model for each job on one screen, a new project uses the workers you already have,
templates start an organization in one click, and a setup tour walks you through. **What you need
to do:** install Plenipo fresh and use only the tour to get to a running objective.

## 1. Items → result

| #   | Item (the checklist)                                  | Result   | Evidence                                                                                      |
| --- | ----------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------- |
| 2.1 | AI tools cards start closed, with a light and a line  | **Done** | `67978d9`: one card per AI company, closed, with **Sign in**.                                 |
| 2.2 | Connections cards start closed                        | **Done** | `ea21348`: a shared Disclosure part in `packages/ui`, one line per card.                      |
| 2.3 | Remove context size                                   | **Done** | `71b88e4`; ADR-194.                                                                           |
| 2.4 | No "make images" or "use a computer" questions        | **Done** | `71b88e4`; ADR-194.                                                                           |
| 2.5 | Pick the exact model for each job                     | **Done** | `188d339`: every model of every AI tool in the menu, subscriptions first; ADR-195.            |
| 2.6 | One "who uses what" screen                            | **Done** | `188d339`: Settings → AI models starts with **Who uses what**; ADR-195.                       |
| 2.7 | Use the team you hired before bringing in new workers | **Done** | `1ce306f`: no copies at setup; Home asks before hiring; ADR-196.                              |
| 2.8 | Templates                                             | **Done** | `2e7308f`: organizations, departments, and projects; ADR-197.                                 |
| 2.9 | A real setup tour, with Driver.js                     | **Done** | `b1dcffd`: nine steps from signing in to the first objective (Driver.js 1.9.0, MIT); ADR-198. |

The tests for each item are named in the [checklist](phase-25-checklist.md).

## 2. Decisions

- [ADR-194 (what a model can do is no longer asked)](../adr/ADR-194-what-a-model-can-do-is-no-longer-asked.md)
- [ADR-195 (who uses what)](../adr/ADR-195-who-uses-what.md)
- [ADR-196 (use the team you hired first)](../adr/ADR-196-use-the-team-you-hired-first.md)
- [ADR-197 (templates)](../adr/ADR-197-templates.md)
- [ADR-198 (the setup tour)](../adr/ADR-198-the-setup-tour.md)

## 3. The owner's checks (Windows 11)

- A brand-new install, using only the setup tour, gets from the first launch to a running
  objective, with Fable for the developer and Sonnet for the docs.
- The project reuses the workers already hired.
