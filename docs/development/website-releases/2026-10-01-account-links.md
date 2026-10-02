# Account links and current editions

Made by 8 West Ventures, LLC. Released October 1, 2026 Pacific (October 2, 03:14:49 UTC).
The public website now provides Sign in, Create an account, Manage account and plan links to
`https://account.getplenipo.com`. Obsolete planned-edition limits and prices were replaced with
the shipped edition limits; the account catalog is the single source of prices. Paid purchases
remain closed while the account service uses Stripe test mode.

## Exact release

- PR #125 reviewed head `59b811952689120729cc94691c98681da7566d26`; merged source
  `3a00a9cc984e4e0429ecdb32683345baf161f830`. The merged tree matched the reviewed candidate.
- Main CI `36956567089` and Website `36956567034` both completed successfully at that source.
- Coastline image `plenipo-website:3a00a9cc984e4e0429ecdb32683345baf161f830-v1.18.1`, image ID
  `sha256:d1da0a08fa1047c0965a10d552ae5fc442da316ac53a914a538d2f4233d16268`.
- Container `302ad0a0cf0c`: healthy, zero restarts, bound only to `127.0.0.1:14380`.
- Public and origin `/release.json` agree on source `3a00a9cc...` and installer version `1.18.1`.
- Account service public source `f364fa1b96f7967f3f241d2e3306ba0aff77d5cd` was verified before
  this website release. Its branding/cache correction passed fresh and affected-browser checks.

## Release gates and public acceptance

The deployed website-source gap from `0e5975ca` contained only the reviewed homepage and website
test changes. Dockerfile, dependencies, deployment script, legal pages and interactive sample
were unchanged. The frozen website candidate had already passed desktop/mobile staging checks.

The installed updater matched the reviewed SHA-256
`2e2da01bd7a851cbf55d8b7f657a6f3178ede4f55a4a474653e36652717bd4af`. It ran as the existing
deploy owner with its normal defaults after exact-main checks passed. The persistent release
and allocation locks serialized the build and replacement. No force/source override, updater
upgrade, infrastructure setting or unrelated service change was used.

Postrelease verification matched all 64 origin files and all 59 public non-HTML files to the
actual container. Public HTML was checked for the intended content; byte equality was not
claimed because the existing public edge can insert analytics. Public policy pages, source,
two download links and two generated version labels passed. The Python HTTP client initially
received a public 403; normal curl requests returned 200 and completed the hash checks, and
Chrome rendered the live pages. No edge or security settings were changed.

Desktop browser clicks verified the visible header **Sign in** reaches `/signin`, **Create an
account** reaches `/signup`, and **Manage account** reaches `/signin?next=%2Faccount` for a
signed-out visitor. At 390px, **Open navigation** exposes the Sign in link; the same three
destinations passed. The page width and scroll width were both 390px, with no horizontal
overflow and no console warnings/errors. The viewport override was cleared and worker test
tabs were closed. No real signup, email, purchase or account change was submitted.

## Rollback and retained evidence

The previous website image `plenipo-website:0e5975ca22fb07ca38544da69021a422e876a38c-v1.18.1`,
ID `sha256:c802c09fb396f6e622368f64b11d4a647e166fa9952e762673bb7068fe2fbdb1`, and its immutable
release folder remain available. The updater recorded it as `PREVIOUS_IMAGE`; no rollback was
needed. The update timer remains active. Release/allocation lock inodes remained unchanged.

Host records are under `/srv/8west/apps/plenipo-website/auto-release`: `current.env`,
`release.json` and `history.jsonl`. Worker evidence includes `marketing-production-3a00a9cc.json`,
`marketing-live-browser.json`, desktop and mobile-menu screenshots, and the earlier staged
branding evidence. Temporary preview containers, networks, port reservations, SSH forwards and
worker browser tabs are cleaned up; source/build evidence and rollback images remain retained.

The account release and operational evidence remain in its private repository. Its existing
15-minute checked updater also completed a natural documentation-only check at 02:48:11 UTC,
retaining the already verified account runtime without replacement.
