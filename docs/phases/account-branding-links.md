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
- [ ] Finish the final website preview and GitHub checks.
- [ ] Merge and verify the account service first, with its separate checked-update receipt.
- [ ] Merge this website change after account acceptance, then verify the existing Coastline
      updater publishes the expected source and public links.

The account updater's design, tests, credentials and recovery procedure are documented in the
private `plenipo-account` repository. The website's existing checked updater remains in place.
No account secrets or customer records belong in this public repository. A merge alone is not
proof that either public service has updated. Final release evidence will record both revisions.
