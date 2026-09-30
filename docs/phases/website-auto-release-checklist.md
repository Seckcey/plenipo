# Website follows new releases — checklist

[ADR-069 (the website follows new releases)](../adr/ADR-069-website-follows-releases.md). Report:
[acceptance](website-auto-release-acceptance.md).

## Built

- [x] "What's new in vX.Y.Z" section above Download, from the release's own notes
      (`apps/website/scripts/release-notes.mjs`), with the rest under **Read the full release
      notes**
- [x] Notes are escaped text: no HTML reaches the page; only http(s) links become links;
      relative links point at the release's tag on GitHub
- [x] `release.json` says whether the page has notes (`releaseNotes`)
- [x] A build without notes for its version links to the release on GitHub instead
- [x] The image build copies `apps/website/release-notes/` (empty in the repository)
- [x] `apps/website/deploy/auto-release.sh`: latest published release → build from `main` with
      its version and notes → swap under the allocation lock → checks → roll back on failure
- [x] `--check`, `--force`, `--version`, `--source-ref`; one run at a time
- [x] systemd service and 15-minute timer (`apps/website/deploy/`)
- [x] Website workflow checks "What's new" names the version
- [x] Website guide: how it works, install once, check on it, pause it, go back

## Checked

- [x] Website tests (14), formatting, shellcheck
- [x] End to end against a stand-in for Coastline, with the real GitHub release v1.11.0
- [x] Up to date: a second run changes nothing
- [x] A broken build is caught and the previous release comes back by itself
- [x] Desktop and phone layout, no console errors

## On Coastline

- [x] Merged ([Seckcey/plenipo#98](https://github.com/Seckcey/plenipo/pull/98))
- [x] Installed, and the first update to v1.11.0 run: live at
      <https://getplenipo.com/#whats-new>
- [x] Timer on, running as the deploy account, every 15 minutes

## Follow-up (the same day)

- [x] The script refuses to run as root; the install steps run it as the folder's owner
- [x] The service file names no account; the install steps fill it in
- [x] Release notes v0.6.1 to v1.13.0 name no one, and talk to Plenipo's users only
- [x] The website reads the notes from `main`, and updates when the shown release's notes change
- [x] Release notes workflow: GitHub release pages follow `docs/releases`
- [x] Test data uses a made-up person, not a real one
- [ ] After merge: the Release notes workflow runs by itself (the notes changed on `main`); check
      its log, and that the next timer run shows the new wording on the website
