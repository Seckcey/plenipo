# Repository presentation validation

Review date: September 27, 2026. Base: `eb49e9f8e53e25ab7d66babd79b65dfeee6278f9`.
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
  approval-switch caveat. SECURITY.md now describes that caveat.
- GitHub reports Issues/Discussions enabled, wiki disabled, and private vulnerability reporting
  **disabled**. SECURITY.md and issue-template text no longer promise unavailable private intake.
  Enabling it or publishing an approved monitored contact remains an owner decision.

## Verification

The local `pnpm check` passed: version alignment, repository formatting, lint, typecheck, 240
shared UI tests, 175 desktop frontend tests, and 11 website tests. `git diff --check` passed.
A link audit checked 99 local documentation/image targets and anchors with no failures.
The public website, release page, latest-release redirect, and Discussions returned HTTP 200;
the issue-creation URL correctly redirects an unsigned-in visitor to GitHub login.

Both README logos match the approved source PNGs byte for byte, and the Organization capture
matches its acceptance-evidence source byte for byte. The social card is 1280 × 640 and 67,266
bytes. GitHub Settings displayed the uploaded Pip card successfully. About now names all five
tools and their concrete purpose; topics remain at 20, replacing redundant `tauri-app` with
`kimi-code`. The existing correct homepage was retained. No security setting was changed.

Actual GitHub README rendering is checked on the review branch before handoff.
Rust tooling is unavailable in the local documentation checkout; Rust, binding, installer and
real-app end-to-end validation must be assessed through exact-head GitHub CI. No desktop Docker,
Coastline preview, production release, or provider task is needed for this work.
