# Account branding and website links

Made by 8 West Ventures, LLC. Scope: the marketing website and the separate private account
service. This does not turn on paid purchases or change desktop licensing.

The website adds sign-in, account creation, plan and account-management links to
`https://account.getplenipo.com`. The account service reuses the Pip wordmark, icons, Inter font,
blue buttons and white layout already shipped on the marketing site. It preserves its own
account forms, CSRF checks, signed-in navigation and attorney-approved account policies.

The old website still described Free and Pro as planned and advertised superseded pricing.
The comparison now follows [the shipped editions](../editions.md): one Free organization,
up to three Pro organizations, three concurrent Free workers and four per Pro organization.
The private account service's catalog remains the single source for Pro and Partner prices.
Both sites say paid purchases are not open yet. Free needs no account or card.

## Acceptance and release order

- [x] Reuse owned assets and correct outdated edition statements.
- [x] Keep both download links and both generated version labels.
- [x] Test account destinations and reject stale planned pricing in the website checks.
- [x] `pnpm check` passed: 304 shared UI, 521 desktop, 15 website and 13 repository-script tests.
      The 18 Linux-only website updater tests also passed on Coastline. Rust formatting, strict
      Clippy and 1,666 Rust tests passed; binding generation left no generated-file differences.
- [x] Frozen website source `8e7b6a4` built on Coastline; desktop and 390px browser checks
      verified the account destinations, edition comparison, version labels and no horizontal
      overflow or console warnings/errors. All GitHub checks passed at that application head.
- [x] Account service PR #13 merged and deployed first at source
      `2bae4aeeb558bc59b4c4a68f890acffef8acd703`. Public release identity, health, branded assets
      and landing/sign-in/reset/signup navigation passed. Stripe remains in test mode. Its
      checked updater is enabled and the first run passed; private release evidence stays in
      the account repository.
- [ ] Merge this website change after account acceptance, then verify the existing Coastline
      updater publishes the expected source and public links.

The account updater's design, tests, credentials and recovery procedure are documented in the
private `plenipo-account` repository. The website's existing checked updater remains in place.
No account secrets or customer records belong in this public repository. A merge alone is not
proof that either public service has updated. Final marketing release evidence will record both
revisions. The completed preview containers/network, temporary ports, SSH forwards and test
browser tabs have been cleaned up; evidence and rollback images remain retained.
