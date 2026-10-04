# Versioning

Plenipo uses [Semantic Versioning 2.0.0](https://semver.org) with a **single product version**
shared by every package and crate in the repository.

## Source of truth

The root `package.json` `version` is authoritative. These must match it:

| File                                     | Field                                                                     |
| ---------------------------------------- | ------------------------------------------------------------------------- |
| `apps/desktop/package.json`              | `version`                                                                 |
| `apps/remote/package.json`               | `version`                                                                 |
| `packages/types/package.json`            | `version`                                                                 |
| `packages/ui/package.json`               | `version`                                                                 |
| `tests/e2e/package.json`                 | `version`                                                                 |
| `Cargo.toml`                             | `[workspace.package] version` (inherited by all crates)                   |
| `apps/desktop/src-tauri/tauri.conf.json` | `version` must be `"../package.json"` (reads the desktop package version) |
| `apps/website/index.html`                | carries `__PLENIPO_VERSION__`; the website build fills it in              |

`pnpm versions:check` (run in CI) fails if any of these disagree, or if a version is typed into
the website's page by hand (a download link, a version label, or the structured data). The
website's build (`apps/website/scripts/build.mjs`) reads the root `package.json`, or
`PLENIPO_VERSION` when it is given: a container build has no repository around it and stops
without it. A version on `main` is not yet an installer on GitHub Releases, so the website is
deployed with the release that is published ([the website](website.md)).

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

After the MVP:

| Release                                                                              | Version  |
| ------------------------------------------------------------------------------------ | -------- |
| Grok joins the AI tools (ADR-015)                                                    | `1.1.0`  |
| Ollama's cloud models join the AI tools (ADR-017)                                    | `1.2.0`  |
| Phase 10 — Browser automation and computer use (Phase 9 postponed)                   | `1.3.0`  |
| Switches in Settings and workers learning (ADR-023, ADR-024)                         | `1.4.0`  |
| Kimi joins the AI tools (ADR-027)                                                    | `1.5.0`  |
| Phase 11 — Servers over SSH (ADR-025, ADR-026)                                       | `1.6.0`  |
| Phase 12A — Visual design system (ADR-030)                                           | `1.7.0`  |
| Phase 12 — Home, the pages, the terminal, notices (ADR-031, 033)                     | `1.8.0`  |
| Phase 13 — Installer, updates, and recovery (ADR-037, ADR-038)                       | `1.9.0`  |
| Phase 17 — The owner's control over workers (ADR-041 to ADR-045)                     | `1.10.0` |
| Phase 18 — The organization canvas (ADR-053 to ADR-056)                              | `1.11.0` |
| Phase 19 — The AI tools page (ADR-058 to ADR-060)                                    | `1.12.0` |
| Phase 20A — Connections: Microsoft 365 (ADR-061 to ADR-068)                          | `1.13.0` |
| Phase 16 Wave 1 — Who made each model; Antigravity (ADR-080–082)                     | `1.14.0` |
| Phase 20B — Connections: Slack and Google (ADR-070)                                  | `1.14.1` |
| Phase 20C — Connections: HubSpot, Stripe, website, add-ons (ADR-071)                 | `1.14.2` |
| Fix — WooCommerce keys: add one later, and why one is refused                        | `1.14.3` |
| Phase 16 Wave 2 — GitHub Copilot; Cursor's finding (ADR-083, 084)                    | `1.15.0` |
| Phase 21 — Panels, files, and more than one organization (ADR-091–094)               | `1.16.0` |
| Phase 16 Wave 3 — Paid AI keys, spending caps, OpenRouter, direct keys (ADR-085–087) | `1.17.0` |
| Phase 11A — Free and Pro editions and the license key (ADR-021, 022, 100–118)        | `1.18.0` |
| Fix — A key box on every AI tool card; no spending cap needed (ADR-085)              | `1.18.1` |
| Phase 14, part 14A — Plenipo on your phone: the sealed line and approvals            | `1.19.0` |
| Phase 14, part 14B — Plenipo on your phone: everything else that is safe             | `1.19.1` |
| Phase 14, part 14C — Notices when the phone's page is closed (Phase 14 done)         | `1.19.2` |
| Plenipo on your phone: the relay, built in this repository (ADR-149)                 | `1.19.3` |
| Plenipo on your phone is on: 8 West's relay is live (ADR-149)                        | `1.19.4` |
| Phase 23, Waves 0 and 1 — Getting ready for Mac and Linux                            | `1.20.0` |
| Phase 23, Wave 1 — Mac and Linux: the shared base (ADR-158)                          | `1.21.0` |
| Phase 24, part 24C — Community, the people part (ADR-170, ADR-171)                   | `1.22.0` |
| Phase 23, Wave 2 — Linux, first look (ADR-152, ADR-154)                              | `1.23.0` |
| A live chat with each agent (ADR-200–202), and security fixes (ADR-211, 212)         | `1.24.0` |
| Phase 25 — Fixes and a simpler Plenipo; Guard asks first (ADR-190–199, 213, 250–259) | `1.25.0` |

## Releasing

1. Update the version in the files above (and `Cargo.lock`: `cargo update --workspace`). The
   website's page needs no change; its build fills the version in.
2. Run `pnpm versions:check`.
3. Add release notes at `docs/releases/vX.Y.Z.md` (first line `# <release title>`).
4. Commit as `chore(release): vX.Y.Z` and merge to `main`.
5. Start the release, either way:
   - **Run workflow (no tag push needed):** GitHub → **Actions** → **Release** → **Run workflow**
     on `main`. It releases the version in `package.json`: it checks the version and release
     notes, builds the installer, and only then creates the tag `vX.Y.Z` on that commit and
     publishes the release. It refuses a version that is already tagged or released.
   - **Tag push:** tag the merge commit `vX.Y.Z` and push the tag.
6. **Approve the run.** Its job waits until you do: open the run under **Actions → Release**,
   click **Review deployments**, tick `release`, and click **Approve and deploy** (ADR-052,
   signing runs only for main and release tags, behind the owner's approval; where to click is
   in [code signing → approving a release run](code-signing.md#approving-a-release-run)). Reject
   a run you did not start.
7. Nothing else to update by hand. Once the release is published, these follow it by themselves
   ([ADR-069, the website follows new releases](../adr/ADR-069-website-follows-releases.md)):
   - **the website**, within 15 minutes: its version, download buttons, and "What's new" from
     `docs/releases/vX.Y.Z.md` ([automatic updates](website.md#automatic-updates));
   - **the README**: its latest-release badge reads GitHub Releases each time the page is shown;
   - **the release pages**: the Release notes workflow puts `docs/releases/vX.Y.Z.md` on each
     release page whenever the notes change on `main`.

   So pages people read on GitHub never type which version is the latest (the README, `SUPPORT.md`,
   `SECURITY.md`, `CONTRIBUTING.md`, `docs/faq.md`, `docs/roadmap.md`): they link to the latest
   release, or show its badge. `pnpm versions:check` fails if one names the latest version by
   hand.

`.github/workflows/release.yml` runs on Windows: it checks that the version matches and that
release notes exist, builds the NSIS installer signed as 8 West Ventures, LLC
([code signing](code-signing.md)), checks the signature, and publishes a GitHub release with the
installer attached. From 1.9.0 it also signs the installer with the updater key and attaches
its `.sig` and `latest.json`, which installed copies read to find the new version (ADR-038,
updates; [code signing](code-signing.md#updates-the-updater-key-phase-13-adr-038)). From 1.23.0 a
Linux job builds the `.deb` and the AppImage on Ubuntu 22.04 with no secret, and checks that they
are this version's, that the `.deb` installs with `apt`, that both start and keep running, and
that `apt` removes Plenipo cleanly (`scripts/linux-package-check.sh`, Phase 23, ADR-152). After
the owner's one approval, the Windows job signs the AppImage with the same updater key, writes
one `latest.json` listing both systems (`scripts/update-manifest.mjs`), and publishes the
installer, the `.deb`, the AppImage, and its `.sig` together, so a release never has half its
files. `0.x` versions and SemVer pre-releases are published as GitHub pre-releases.
The signing secrets are Environment secrets of `release`, which only `main` and `v*` tags may use
([code signing → where the secrets live](code-signing.md#where-the-secrets-live-the-release-environment)).
**Run workflow** with **Dry run** ticked builds an unsigned installer from any branch, needs no
approval, touches no secret, and publishes nothing; it also builds and checks the Linux files and
a trial Mac app, kept as downloads for 7 days.

The workflows (`ci.yml`, `release.yml`, `website.yml`) name each GitHub Action they use by the
exact commit it runs, not by a tag that someone could move to other code, and Dependabot
(`.github/dependabot.yml`) opens a pull request each week when a newer release of an action, an
npm package, or a Rust crate is out.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org): `feat:`, `fix:`, `docs:`,
`chore:`, `refactor:`, `test:`, `ci:`, with an optional scope (`feat(core): …`).
