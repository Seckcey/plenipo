# Website follows new releases — acceptance report

[ADR-069 (the website follows new releases)](../adr/ADR-069-website-follows-releases.md).
Checklist: [website-auto-release-checklist.md](website-auto-release-checklist.md).

## What was tested, and where

Coastline cannot be reached from the cloud session that built this, so the whole update was run
against a stand-in on 2026-09-29: the same Docker Compose files, project name
(`plenipo-website`), port (`127.0.0.1:14380`), subnet (`10.204.229.0/28`), and an allocation lock
file, with the real GitHub releases API. The stand-in started from today's live site: the image
of `0a018ad` built with v1.6.0, whose `release.json` matched production's.

One difference, for the test only: the session's network goes through a proxy with its own
certificate, so a small test wrapper gave `docker build` that certificate. The script itself was
not changed for the test, and Coastline needs no wrapper.

## Results

| Test                                        | Result                                                                                                                                           |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `--check` from v1.6.0                       | Found v1.11.0 and its installer; planned the folder and image; changed nothing                                                                   |
| First update, v1.6.0 → v1.11.0              | Built, swapped, and passed every check in 15 seconds; `release.json` shows 1.11.0 with `releaseNotes: true`                                      |
| Second run                                  | "Up to date. Nothing to do."                                                                                                                     |
| Broken build (`--force` with a broken page) | Caught "home page is missing its heading", put v1.11.0 back, healthy in 6 seconds; exit 1; recorded as `rolled-back`                             |
| Page at 1440 px and 390 px                  | "What's new in v1.11.0", the release's title and opening paragraphs, **Read the full release notes** opens; no sideways scroll or console errors |
| Website tests, formatting, shellcheck       | 14 tests pass; clean                                                                                                                             |

## Found and fixed while testing

- GitHub's installer link redirects to a signed storage link that may refuse a headers-only
  request. The check now downloads one byte instead.
- The version showed twice ("What's new in v1.11.0" over "v1.11.0 — The organization canvas").
  The notes' title now drops the repeated version.

## Not yet checked

- The real run on Coastline (the owner's steps in the
  [website guide](../development/website.md#install-it-on-coastline-once)): that `jq` is there,
  that `seckcey` can write to the app folder, and that the timer fires.
- Public acceptance through the Cloudflare Tunnel after that run.
