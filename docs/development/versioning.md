# Versioning

Plenipo uses [Semantic Versioning 2.0.0](https://semver.org) with a **single product version**
shared by every package and crate in the repository.

## Source of truth

The root `package.json` `version` is authoritative. These must match it:

| File                                     | Field                                                                     |
| ---------------------------------------- | ------------------------------------------------------------------------- |
| `apps/desktop/package.json`              | `version`                                                                 |
| `packages/types/package.json`            | `version`                                                                 |
| `Cargo.toml`                             | `[workspace.package] version` (inherited by all crates)                   |
| `apps/desktop/src-tauri/tauri.conf.json` | `version` must be `"../package.json"` (reads the desktop package version) |

`pnpm versions:check` (run in CI) fails if any of these disagree.

## Pre-1.0 rules

While `0.y.z`:

- `y` (minor) increments when a rollout phase is accepted: Phase 0 → `0.1.0`, Phase 1 → `0.2.0`, …
- `z` (patch) for fixes within a phase.
- Pre-release tags for test builds: `0.2.0-alpha.1`.

`1.0.0` is reserved for the first release after the MVP boundary (Phase 8) that is fit for
day-to-day use.

## Releasing

1. Update the version in the files above.
2. Run `pnpm versions:check`.
3. Commit as `chore(release): vX.Y.Z` and tag `vX.Y.Z`.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org): `feat:`, `fix:`, `docs:`,
`chore:`, `refactor:`, `test:`, `ci:`, with an optional scope (`feat(core): …`).
