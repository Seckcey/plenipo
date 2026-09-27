## What this changes

<!-- One or two sentences. What does this do, and why? -->

## How it was checked

<!-- Delete any line that does not apply. -->

- [ ] `pnpm check`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `pnpm bindings`, and no diff in `packages/types/src/generated`
- [ ] `pnpm e2e`
- [ ] Tried it in the running app

## Screenshots

<!-- Required if anything changed on screen. Before and after, if you have both. -->

## Notes for the reviewer

- **Plain words:** any new text a person can see uses everyday words
  ([`docs/design/vocabulary.md`](https://github.com/Seckcey/plenipo/blob/main/docs/design/vocabulary.md)).
- **Decisions:** anything that changes the architecture, or deviates from
  [`ROLLOUT_PLAN.md`](https://github.com/Seckcey/plenipo/blob/main/ROLLOUT_PLAN.md), has an ADR in [`docs/adr/`](https://github.com/Seckcey/plenipo/blob/main/docs/adr/README.md).
- **Permissions:** anything touching files, programs, the network, the browser, or the screen goes
  through Guard and the capability broker.

<!-- Closes #000 -->
