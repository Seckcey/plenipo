# Website domain migration: checklist and acceptance

Date: September 30, 2026. Plenipo is made by 8 West Ventures, LLC.
Authority: [ADR-130 (the website address move)](../adr/ADR-130-website-domain-migration.md).

## Scope

Make `https://getplenipo.com` the canonical website. Keep the same product, design,
demo, published Windows installer, and Coastline website service. Send the old website
and `www` URLs to the apex with their paths and query strings. Keep the account service,
mail records, publisher identity, and OAuth settings separate.

## Release checklist

- [x] Inspect the exact source, live service, tunnel, and concurrent ownership.
- [x] Obtain the automatic-updater owner's file and installation handoff.
- [x] Add and verify apex and www HTTPS routes before redirecting the old host.
- [x] Confirm the DNS change adds only the two website records.
- [x] Update website metadata, policies, sitemap, robots, social card, and current links.
- [x] Add checks for preserved redirect paths/queries and exact-source canonical identity.
- [x] Complete repository checks, exact-source staging, and rollback rehearsals.
- [x] Obtain coordinator review and merge with the persistent updater lock held.
- [x] Verify merged-source CI and staging parity; receive the release handoff.
- [x] Deploy and install the exact updater under the same continuous lock.
- [x] Verify public redirects, content, metadata, assets, browser flows, and account isolation.
- [x] Update GitHub homepage and Microsoft homepage/terms/privacy URLs.
- [x] Retain rollback and evidence, clean up owned previews, and complete documentation.

## Initial routing evidence

The existing Cloudflare account is `a4848d680c388d8c5d2b83b7918d0184`. Its healthy
`8west-lab` tunnel is `6146d514-81f7-4e1e-8f14-2acefd2d1dcf`. The old website route
uses `http://localhost:14380`. The new apex and www routes use `http://127.0.0.1:14380`
with no origin overrides. The prior 44 routes remain; the website additions are routes
45 and 46. The domain's seven mail/verification records remain unchanged.

Both new hosts returned HTTPS 200 for website content before source release. The
website then showed source `8b32203cd5825a3b488c6f5a88b83b82ea118c0f`, installer
`1.17.0`, after the normal updater advanced it during inspection. This proves routing,
not completion of the canonical-host migration. At this baseline,
`account.getplenipo.com` had no DNS record. No account route was created or redirected.

## Source and test receipt

[PR #118](https://github.com/Seckcey/plenipo/pull/118) was squash merged as
`61e87b6d063496394c27677d1402c6c72c1d3374`, with parent
`8b32203cd5825a3b488c6f5a88b83b82ea118c0f`. The reviewed head was
`8f91b2295b09333a259b2f2187a3367b6ac0c551`. Both have the same whole-repository
tree, `5b7e1520ea78c080372825fb645db2dcbec00865`. The website still offers installer
`1.17.0`; this address move does not change the desktop app.

The isolated Coastline copy matched all 2,086 tracked source files. Version, format,
lint, type checks, Rust format/clippy, and generated-binding comparison passed.
All 1,568 Rust, 33 website, 304 shared UI, and 508 desktop tests passed. The test
container needed GNU `flock` from `util-linux`; BusyBox's version lacks the timeout
option used by the updater tests.

An initial server-side status poll reached GitHub's unauthenticated API quota before
any rehearsal deployment. Polling moved to the signed-in local CLI. After the quota
reset, the real updater's normal API checks passed; no credentials or gate overrides
were introduced.

The exact reviewed head passed [full CI](https://github.com/Seckcey/plenipo/actions/runs/36767879133)
and [Website CI](https://github.com/Seckcey/plenipo/actions/runs/36767879164).
The updater also requires Website CI at the last website-changing commit. The genuine
[Website run at `e4ccc22`](https://github.com/Seckcey/plenipo/actions/runs/36768207230)
passed with the same website tree. This check was run using a temporary validation ref,
without changing the PR or bypassing its gate. The owned ref was removed afterward
with an exact-SHA guard.

The merged source also passed all six jobs in [full CI](https://github.com/Seckcey/plenipo/actions/runs/36773693632)
and [Website CI](https://github.com/Seckcey/plenipo/actions/runs/36773693537).
The coordinator reviewed the exact merged staging parity and issued the source-pinned
release handoff after those checks passed.

## Staging and rollback receipt

The separate Compose project `plenipo-domain-01a0f3b9` used loopback port `14382` and
subnet `10.204.231.0/28`. It used synthetic website state and the retained prior image.
Production remained at port `14380`.

At 20:29–20:30 UTC, actual updater rehearsals injected a site-verification failure
and an updater-installation failure. Both restored:

- Source `8b32203cd5825a3b488c6f5a88b83b82ea118c0f`, version `1.17.0`.
- Image `sha256:3e58eae68c8110f66a8cdacbf52161ce25a5a9d64308fe429e271ba390817151`.
- Updater SHA-256 `16e8ac4a7d0675a8c3fed2d6e2b1306b24ecff4de0860f64a3f3dec681de90c4`.
- The exact prior `current.env`, matching release metadata, a healthy container with
  zero restarts, and the loopback-only binding.
- The old canonical address on the homepage and both legal pages.

The persistent staging lock stayed held after each failure until independent recovery
checks passed and the owned rehearsal process was stopped. Separate child-failure tests
proved the hold survives before, during, and after export cleanup. The inherited
updater's health-only rollback result was not treated as complete acceptance.

The success rehearsal verified the new site, recorded it, installed the exact source
updater, and then released the lock. Its updater SHA-256 was
`2e2da01bd7a851cbf55d8b7f657a6f3178ede4f55a4a474653e36652717bd4af`.
All 64 served files matched the browser-accepted preview. The exact merged-source
image then matched 63 of those files; only `release.json` changed its source revision.
All other release fields stayed equal. Both origins passed 14 old/www redirect cases,
including encoded paths and query strings, new canonical pages, health, and real 404s.

Chrome acceptance passed at desktop 2560×1215 and phone 390×844: no horizontal overflow,
the phone's List view and navigation, four demo tabs, a sample approval, the policy tab,
download targets, and terms/privacy navigation. No browser warnings or errors were seen.

## External link receipt

GitHub's repository homepage was saved and reread as `https://getplenipo.com`.
The existing Microsoft app's homepage, terms, and privacy URLs were saved as
`https://getplenipo.com`, `https://getplenipo.com/terms/`, and
`https://getplenipo.com/privacy/`. They persisted after a page reload. The app name,
publisher domain, publisher-verification state, OAuth configuration, credentials,
and permission grants were not changed.

The two attached 8 West website repositories had no Plenipo or old-host link to change.
Historical website receipts retain the addresses and revisions they originally verified.
The separate account-service plan remains outside this website release.

## Cleanup receipt

The staging container and network were removed at 20:39 UTC under the shared allocation
lock. Port `14382` was verified free and its reservation was archived. The staging images,
source, built files, and receipts were retained. The temporary Chrome preview tab and
local SSH forward were closed. The browser viewport override was reset.

The task-created DNS and CI tabs were closed. The existing Microsoft app tab was
preserved, and the verified public website was left open as the deliverable. No
temporary test container or preview listener remains. Desktop Docker stayed disabled.

## Production receipt

The same persistent production lock, inode `6423019`, stayed held from 20:34:01 UTC
before the merge through verification and updater installation at 21:15:25 UTC.
The normal timer observed the held lock and exited without deploying during that
interval. The exact merged-source handoff was accepted at 21:15:14 UTC.

The deployed source is `61e87b6d063496394c27677d1402c6c72c1d3374`, installer `1.17.0`,
image `sha256:57e0dfc300a707f41cbca71924b43806dc19295540c64137a4c69aa8335878bc`,
container `c2b1ba0428b5`. The Coastline website service was healthy with zero restarts,
read-only filesystem, user `10001:10001`, 64 MiB memory, 0.5 CPU, and only the
`127.0.0.1:14380` published binding. The current release record matched the runtime.

The unmodified source updater, SHA-256
`2e2da01bd7a851cbf55d8b7f657a6f3178ede4f55a4a474653e36652717bd4af`, was installed
before the lock was released. The lock file was preserved, and the normal timer
remained enabled and active. A clean-environment invocation of the installed updater
with `--check` at 21:17:21 UTC reported that the website was up to date.

All 64 production files were identical to merged-source staging. All 64 files also
matched through public HTTPS; comparison normalized only Cloudflare's established
analytics script and email-protection markup in HTML. Other assets were compared
byte-for-byte. No cache purge or cache-busting workaround was needed.

The origin passed 14 old/www redirect cases and the public edge passed 12, including
encoded paths and query strings. Both use permanent 308 redirects to the apex. HTTP
upgrades, valid HTTPS, homepage/legal canonical metadata, robots, sitemap, health,
and real 404 responses passed. The account host and `/v1/check` did not resolve to
website application content. The seven existing DNS mail/verification records and
separate account-service configuration were preserved.

Chrome public acceptance passed on desktop and at 390×844: canonical URLs, both
v1.17.0 download targets, demo tabs and sample approval, policy content, terms/privacy
navigation, mobile List view and menu, and no horizontal overflow. Old-host privacy
and www terms navigation retained their paths and encoded queries at the apex.
No browser warnings or errors were reported.

The verified prior image and release source remain available. The exact old updater,
release record, and image identity are retained under
`/srv/8west/apps/plenipo-website/auto-release/domain-backup-8f91b229`.
The 36 release helpers, hash manifests, rehearsal/recovery proofs, handoff, runtime,
origin/public/browser checks, and cleanup receipts are retained with an evidence hash
manifest under
`/srv/8west/apps/plenipo-website/auto-release/evidence/61e87b6d063496394c27677d1402c6c72c1d3374-domain`.
Screenshots and external-settings evidence are also retained in the task's local
artifact folder. The account, mail, Stripe, publisher-verification, and OAuth settings
were not part of the release.

## Acceptance status

Production acceptance passed on September 30, 2026. `https://getplenipo.com` is the
live canonical website, and the old hostname and www redirect to it. This final
receipt is a documentation-only follow-up; production remains the verified code
release `61e87b6d063496394c27677d1402c6c72c1d3374`.
