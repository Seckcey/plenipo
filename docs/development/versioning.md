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

## Version per phase (owner decision)

The **minor** version increases by one each time a rollout phase is accepted, until the MVP:

| Phase accepted                           | Version     |
| ---------------------------------------- | ----------- |
| 0 — Repository foundation                | `0.1.0`     |
| 1 — Desktop shell and runtime supervisor | `0.2.0`     |
| 2 — Ledger                               | `0.3.0`     |
| 3 — Provider runtime adapters            | `0.4.0`     |
| 4 — Liaison message bus                  | `0.5.0`     |
| 5 — Workforce and organization engine    | `0.6.0`     |
| 6 — Model policy and routing             | `0.7.0`     |
| 7 — Capability broker, Guard, approvals  | `0.8.0`     |
| **8 — Development Department MVP**       | **`1.0.0`** |

- **Patch** (`z`) for fixes within a phase, e.g. `0.2.1`.
- **Pre-release** tags for test builds, e.g. `0.3.0-alpha.1`.
- After `1.0.0`, normal SemVer applies: minor for new features (Phases 9+), major for
  breaking changes.

## Releasing

1. Update the version in the files above (and `Cargo.lock`: `cargo update --workspace`).
2. Run `pnpm versions:check`.
3. Add release notes at `docs/releases/vX.Y.Z.md` (first line `# <release title>`).
4. Commit as `chore(release): vX.Y.Z`, merge to `main`, then tag the merge commit `vX.Y.Z` and
   push the tag.

Pushing a `v*` tag runs `.github/workflows/release.yml` on Windows: it checks that the tag matches
the product version and that release notes exist, builds the NSIS installer, and publishes a
GitHub release with the installer attached. `0.x` versions and SemVer pre-releases are published
as GitHub pre-releases.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org): `feat:`, `fix:`, `docs:`,
`chore:`, `refactor:`, `test:`, `ci:`, with an optional scope (`feat(core): …`).
