# ADR-004: Minimal monorepo layout, grow crates per phase

- **Status:** Proposed
- **Date:** 2026-09-25
- **Phase:** 0

## Context

ROLLOUT_PLAN.md suggests a structure with `crates/core`, `runtime`, `liaison`, `capabilities`,
`guard`, `ledger`, `integrations`, `packages/ui`, `packages/types`, and `tests`, while also
saying: "keep the number of packages small until real separation is necessary". Empty crates
add build time, maintenance, and false signals about what exists.

## Decision

Phase 0 creates only what has real content:

| Path                     | Why it exists now                            |
| ------------------------ | -------------------------------------------- |
| `apps/desktop`           | UI (React/TS)                                |
| `apps/desktop/src-tauri` | Tauri backend / command boundary             |
| `crates/core`            | Shared DTOs and domain logic                 |
| `packages/types`         | TypeScript DTOs generated from `crates/core` |

Deferred until the phase that needs them:

| Path                                   | Created in                                            |
| -------------------------------------- | ----------------------------------------------------- |
| `crates/runtime`                       | Phase 1 (supervisor)                                  |
| `crates/ledger`                        | Phase 2                                               |
| `crates/liaison`                       | Phase 4                                               |
| `crates/capabilities`, `crates/guard`  | Phase 7                                               |
| `crates/integrations`                  | Phase 8/9                                             |
| `packages/ui`                          | When a second UI consumer exists (e.g. CrewOS)        |
| `tests/` (cross-crate integration/e2e) | Phase 1, when there is a multi-component path to test |

Unit tests live next to their code (`#[cfg(test)]` modules and `*.test.ts(x)`).

The Tauri app lives at `apps/desktop/src-tauri` (Tauri's convention) and is a member of the
root Cargo workspace, so `cargo test --workspace` covers it.

## Consequences

- Structure matches reality; each new crate appears with the phase that justifies it.
- Moving code out of `crates/core` into a new crate later is a mechanical refactor.
- Reviewers should push back on new crates or packages that lack a concrete consumer.

## Alternatives considered

- **Create every suggested directory now with placeholder code** — rejected; it contradicts the
  plan's own "keep packages small" guidance and adds no verifiable value.
