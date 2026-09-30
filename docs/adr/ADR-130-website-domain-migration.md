# ADR-130: The public website moves to getplenipo.com

- **Status:** Accepted by the owner, 2026-09-30
- **Date:** 2026-09-30
- **Scope:** Website address only; amends item 4 of [ADR-105](ADR-105-domain-and-web-address.md)

## Request

The owner requested: "We need to change plenipo.8westit.com to the new domain getplenipo.com."
This is the separate website move anticipated by ADR-105. Plenipo remains made by
8 West Ventures, LLC. Product plans and account-service work continue separately.

## Decision

The canonical website address is `https://getplenipo.com`. The `www.getplenipo.com` and
`plenipo.8westit.com` addresses permanently redirect to that address with the original path
and query string. This includes policy pages, assets, and release metadata. Add and verify
the new HTTPS routes before enabling the old-address redirect.

Homepage search metadata, social links, policy links, robots, sitemap, and the brand export
use the new address. GitHub's homepage and Plenipo's Microsoft homepage/privacy/terms URLs
follow after the new public pages pass verification. OAuth redirects, publisher domain,
publisher verification, permissions, and credentials are outside this change.

The existing Coastline website service and loopback port stay in use. Preserve the automatic
updater's checks, source identity, installer selection, and rollback. Verification must use
the canonical address from the exact source being checked, so the prior website can still
be verified during rollback.

`account.getplenipo.com`, including the fixed `/v1/check` address, remains separate. This
move changes no account routing, email records, Stripe settings, license identity, or data.

## Release and recovery

Keep the old host serving until the new apex and www HTTPS routes work. Validate the exact
candidate on Coastline before the website release. Retain the previous image, source,
deployment settings, and installed updater. Verify redirects, homepage, demo, download,
policies, search files, release metadata, assets, and missing-page responses after release.

If the website release fails, restore the prior image/settings/updater using the release
lock. The old host then serves the prior website again; preserve the new routes while
investigating. Do not change mail or the separate account service to recover the website.

Acceptance and live identities belong in the dated website release receipt. This decision
records authorization and the intended behavior; it is not evidence that release completed.
