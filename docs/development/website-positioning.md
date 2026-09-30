# Website positioning: a coordinated team across AI providers

The homepage explains the product through a job: give a coordinator an outcome, hire agents
for the roles it needs, and let those roles work together across supported AI providers.
Model policies and development tools explain how the team works. They support the story
rather than being the starting point.

## Copy and scope

The headline is **Hire your AI team. Give it a goal.** The supporting copy introduces a
coordinator, roles, multiple providers, and owner-controlled model and effort choices.
The example hires a developer, reviewer, and writer for a services page; Claude Code and Codex
illustrate a possible policy configuration rather than a mandatory or preconfigured pairing.

The change covers homepage copy and search descriptions, a hiring example, and a development
workspace section. It preserves Pip and the existing visual system, interactive sample and
automatic startup, sample disclosures, installer/version templating, pricing, and product
behavior. The README introduction follows the same positioning, preserving the live release
badge and other changes from prerequisite PR #99, merged before this work's integration.
The social image is retained as a brand asset; its original wording is not regenerated here.

## Claim evidence

Inspected GitHub main at `1c5bc2bef2970f0bef98724b08c0c426ad9b5690` and the published
`v1.11.0` tag on September 29, 2026. The role defaults and code Watch component are identical
between those snapshots. The policy engine adds a readiness refusal for an unresponsive updated
AI tool; its model/effort selection rules are unchanged. Main contains newer desktop work, so a main version
number alone is not evidence that a capability has a downloadable installer.

| Homepage claim                                             | Source and published scope                                                                                                                                                                                                     | Wording boundary                                                                                                                                                                                                |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A coordinator delegates work to roles and receives results | [Development design](../adr/ADR-016-development-department.md), `crates/workforce/src/directory.rs`, `crates/capabilities/tests/development.rs`; present in v1.11.0                                                            | Describe coordination and handoffs; do not promise successful completion of every goal.                                                                                                                         |
| Hire by role with responsibilities and permissions         | `crates/workforce/src/templates.rs`, `crates/workforce/src/service.rs`, [provider-independent roles](../adr/ADR-003-provider-independent-roles.md)                                                                             | Hiring is an AI-agent metaphor, not a claim of human judgment or employment.                                                                                                                                    |
| Suggested role policies select available models            | `template_policies()` in `crates/workforce/src/templates.rs`, `route()` in `crates/router/src/engine.rs`; selection behavior present in v1.11.0                                                                                | Defaults set preferences, such as another provider for review and economical models for documentation. They do not seed a benchmarked best-model assignment for every job.                                      |
| Owners can change model and effort rules                   | `crates/router/src/engine.rs`, `config.rs`, [layered policies](../adr/ADR-041-model-effort-learning-layers.md); present in v1.11.0                                                                                             | Closest applicable settings decide; excluded providers accumulate. Unset effort falls back to model or tool defaults. Existing conversations do not silently switch providers on every task.                    |
| Cross-provider review                                      | `crates/router/src/engine.rs`, [routing design](../adr/ADR-011-model-policy-routing.md)                                                                                                                                        | Policies can prefer or require another provider. Availability, sign-in, capabilities, and project permissions still apply. Ollama cloud integration is text-only.                                               |
| Development workspace                                      | [development working copies](../adr/ADR-016-development-department.md), [terminal panel](../adr/ADR-031-terminal-panel.md), [code Watch](../adr/ADR-055-watch-a-worker-write-code.md), [v1.11.0 notes](../releases/v1.11.0.md) | Name working copies, Git branches, owner terminals, read-only Watch, changes, test results, and history. Do not claim a full editable IDE, debugger, extension marketplace, or feature parity with another IDE. |

[Phase 8 acceptance](../phases/phase-8-acceptance-report.md) exercises collaboration with
scripted provider stand-ins. It supports the implemented workflow, not comparative model
quality, autonomous business outcomes, or measured time savings. This marketing work does
not run new provider sessions or reassess those desktop acceptance results.

## Validation and release boundary

Run existing website tests, type checks, production build/origin checks, formatting and
documentation-link checks. Inspect desktop and phone rendering, policy tabs and FAQ, hiring
example and workspace, automatic demo entry, static fallback and reopening. Container work
runs only in an isolated Coastline task environment.

The website CI and automatic updater previously checked the former headline literally. Their
page-identity checks now require the canonical Plenipo URL and a nonempty hero heading,
so both this page and an older rollback can pass without freezing marketing copy. They preserve
health, version, release-notes, source, image, restart, port, and unknown-route checks.
The updater change builds on merged PR #99; do not deploy a changed headline through an
old installed verifier. Release requires agreed file ownership and release order, the
persistent updater and allocation locks, exact candidate validation, current installer
metadata, and a verified rollback.

## Validation receipt — September 29, 2026

- PR #101 was reconciled with main `15046dc` as `b485f73`; incoming provider documentation
  and setup links are preserved. The website files and CI verifier are unchanged from the
  initial accepted candidate `1544bb5`.
- Isolated Coastline `pnpm check` passed on the initial candidate: version checks, formatting,
  lint, documentation links, type checks, 13 repository-script tests, 14 website tests,
  304 UI tests, and 422 desktop tests. Updater `bash -n` passed. After the main merge,
  documentation links passed again; exact-head GitHub checks run separately before integration.
- All 82 tracked website source files matched the preview's source hashes. Existing tests
  cover the automatic loader, retry, static fallback, asset budgets, links, metadata, and
  installer/version consistency.
- Chrome at 1920 × 911 and 390 × 844 showed the new copy without horizontal overflow.
  Policy tabs, policy FAQ, phone navigation, and the workspace layout passed. Fresh entry
  kept BODY focus and scroll zero while the demo opened; phone entry selected List.
  Static view and Enter reopening passed, returning focus to `demo-tab-team`. Console
  warnings/errors were empty. Browser screenshots are retained with the task's evidence.
- Actual Bash page-identity checks passed for the old live page and new built page, and
  correctly rejected a missing heading, blank heading, and wrong canonical URL.
- The complete current updater was rehearsed against the isolated preview on port 14382,
  using its own state directory and run lock plus the shared allocation lock. It successfully
  replaced the preview with exact `b485f73`, image
  `sha256:6ef7eb8cf40047b3c37838b98607a169fa25175475eab8c770ac0f058ff3a8d3`, retaining
  installer v1.11.0 and its notes. This validates the PR #99 updater changes together with
  the new page verifier. It is staging evidence, not a production release claim.

At this receipt, production still serves `1c5bc2b`, installer v1.11.0. Its installed updater
also matches `1c5bc2b`; a GitHub merge alone does not update that script. Production acceptance,
the final release identity, rollback retention, and preview cleanup must be recorded at release.
