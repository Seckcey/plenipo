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

## Owner

- [ ] Merge the pull request
- [ ] Install on Coastline and run the first update to v1.11.0
      ([steps](../development/website.md#install-it-on-coastline-once))
- [ ] Look at <https://plenipo.8westit.com/#whats-new>
