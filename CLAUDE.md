# Plenipo — notes for AI coding sessions

- **Always explain like the owner is in 5th grade.** Every time you explain something to the
  owner — in chat, in a pull request summary, or in steps to follow — use short sentences and
  everyday words. Say what happened and what the owner needs to do first. If you must use a
  technical word, say what it means in a few simple words right after it.
- **Plain words on screen.** Everything a person sees uses simple, everyday words. Follow
  [`docs/design/vocabulary.md`](docs/design/vocabulary.md): "AI tool", not "runtime"; Worker →
  Supervisor → Manager → VP → President, not coordinator or superintendent. Code keeps the
  rollout plan's names (ADR-010, plain words and rank names).
- **Credit 8 West Ventures, LLC.** Plenipo is made by 8 West Ventures, LLC. Give it credit
  wherever it fits — the installer, About, release notes, the website, documents, and pull
  requests. Never remove or rename a reference to 8 West Ventures, LLC or 8 West IT (a change
  that must move one keeps it in the new place).
- **Name decisions, never just number them.** When you mention an ADR to the owner, say what it
  is and what accepting it means — for example, "ADR-010 (plain words and rank names)". People
  don't remember records by number.
- **Work the plan:** `ROLLOUT_PLAN.md` phase by phase, with a checklist and an acceptance report
  in `docs/phases/`, and an ADR in `docs/adr/` for any deviation.
- **The final push (ADR-132, the owner's order, 2026-10-01).** After Phase 22's go-live (done in
  its own session), the work is exactly three phases, in this order: **1. Phase 14** (Plenipo on
  your phone), **2. Phase 23** (Mac and Linux), **3. Phase 24** (Community). Do not start Phase
  14 until the owner says Phase 22 is live. Phase 16's Wave 4, Phase 15, and Phase 9 are
  **parked**: never start one on your own; only the owner can schedule it.
- **Before pushing** (see `docs/development/setup.md`):
  - `pnpm check`
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `pnpm bindings`, then no diff in `packages/types/src/generated`
- **Docs-only changes** (only `.md` files, or pictures under `docs/`): `pnpm docs:check` is
  enough before pushing. On GitHub, only the quick **Docs** check runs (about a minute); the long
  jobs skip themselves and count as passed. Anything else runs the full list above.
