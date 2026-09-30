# Coordinated-team positioning release — September 30, 2026 UTC

The homepage now leads with **Hire your AI team. Give it a goal.** It explains roles,
coordinator handoffs across supported AI providers, suggested policies, owner-controlled model
and effort choices, and the supporting development workspace. A services-page example connects
the developer, reviewer, and writer. The README introduction uses the same positioning.

PR [#101](https://github.com/Seckcey/plenipo/pull/101) was normally merged and its exact merge
deployed to <https://plenipo.8westit.com/>. Pip, the visual system, automatic interactive sample,
sample disclosures, edition plans, installer templating, and desktop behavior are preserved.
The [claim map and staging receipt](../website-positioning.md) describe the evidence and wording boundaries.

## Source, build, and runtime

- Reviewed PR head: `0fde8bd0463900eea364928fda9aa6ab0f5dc22e`.
- Merge/source: `ae7356061eadc9ea498f1e3b88ff87541f37d75d`, parents `9d8a137` and `0fde8bd`.
- Deployed: September 30, 2026 at 00:28:24 UTC (September 29 in Pacific time).
- Published installer: v1.11.0, verified as the latest full release at deployment; its notes are included.
- Host/project: verified `coastline`, deploy account `seckcey`, local Docker Unix socket,
  Compose project `plenipo-website`, `/srv/8west/apps/plenipo-website`.
- Image: `plenipo-website:ae7356061eadc9ea498f1e3b88ff87541f37d75d-v1.11.0`.
- Image ID: `sha256:9d58693866a22d0c625e9912b29edd0c5afd7b65f5c2e1f57ee3bca62338ea8d`.
- Container: `06d3fca7aa0f4fdc142224229ff5beca2d07fd0db77363444e8f9ff6606625dc`.
- Binding: `127.0.0.1:14380` to container 8080; existing network `10.204.229.0/28`.
- Runtime: healthy, zero restarts, user `10001:10001`, read-only, 64 MiB memory, 0.5 CPU,
  no data mounts, `unless-stopped`. No warning/error log lines were found at acceptance.

Website source and website CI match the reviewed head exactly. The intervening PR #102 changes
only two Phase 16 documents. Exact merged staging compared all 62 built files with the accepted
preview: 61 were identical and only the source revision in `release.json` differed. All 62 files
in the production image then matched exact merged staging. Image IDs are recorded separately
because rebuilding identical served files can produce a different image identity.

## Verification and updater installation

All seven exact-head PR checks passed: website build/origin, documentation, changed-file
classification, frontend, Rust, Windows build/installer/launch, and Linux real-app end-to-end tests.
Full CI run [36647792148](https://github.com/Seckcey/plenipo/actions/runs/36647792148) and
website run [36647792230](https://github.com/Seckcey/plenipo/actions/runs/36647792230) were green
before merge. At this receipt, default-branch website run
[36650363620](https://github.com/Seckcey/plenipo/actions/runs/36650363620) passed and full run
[36650363617](https://github.com/Seckcey/plenipo/actions/runs/36650363617) was still running,
tracked separately by the coordinator. Passing PR checks are not presented as a completed default-branch run.

- Origin and public health returned `ok`; source, version, and notes metadata matched.
  Unknown routes returned 404. All 58 checked static assets matched image bytes at both origin
  and public URLs. Origin HTML matched the image exactly; public copy and canonical identity
  were checked separately because the existing Cloudflare edge may inject analytics.
- Public Chrome desktop entry had six demo nodes, BODY focus, scroll zero, and no horizontal
  overflow. The policy tab worked. Fresh 390 × 844 phone entry had BODY focus, scroll zero,
  no overflow, and List selected. Browser warning/error logs were empty. Desktop and phone
  screenshots were reviewed; temporary viewport overrides were reset.
- The page verifier now checks the canonical URL and a nonempty hero heading. Old and new pages
  pass; missing/blank headings and a wrong canonical URL fail. Other health, version, source,
  image, restart, port, notes, and 404 checks remain intact.

Before this release, the existing updater had independently advanced production to `9d8a137`
with the reviewed PR #99 release-notes changes. Its installed SHA-256 was
`a6aa503c7cf05e5da50d14e233b80abbe8ba745661adce2de7a214bb90d6b019`.
That actual baseline was preserved for rollback; the earlier staging baseline was not restored over it.

The release used the original updater lock for the entire operation. A task-local execution copy
added a preflight/backup hook after successful lock acquisition and an installation hook after
successful deployment verification and recording. All original updater logic remained intact.
The exact unmodified merged updater was installed atomically before releasing the lock; its SHA-256 is
`16e8ac4a7d0675a8c3fed2d6e2b1306b24ecff4de0860f64a3f3dec681de90c4`.
The timer stayed active and enabled. No sudo, timer configuration change, migration, or unrelated service restart was needed.

Isolated rehearsals proved successful deployment, contention refusal, distinct-image rollback
after controlled verification and installation failures, rejection of mismatched rollback metadata,
and preservation of unexpected installed-script bytes. No failure-injection flags were set in production.
The persistent lock inodes stayed `6423019` (updater) and `6161580` (allocations); neither was deleted or replaced.

## Rollback, evidence, and cleanup

Immediate rollback remains source `9d8a137704d17a46d459b51bcb69cf30464984c6`, image
`plenipo-website:9d8a137704d17a46d459b51bcb69cf30464984c6-v1.11.0`, image ID
`sha256:13066806101da6387ad50a3e0624cde024a657f377880cfc2c0dacc5d4edd955`.
The original release directory and Compose files remain. Previous updater bytes, `current.env`,
release metadata, image identity, and installation receipt are copied into durable evidence:

`/srv/8west/apps/plenipo-website/auto-release/evidence/ae7356061eadc9ea498f1e3b88ff87541f37d75d-positioning/`

That directory also retains `runtime-receipt.json`, `public-acceptance.json`, `manifest-production.txt`,
merged-stage parity, distinct-artifact rollback evidence, helper scripts/diff, release logs,
desktop/phone screenshots, and `cleanup-receipt.json`. Production allocation metadata points to
the current runtime receipt. Application data, Cloudflare, DNS, and unrelated services were untouched.

At 00:31:17 UTC the task preview `plenipo-positioning-01a0ef6d`, its sole container/network,
and its own 14382 reservation were removed under the shared allocation lock. The port was bindable
and the production container remained unchanged. The exact task-owned Windows SSH forward,
PID 14844, was stopped after its command line was verified; Windows loopback 14382 was also bindable.
No task preview containers remain. Source, images, logs, and rollback material are retained.
The worker's browser tab now shows the public deliverable; no preview tab or viewport override remains.
