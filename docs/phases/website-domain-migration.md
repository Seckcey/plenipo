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
- [ ] Complete repository checks, exact-source staging, and rollback rehearsals.
- [ ] Obtain coordinator review and merge with the persistent updater lock held.
- [ ] Verify merged-source CI and staging parity; receive the release handoff.
- [ ] Deploy and install the exact updater under the same continuous lock.
- [ ] Verify public redirects, content, metadata, assets, browser flows, and account isolation.
- [ ] Update GitHub homepage and Microsoft homepage/terms/privacy URLs.
- [ ] Retain rollback and evidence, clean up owned previews, and complete documentation.

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

## Acceptance status

Implementation and release checks are pending. Complete this record from the actual
release receipt; a merged pull request alone is not a deployed website. Historical
website receipts keep the addresses and revisions that they verified at the time.
