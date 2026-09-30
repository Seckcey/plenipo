# ADR-069: The website follows new releases by itself, with each release's notes

- **Status:** Accepted (2026-09-29; option 1 of the three below)
- **Date:** 2026-09-29; amended the same day after the first run on Coastline
- **Phase:** Website (follow-up to the automatic homepage demo release)
- **Changes:** [the website's deploy steps](../development/website.md#reserve-deploy-and-roll-back),
  which were done by hand for every release

## Context

The website at <https://plenipo.8westit.com> runs on Coastline. Its version (the download
buttons, the version labels, the structured data, and `release.json`) is filled in when its
image is built, and until now a person logged in to Coastline and built and started it by hand.
So the site fell behind: on 2026-09-29 it still showed v1.6.0 while v1.11.0 was the latest
release. The page also had no release notes of its own; it only linked to GitHub.

Three ways to update it automatically were weighed:

1. **Coastline checks GitHub on a timer** and updates itself when a new release appears.
2. **A GitHub Actions runner on Coastline** (GitHub's program that runs workflow jobs), started
   by the Release workflow. Updates within seconds, but the repository is public, and GitHub
   advises against runners on a public repository's own machines: a workflow in a pull request
   could run code on Coastline.
3. **GitHub logs in to Coastline** over SSH through the Cloudflare Tunnel. Needs a key for
   Coastline stored in GitHub and Tunnel access for it.

## Decision

1. **Coastline checks for itself.** A systemd timer runs
   [`apps/website/deploy/auto-release.sh`](../../apps/website/deploy/auto-release.sh) every 15
   minutes as the deploy user. It only reads from GitHub (the public releases API and a public
   clone), so GitHub holds no login for Coastline, nothing new is opened on Coastline, and
   nothing on GitHub can send Coastline a command. A new release shows on the site within about
   15 minutes.
2. **What it shows.** The latest published release: never a draft or a pre-release (the same
   rule installed copies follow for updates, ADR-038), and only when its Windows installer is
   attached and its download link works.
3. **What it builds.** The website code from `main`, the reviewed website, with the release's
   version and its notes, `docs/releases/vX.Y.Z.md` as it reads on `main` (from the release's tag
   only if `main` has none). A correction to the notes merged after a release is shown within 15
   minutes, because a run also updates the site when the shown release's notes changed. A
   website code fix merged after a release originally showed with the next release (or at once
   with `--force`). The September 30 update,
   [publish checked website changes](website-changes-follow-checks.md), makes those changes
   automatic too after GitHub checks pass, once the updated script is installed on Coastline.
4. **Release notes on the page.** A "What's new in vX.Y.Z" section above Download shows the
   notes' title and opening paragraphs, with the rest under **Read the full release notes**. The
   notes are rendered as plain text with a few shapes (headings, paragraphs, lists, code, bold,
   links); no HTML in them reaches the page, and only http(s) links become links.
   `release.json` says whether the page has notes (`releaseNotes`).
5. **Safe to run unattended.** Each release goes into a new folder that is never edited, the
   container is swapped while holding the shared port-allocation lock, and the new site is
   checked: health, the home page, "What's new" for this version, `release.json`'s version,
   source, and notes, a real 404, the image it runs, its restart count, and a loopback-only
   port. If a check fails, the previous image is put back and checked again, and the run is
   recorded as rolled back. Every run is logged to the systemd journal, and every update to
   `history.jsonl`. It never prunes images, stops other services, or touches the firewall, DNS,
   or the Tunnel.
6. **Never as root.** The script and its timer run as the account that owns the app folder. The
   script refuses to run as root, because files made by root lock that account out (found on the
   first run, below).
7. **Release pages follow the notes too.** The Release notes workflow
   (`.github/workflows/release-notes.yml`) puts each `docs/releases/vX.Y.Z.md` on its GitHub
   release page when the notes change on `main`, or when run by hand. It changes only titles and
   notes, never files, tags, or signatures.
8. **Notes are written for Plenipo's users.** They are public on the website and on GitHub, so
   they describe Plenipo, and name no maintainer or person: "accepted", not "accepted by you";
   "not yet checked on a real Windows PC", not "please try on your PC".

9. **Pages on GitHub never go stale either.** The README shows a latest-release badge (shields.io
   reads GitHub Releases each time the page is shown) instead of a typed version, and points to
   GitHub Releases and `docs/releases` instead of keeping its own list of versions. The README,
   `SUPPORT.md`, `SECURITY.md`, `CONTRIBUTING.md`, `docs/faq.md`, and `docs/roadmap.md` never
   name the latest version by hand; `pnpm versions:check` fails if one does. Nothing commits to
   the repository on its own after a release.

## Consequences

- Releasing no longer needs a second, manual website step. A maintainer installs the timer once.
- Up to 15 minutes between a release and the site showing it. `auto-release.sh` can be run by
  hand to update at once.
- The release notes are now on the public website as well as on GitHub. The earlier notes (v0.6.1
  to v1.13.0) were reworded on 2026-09-29 to name no one; the website and, after the Release
  notes workflow runs, the release pages show the new wording. The notes inside v1.11.0's
  `latest.json` (what installed copies show when offering that update) keep the old wording: that
  file is part of the published release that installed copies read, and is left as published.
- A failed update leaves the previous release running and says so in the journal; nobody is
  told by email or phone yet. `systemctl status plenipo-website-update` shows the last run.
- The script on Coastline is an installed copy. A change to it in the repository reaches
  Coastline only when it is copied again (the install steps in the website guide), so a merge
  alone never changes what runs as the deploy user.
