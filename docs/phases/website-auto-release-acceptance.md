# Website follows new releases — acceptance report

[ADR-069 (the website follows new releases)](../adr/ADR-069-website-follows-releases.md).
Checklist: [website-auto-release-checklist.md](website-auto-release-checklist.md).

## What was tested, and where

Before the merge, the whole update was run against a stand-in on 2026-09-29 (the cloud session
that built it cannot reach Coastline): the same Docker Compose files, project name
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

## The first run on Coastline, 2026-09-29

After the merge, the script and timer were installed on Coastline with the guide's steps.

| Step                         | Result                                                                                                                                                |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--check`                    | Found v1.11.0 with the site on 1.6.0; planned website code `main` at `1c5bc2b`; changed nothing                                                       |
| First update, 20:16 UTC      | Built and started `plenipo-website:1c5bc2b…-v1.11.0` (image `sha256:0637b68e…`) in 12 seconds; every check passed; the previous image was `0a018ad`   |
| Public check                 | <https://plenipo.8westit.com/release.json> shows 1.11.0, source `1c5bc2b`, `releaseNotes: true`; the page shows "What's new in v1.11.0" and its title |
| Timer run as the deploy user | After the fix below: "Up to date. Nothing to do.", exit 0                                                                                             |
| Timer                        | Next run 15 minutes later, then every 15 minutes                                                                                                      |

## Found on Coastline, and fixed

- **The first update was run as root.** Its files (the local copy of the repository, the state
  folder, and the release folder) then belonged to root, and the timer, which runs as the
  folder's owner, could not use them. The files were given back to the deploy account and the
  timer's next run passed. Now the script refuses to run as root, the install steps run it as the
  folder's owner (`sudo -u`), and the service file takes that account from the install steps
  instead of naming one.
- **The release notes talked to one person.** "Accepted by you", "your choice", and "not yet
  checked on Windows by you" read wrongly on a public page, and on a public repository. The notes
  from v0.6.1 to v1.13.0 were reworded to describe Plenipo and name no one (ADR-069, point 8). So
  the website shows the new wording, it now reads the notes from `main` and updates when they
  change; the Release notes workflow does the same for the GitHub release pages. Test data that
  used a real person's name and addresses now uses a made-up "Alex Rivera"; the company domain
  in the tests stays, because the tests match addresses by it.

## Not yet checked

- The first update that shows the reworded notes (the next timer run after this is merged), and
  the first Release notes workflow run.
- The first update for a new release made after this (v1.12.0 or later).
