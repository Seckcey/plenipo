# Decision: Shared terms and privacy statements for GitHub and the website

- **Status:** Accepted for implementation; policy wording awaits publication review
- **Date:** 2026-09-30
- **Phase:** Website

## Context

The owner requested terms of service and a privacy statement on Plenipo's GitHub
repository and website, and supplied `admin@8westventures.com` as the contact address.
The statements must describe the local app, connected providers, public website,
and support without changing the Elastic License 2.0 or claiming that AI requests
stay on the computer.

The website's production build sees only `apps/website`. Separate manually maintained
copies would let the GitHub and website wording drift.

## Decision

Keep the full statements as Markdown in `apps/website/legal`. Link them from the
repository README, support guide, and FAQ. Render those same documents into static
`/terms/` and `/privacy/` pages during every website build, including standalone
Docker builds.

Reuse the existing escaped Markdown renderer. Complete documents keep their heading
levels and allow email links; release notes retain their existing heading and link
rules. Legal pages need no scripts. Add both pages to the homepage footer, download
section, and sitemap. Serve their HTML with revalidation so visitors can see updates.

Use the owner's contact address. Describe website access logs and Cloudflare delivery
and analytics separately from local app records and chosen providers. Do not invent
a governing jurisdiction, arbitration clause, fixed retention promise, or a claim
of regulatory certification.

## Consequences

The public GitHub documents and website pages use one source. Publishing website
code requires a reviewed merge and a normal website deploy; the existing automatic
updater does not necessarily pick up a website-only change immediately. The owner
should review the policy wording before publication and keep it current as practices
change.

The statements do not add an in-app acceptance screen, change software license rights,
configure provider accounts, or change Cloudflare settings. Those require separate work
if requested.
