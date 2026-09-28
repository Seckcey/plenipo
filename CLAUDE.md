# Plenipo — notes for AI coding sessions

- **Always explain like the owner is in 5th grade.** Every time you explain something to the
  owner — in chat, in a pull request summary, or in steps to follow — use short sentences and
  everyday words. Say what happened and what the owner needs to do first. If you must use a
  technical word, say what it means in a few simple words right after it.
- **Plain words on screen.** Everything a person sees uses simple, everyday words. Follow
  [`docs/design/vocabulary.md`](docs/design/vocabulary.md): "AI tool", not "runtime"; Worker →
  Supervisor → Manager → VP → President, not coordinator or superintendent. Code keeps the
  rollout plan's names (ADR-010, plain words and rank names).
- **Name decisions, never just number them.** When you mention an ADR to the owner, say what it
  is and what accepting it means — for example, "ADR-010 (plain words and rank names)". People
  don't remember records by number.
- **Work the plan:** `ROLLOUT_PLAN.md` phase by phase, with a checklist and an acceptance report
  in `docs/phases/`, and an ADR in `docs/adr/` for any deviation.
- **Before pushing** (see `docs/development/setup.md`):
  - `pnpm check`
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `pnpm bindings`, then no diff in `packages/types/src/generated`
- **Docs-only changes** (only `.md` files, or pictures under `docs/`): `pnpm docs:check` is
  enough before pushing. On GitHub, only the quick **Docs** check runs (about a minute); the long
  jobs skip themselves and count as passed. Anything else runs the full list above.
