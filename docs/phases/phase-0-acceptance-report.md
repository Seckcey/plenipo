# Phase 0 — Acceptance Report

|                     |                                                                                               |
| ------------------- | --------------------------------------------------------------------------------------------- |
| **Phase**           | 0 — Repository Foundation and Architectural Contract                                          |
| **Branch**          | `claude/inspiring-cray-qdwzmr`                                                                |
| **Commit verified** | `8ea82e0`                                                                                     |
| **CI run**          | [36202467902](https://github.com/Seckcey/plenipo/actions/runs/36202467902) — all 3 jobs green |
| **Date**            | 2026-09-25                                                                                    |
| **Result**          | **All Phase 0 acceptance criteria pass.** Owner review items below.                           |

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                         | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                      |
| --- | ----------------------------------------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1  | A clean Windows machine with documented prerequisites can build and launch Plenipo. | **Pass** | CI job _Windows (test, build, installer, launch smoke)_ on a fresh ephemeral `windows-latest` VM: `pnpm install --frozen-lockfile` → `pnpm build` (release exe + NSIS installer) → launch smoke test logged `Plenipo exited with code 0`. Installer uploaded as artifact `plenipo-windows-installer` (1.88 MB zip). Prerequisites documented in [`docs/development/setup.md`](../development/setup.md). See caveat R1.        |
| A2  | The app opens to a branded shell without runtime errors.                            | **Pass** | Launch smoke test (see §3) only succeeds if the React shell mounts **and** a real IPC round-trip to Rust Core (`get_app_info`) succeeds; any render/runtime/IPC failure causes watchdog exit 1. Passed on Windows (CI) and Linux (local, twice incl. fresh clone). Screenshot: [`evidence/phase-0-shell-linux.png`](evidence/phase-0-shell-linux.png). Frontend tests assert the brand, heading, and "Core: Connected" state. |
| A3  | No provider API keys or credentials are required merely to launch.                  | **Pass** | Local launches ran under `env -i PATH=… HOME=<empty dir>` — no other variables, no config, no credentials — and passed. CI Windows job has no secrets configured. No `.env` is read. Test `requires no credentials to render` asserts no key/password/token inputs. Shell footer states "no providers or credentials required".                                                                                               |
| A4  | All required checks are green.                                                      | **Pass** | [CI run 36202467902](https://github.com/Seckcey/plenipo/actions/runs/36202467902): Frontend ✅ · Rust ✅ · Windows ✅.                                                                                                                                                                                                                                                                                                        |

## 2. Required Phase 0 tests → evidence

| Test (ROLLOUT_PLAN.md)                         | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Fresh clone can install dependencies and build | **Pass** | Local: fresh `git clone` of the pushed branch into an empty directory with a separate target dir → `pnpm install --frozen-lockfile`, `pnpm check`, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` (24 passed), `pnpm build` → release binary; `git status` clean afterwards (no generated-file drift). Binary then passed the launch smoke test. CI: every job starts from a fresh checkout with `--frozen-lockfile` / `--locked`. |
| Tauri application launches on Windows          | **Pass** | CI Windows launch smoke: `Plenipo exited with code 0` (renders shell + Core IPC).                                                                                                                                                                                                                                                                                                                                                                 |
| Frontend test command succeeds                 | **Pass** | `pnpm test`: 2 files, 7 tests passed (Linux CI, Windows CI, local).                                                                                                                                                                                                                                                                                                                                                                               |
| Rust test command succeeds                     | **Pass** | `cargo test --workspace --locked`: 24 passed (core 9, desktop 15) on Linux CI, Windows CI, local.                                                                                                                                                                                                                                                                                                                                                 |
| TypeScript typecheck succeeds                  | **Pass** | `pnpm typecheck` (both packages) in CI Frontend job.                                                                                                                                                                                                                                                                                                                                                                                              |
| CI succeeds on the initial branch              | **Pass** | Run 36202467902 on `claude/inspiring-cray-qdwzmr`.                                                                                                                                                                                                                                                                                                                                                                                                |

## 3. Failure-path verification (§3.4: "do not mark complete because the happy path worked once")

| Scenario                                                                                   | Expected                            | Observed                                                             |
| ------------------------------------------------------------------------------------------ | ----------------------------------- | -------------------------------------------------------------------- |
| Frontend never loads (debug binary, no dev server) under smoke mode                        | exit 1                              | `smoke test FAILED: frontend did not report ready in 5s`, **exit 1** |
| Normal launch, no smoke env                                                                | keeps running                       | still running after 8 s (killed by `timeout`, code 124)              |
| Frontend: Core IPC fails                                                                   | error shown, ready **not** reported | test `shows an error and does not report ready…`                     |
| Raw `invoke` imported outside the typed client                                             | lint error                          | ESLint `no-restricted-imports` error confirmed                       |
| Unknown command (`run_shell`)                                                              | rejected                            | IPC boundary test                                                    |
| OS plugin commands (`plugin:shell\|execute`, `plugin:fs\|read_file`, `plugin:http\|fetch`) | rejected                            | IPC boundary test                                                    |
| Granted command from a remote origin (`https://example.com`) in `main`                     | rejected                            | IPC boundary test                                                    |
| Granted command from a window without a capability grant                                   | rejected                            | IPC boundary test                                                    |
| Malformed backend error kind / non-error rejection                                         | normalized to `internal`            | `commands.test.ts`                                                   |
| Rust ↔ TS DTO drift                                                                        | CI fails                            | "Generated TypeScript bindings are up to date" step                  |
| Manifest version mismatch                                                                  | CI fails                            | `pnpm versions:check`                                                |

Two defects were found and fixed by these checks during Phase 0:

1. **Exit code swallowed.** Tauri's `Builder::run` (and, on Linux, `run_return`) returned 0 even after `exit(1)`, so a broken UI would have passed the smoke test. Fixed by tracking the smoke outcome in shared state (`smoke.rs`, first-outcome-wins) with unit tests.
2. **Windows test binaries crashed** (`STATUS_ENTRYPOINT_NOT_FOUND`) because `tauri-build` embeds the Common-Controls v6 manifest only into the app binary. Fixed in `build.rs` by embedding it through linker args for all targets on Windows MSVC (commit `8ea82e0`).

## 4. Deliverables

| Deliverable                                | Location                                                                                       |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| Tauri 2 desktop project                    | `apps/desktop/src-tauri/`                                                                      |
| React + TypeScript frontend                | `apps/desktop/src/`                                                                            |
| Rust Tauri backend, typed command boundary | `src-tauri/src/commands.rs`, `build.rs` command manifest, `capabilities/default.json`          |
| Shared serializable DTOs                   | `crates/core/src/dto.rs` → generated `packages/types/src/generated/`                           |
| Repository structure                       | pnpm + Cargo workspaces; see [ADR-004](../adr/ADR-004-repository-layout.md)                    |
| README                                     | [`README.md`](../../README.md)                                                                 |
| Architecture document                      | [`docs/architecture/overview.md`](../architecture/overview.md)                                 |
| Developer setup                            | [`docs/development/setup.md`](../development/setup.md)                                         |
| Environment/config conventions             | [`docs/development/configuration.md`](../development/configuration.md), `.env.example`         |
| Linting and formatting                     | ESLint + Prettier; `cargo fmt` + `clippy -D warnings`                                          |
| Unit-test harnesses                        | Vitest + Testing Library; `cargo test` incl. Tauri mock-runtime IPC tests                      |
| CI workflow                                | [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)                                   |
| Versioning convention                      | [`docs/development/versioning.md`](../development/versioning.md), `scripts/check-versions.mjs` |
| ADR directory                              | [`docs/adr/`](../adr/README.md): ADR-001, 002, 003 (required) + 004                            |

## 5. Security review (Definition of Done §7)

- No secrets in the repo; `.env*` (except `.env.example`), `*.pem`, `*.key` git-ignored. CI permissions `contents: read`.
- WebView treated as untrusted: only 2 app commands exist, each needs an explicit capability grant; no fs/shell/http/process plugins installed; CSP restricts scripts to `'self'`.
- `unsafe_code = "forbid"` workspace-wide.
- Smoke-test mode only allows the UI to end the process early; timeout is bounded (1–600 s).
- Nothing requests elevation; the installer is per-user (`installMode: currentUser`).

## 6. Deviations from the plan

| Deviation                                                                                 | Why                                                                         | Recorded                                    |
| ----------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------- |
| Only `crates/core` and `packages/types` created; other suggested crates/packages deferred | Plan says to keep packages small until separation is needed                 | ADR-004 (Proposed)                          |
| TypeScript pinned to 6.0.x (latest is 7.x)                                                | typescript-eslint 8 supports `<6.1`                                         | ADR-001 consequences, setup troubleshooting |
| Added `frontend_ready` command + `PLENIPO_SMOKE_TEST` mode                                | Needed to verify "launches without runtime errors" automatically on Windows | Architecture overview §4                    |
| No cross-crate `tests/` directory yet                                                     | No multi-component path to test until Phase 1                               | ADR-004                                     |

## 7. Residual risks and owner review items

| ID  | Item                                                                                                                                                                  | Recommendation                                                                                                                                         |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| R1  | CI's `windows-latest` image already has build tools installed, so it proves "fresh checkout builds and launches on Windows", not "a bare machine following setup.md". | Owner: follow `docs/development/setup.md` once on your Windows 11 machine (~20 min), or install the CI installer artifact and confirm the shell opens. |
| R2  | Bundle identifier `com.eightwestventures.plenipo` is a placeholder I chose. It determines `%APPDATA%` paths; changing it after real data exists orphans that data.    | Confirm or supply the identifier before Phase 2 (Ledger) writes data.                                                                                  |
| R3  | Installer is unsigned (SmartScreen warning).                                                                                                                          | Planned for Phase 13.                                                                                                                                  |
| R4  | ADR-004 is **Proposed**.                                                                                                                                              | Owner: accept or amend.                                                                                                                                |
| R5  | CI uses the latest stable Rust (1.98 on the runner) while local was 1.94; `rust-toolchain.toml` tracks `stable`.                                                      | Acceptable for now; pin a version if reproducibility issues appear.                                                                                    |

## 8. Phase boundary

Phase 0 is complete. Per ROLLOUT_PLAN §8.12, work stops here for owner review because
**Phase 1 materially expands privileges**: it introduces a runtime supervisor that launches
local executables. Phase 1 has not been started.
