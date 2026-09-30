# Automatic website changes — checklist and acceptance

Date: September 30, 2026. Plenipo is made by 8 West Ventures, LLC.

The owner authorized automatic website updates using the existing Coastline timer.
The decision is
[publish checked website changes](../adr/website-changes-follow-checks.md).

## Checklist

- [x] Detect website and Website workflow changes without a new installer release.
- [x] Avoid rebuilding for unrelated app changes.
- [x] Wait for exact-source CI and checks for the website files being built.
- [x] Keep the current site running for pending, failed, or unavailable checks.
- [x] Preserve published version selection, immutable exports, locks, and rollback.
- [x] Check terms and privacy pages before accepting a deployment.
- [x] Document an atomic, pinned upgrade for the already installed updater.
- [x] Complete repository validation and compare generated bindings.
- [ ] Publish the reviewed automation changes.
- [ ] Install the updated script on Coastline and verify the timer.
- [ ] Verify both policy pages on the public website.

## Validation

The Linux updater tests use real local Git history, a bare fetch, source exports, content
hashes, and deployment records. Only external HTTP responses, Docker, and the UID
check are replaced. They cover website-only and workflow changes, unrelated app
changes, corrected notes with earlier website checks, queued and failing CI,
cancelled or missing runs, wrong source revisions, an API error, a successful
deployment, and rollback for missing or misrouted policy pages. They build no real
containers and contact no real service.

Container lookup uses the app's Compose labels rather than an implicit Compose file.
This lets the systemd service find the previous image and verify the new container
even when its working directory is not the app folder. The Docker stand-in rejects
an implicit `compose ps` lookup, so successful deployment and rollback exercise this.

GitHub's Website workflow also requests both policies from the production container
and checks their canonical identity and contact link. This complements the static
build tests and the updater's origin checks.

Local validation passed: `pnpm check` (812 UI tests), all 30 website tests including
15 updater scenarios, Bash syntax, Rust formatting and strict Clippy, 1,552 Rust
tests, and binding export. All 340 tracked generated bindings matched the exports.
No real container was built or run in this cloud workspace. GitHub's website checks
and the server installation are separate acceptance steps.

## Installation status

This chat provides a cloud Linux workspace. Its runtime has no configured Tailscale
connection, TCP grant, or Coastline credential. No local laptop-control tool is
available. The owner can SSH from their laptop; that connection is not attached to
this session. No server installation or public deployment is claimed by this record.

After the reviewed changes reach `main`, use
[the one-time upgrade](../development/website.md#upgrade-an-existing-updater)
from the laptop's Coastline connection. GitHub publication alone cannot replace the
installed script. Record the installed revision, timer state, public source revision,
and both policy-page responses after a real server run.
