# Automatic homepage demo release - September 28, 2026

The maintainers requested the working demo as soon as the homepage opens. PR [#92](https://github.com/Seckcey/plenipo/pull/92)
was normally merged and the exact merge deployed to <https://plenipo.8westit.com/>. The demo starts
on entry without a click, scroll, or idle gate. Readable content remains during network transfer;
returning to static stays static until explicit reopening. Automatic loading leaves keyboard focus
and scroll alone. Explicit reopening focuses the sample only while the visitor still owns its button.

## Source, build, and runtime

- Merge/source: `0a018ad3bb61b461de76928e1aef377dc98839a7` (PR head `f758ee7c85ca5ef0169199e2e591c5ac2255cb0b`).
- Source archive SHA-256: `4c0f2f7787b3b75a21900977286a9ac07f6ad03b45ceba2091ab27769d85cb7d`.
- Source manifest SHA-256: `bbb61c075f608d45228b8653b1d5f84c771ad92167925afefe35bddde752dd30`; all 1,583 source files verified before and after build.
- Image: `plenipo-website:0a018ad3bb61b461de76928e1aef377dc98839a7`.
- Image ID: `sha256:0e8e56aadcd2dc6f8f58d5400516d3fb3e3827b5d2ced37cdb455c2986ffcd38`.
- Container: `1080d748fad193d5137912a687b1348315ca47b92f341e3a512ca3a1856adef8`.
- Host: verified `coastline`, the deploy account, Docker local Unix socket, Compose 5.3.1.
- Service: `/srv/8west/apps/plenipo-website`, project `plenipo-website`, loopback `127.0.0.1:14380` to port 8080.
- Network: `plenipo-website_default`, `10.204.229.0/28`; existing host `cloudflared` unchanged.
- Runtime: healthy, zero restarts, user `10001:10001`, read-only, 64 MiB limit, 0.5 CPU, no data mounts, `unless-stopped`.
- Applied and HTTP-verified at 2026-09-28 14:22:01 UTC; final browser/runtime acceptance at 14:24:24 UTC.

Build used explicit `PLENIPO_VERSION=1.6.0`. Download links, structured metadata, and release metadata
retain that version. This release makes no claim that v1.6.0 is the newest published installer.
Installer repair, desktop changes, external PR #88, provider connections, and customer data were excluded.

## Scope and acceptance

The final website source matches accepted `e08b0ac` exactly. The merged production build compared
all 62 served files with that preview: 61 were byte-identical; the only difference was the source
revision in `release.json`. The preview had already been compared with accepted `155ad42` plus only
the corrected loader: 61 exact files and one `index.html` whitespace-only difference. No unexplained
material difference was accepted. Full manifests and comparisons are retained with the release.

- Exact PR-head GitHub checks passed: website builds, documentation, frontend, Rust, Linux real-app E2E,
  and Windows build/installer/launch tests. CI run [36432104340](https://github.com/Seckcey/plenipo/actions/runs/36432104340)
  passed before normal merge. The separate post-merge default-branch run was still running at this receipt;
  coordinator tracking of that run does not change the deployed source identity.
- Local complete frontend checks passed: 304 UI, 263 desktop, 10 website, and 13 script tests;
  version/format/lint/types/docs checks passed. Rust format, strict clippy, 1,068 tests, and 241 unchanged
  generated binding files passed. The final focus-only change repeated all affected frontend/build checks.
- Origin and public health returned `ok`; release identity and v1.6.0 matched. All 30 checked asset URLs
  matched build bytes, and a missing route returned 404. Origin HTML matched exactly. Public HTML differed
  only by the existing Cloudflare analytics beacon and whitespace.
- Fresh public Chromium desktop navigation at 1920 px, before any click or scroll: six cards, one root,
  `BODY` focus, scroll zero. Fresh phone navigation at 390 px (375 px layout width): List selected, one
  root, `BODY` focus, scroll zero, no horizontal overflow.
- Native Enter after static return focused `demo-tab-team`. Conversation switching and sample
  decision/activity worked. Blocking the actual module produced readable fallback and retry; restoring
  it and retrying mounted one root with correct focus. Normal browser logs had no warnings/errors.
- No application/provider fetches were observed. Existing Cloudflare analytics was the only external
  resource. Intentional blocked-resource errors were kept separate from normal console acceptance.
- Coordinator independently checked the live six-card demo, selection, and empty warning/error log.
  Its reload preserved a pre-existing scroll position; it is not presented as fresh-navigation proof.
- Final server inspection counted 94 log lines with zero warning/error/critical/alert/emergency entries.

[Implementation acceptance](../../phases/website-autostart-acceptance.md) retains no-JavaScript,
reduced-motion, failure, slow-retry focus, and honest initial-load performance observations. Network
transfer is not instant; no zero-layout-shift or universal speed guarantee is claimed.

## Rollback and cleanup

Immediate rollback is source `155ad42cd7830976400bad4535beda5297de6435`, image ID
`sha256:ec9fa84e16458beda272619df3b0af3e727fe5a1a6db58b881926ceb85bc81c9`, with its original source and
configuration retained. The older `16cc6f3` release is retained too. The new release's `rollback.env`
SHA-256 is `1cad51ac4d191841987863d9ba8683549e166f70a8ae7bf684cc68fd036b9e31`.

At 14:24:44 UTC the task preview `plenipo-autostart-01a0e6d4`, its sole container, network, and own
14382 reservation were removed under the shared allocation lock. Coastline port 14382 was bindable;
production container, identity, and health stayed unchanged. The worker's exact SSH forward PID 32932
was stopped and Windows loopback 14382 was also bindable. Worker test tabs were closed; the coordinator closed its temporary preview tab and retained its public deliverable tab. All browser emulation overrides were reset.
No test containers remain for this task. Source, logs, build comparisons, screenshots, and rollback were retained.

Persistent lock inodes remain `6161580` (allocations) and `5785265` (heavy checks); both were released,
not deleted. Runtime evidence lives under
`/srv/8west/apps/plenipo-website/releases/0a018ad3bb61b461de76928e1aef377dc98839a7/`:
`build-receipt.json`, `merge-output-parity.json`, `release-record.json`, `runtime-final-check.json`,
and `evidence/browser-live.json` with screenshots. Preview cleanup evidence remains under
`/srv/8west/testing/plenipo-autostart-01a0e6d4/cleanup-receipt.json`.

The retained `public-desktop-entry.png` records the initial hero/demo header before interaction.
`coordinator-working-demo.png` shows the actual map and supervisor details;
`public-phone-list-visible.png` shows all six list entries after deliberate page scrolling.
These working-view screenshots are distinct from the no-input entry proofs.
