# Repository presentation validation

Review date: September 27, 2026. Base: `eb49e9f8e53e25ab7d66babd79b65dfeee6278f9`.
Reconciled with main `30a77643760dfae4bab09593535a611a4d8f37dd` before final verification.
Scope: repository docs, copied presentation assets, and GitHub About/topics/social preview.
No app, website, dependency, workflow, license, or production changes are part of this work.

## Evidence and claim corrections

- GitHub Releases API reports v1.6.0 as latest, published September 27, with the uploaded
  `Plenipo_1.6.0_x64-setup.exe` (7,754,640 bytes). Main contains v1.7.0 development. The README
  now distinguishes those states and links to the release's known limits. No local installer run
  is claimed by this documentation work.
- `docs/development/setup.md` and runtime adapter sources describe account sign-ins, five tools,
  Ollama cloud-only text conversations, Windows 11 x64, and provider-specific restrictions.
  README/FAQ replace the previous blanket “no cloud” and “your work never leaves” claims.
- `docs/editions.md` explicitly describes planned editions, not shipped enforcement. README moves
  that distinction ahead of any pricing detail and leaves the plan and license unchanged.
- The real Phase 12A Organization capture replaces the “screenshots coming” placeholder, with a
  development-version and synthetic-data caption. Approved Pip logos are copied, not redesigned.
- Guard defaults in `crates/guard/src/dto.rs` and switch handling in `engine.rs` establish the
  approval-switch caveat. SECURITY.md now describes that caveat. Already-merged ADR-029 and the
  browser broker also contradict the inherited absolute CAPTCHA guarantee; the policy now
  describes that limitation without providing operational instructions or importing PR #36.
  FAQ and security wording account for ongoing CAPTCHA improvements: results depend on the
  website, settings, and installed version; release notes establish what has shipped. No
  unverified success rate or pending improvement is presented as a published capability.
- GitHub reports Issues/Discussions enabled, wiki disabled, and private vulnerability reporting
  **disabled**. SECURITY.md and issue-template text no longer promise unavailable private intake.
  Enabling it or publishing an approved monitored contact remains an owner decision.

## Verification

The local `pnpm check` passed: version alignment, repository formatting, lint, typecheck, 240
shared UI tests, 175 desktop frontend tests, and 2 website tests. `git diff --check` passed.
A final link audit checked 100 local documentation/image targets and anchors with no failures.
The public website, release page, latest-release redirect, and Discussions returned HTTP 200;
the issue-creation URL correctly redirects an unsigned-in visitor to GitHub login.

Both README logos match the approved source PNGs byte for byte, and the Organization capture
matches its acceptance-evidence source byte for byte. The social card is 1280 × 640 and 67,266
bytes. GitHub Settings displayed the uploaded Pip card successfully. About now names all five
tools and their concrete purpose; topics remain at 20, replacing redundant `tauri-app` with
`kimi-code`. The existing correct homepage was retained. No security setting was changed.

Actual GitHub README rendering was checked on `codex/github-discoverability` at initial head
`1606b63`: Pip header in both light and dark themes, loaded image dimensions and selected source,
real screenshot and caption, release guidance, tables, links, and rendered Mermaid hierarchy.
Browser theme emulation is temporary and does not change account preferences.
Durable captures: [light README](evidence/github-readme-light.png),
[dark README](evidence/github-readme-dark.png),
[installation guidance](evidence/github-readme-install.png), and
[uploaded social preview](evidence/github-social-preview.png).

Initial push occurred after local `pnpm check`, but before the repository-mandated Rust checks,
under coordination guidance that was subsequently corrected. Full Rust/bindings verification is
required before a further push and final integration; no pre-push Rust pass is claimed retroactively.
Rust tooling is unavailable in the local documentation checkout. The required isolated Coastline
run passed before the final push, on source `96d46029a5aa7d3edc88e40c02051797453b2b86`
(tree `e4ebe822a20d0c5c2156589532149de359464e6e`):

- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`: **765 passed, 0 failed, 0 ignored**
- `pnpm bindings`: **189 passed**, with no generated-file content or filename changes

The run completed September 27 at 14:37:27 UTC with Rust 1.98.1, Debian Chromium 154, one
build/test thread, a 2 GiB memory limit, and two CPUs. It held the shared test lock and used its
own source, copied build cache, runtime, and container. A copied-cache path mismatch was fixed
in the temporary test setup before the successful full run; no application source was changed.
The test container was removed and the shared lock verified free. Only this receipt and copies
of the reviewed PNG evidence follow that tested source. Final formatting and diff checks passed.

All eight initial-head GitHub checks passed, including Windows installer/smoke and Linux real-app
end-to-end checks. Final-head GitHub CI remains the coordinator's merge gate. No desktop Docker,
production release, or live provider task is part of this validation.
