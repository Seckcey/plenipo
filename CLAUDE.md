# Plenipo — notes for AI coding sessions

- **Plain words on screen.** Everything a person sees uses simple, everyday words. Follow
  [`docs/design/vocabulary.md`](docs/design/vocabulary.md): "AI tool", not "runtime"; Worker →
  Supervisor → Manager → VP → President, not coordinator or superintendent. Code keeps the
  rollout plan's names (ADR-010).
- **Work the plan:** `ROLLOUT_PLAN.md` phase by phase, with a checklist and an acceptance report
  in `docs/phases/`, and an ADR in `docs/adr/` for any deviation.
- **Before pushing** (see `docs/development/setup.md`):
  - `pnpm check`
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `pnpm bindings`, then no diff in `packages/types/src/generated`
