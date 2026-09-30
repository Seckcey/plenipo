# Terms and privacy statements — checklist and acceptance

Date: September 30, 2026. Scope: GitHub documentation and the marketing website.
Contact supplied by the owner: `admin@8westventures.com`.

## Checklist

- [x] Add complete terms of service and a privacy statement credited to 8 West Ventures, LLC.
- [x] Keep software rights under the existing Elastic License 2.0.
- [x] Describe local records, chosen AI services, optional connections, website logs,
      Cloudflare delivery/analytics, support, retention, and deletion choices.
- [x] Use one Markdown source for GitHub and the website's static pages.
- [x] Link the statements from README, support, FAQ, Download, footer, and sitemap.
- [x] Include the policy sources in the standalone website build context.
- [x] Check desktop and phone navigation, content, layout, console health, and reading
      without JavaScript.
- [x] Validate the workspace and standalone website builds and website tests.
- [x] Complete repository checks and confirm generated bindings before pushing.
- [ ] Review policy wording for publication, merge, and deploy the website.
- [ ] Verify both pages and their navigation on the public site after deployment.

## Acceptance evidence

The website build creates `/terms/index.html` and `/privacy/index.html` from the files
in `apps/website/legal`. Both documents carry the same date and contact address used
in GitHub documentation. Legal pages use the current content-hashed site stylesheet,
have canonical URLs, and need no scripts. The server configuration revalidates their HTML.

The existing website tests cover complete pages, contact and navigation links, local
assets, canonical metadata, stylesheet hashes, sitemap entries, and builds with a
release-version override. A renderer test covers retained h1/h2 headings, email links,
escaped HTML, refused unsafe links, and unchanged release-note link rules.

Browser checks used installed Chromium with Playwright because the Browser plugin was
not available. Viewports were 1440 × 1000 and 390 × 844. The flow was homepage footer →
Terms → Privacy → Home → Download privacy link. Both flows passed with the expected
URLs, titles, headings, contact links, no horizontal overflow, and no console errors.
Both documents remained readable with JavaScript disabled. First-viewport screenshots
were inspected. Temporary evidence is in `/tmp/plenipo-legal-qa`, outside the repository.

The standalone check copied only `apps/website` to a temporary directory, installed
its committed npm lock with `npm ci`, and built with `PLENIPO_VERSION=1.15.0`. Both
policy pages were present. This verifies the standalone source/package context, not
a production container or a published installer version.

The repository checks passed: versions, formatting, lint, TypeScript, 747 UI tests,
15 website tests, Rust formatting and strict Clippy, and 1,386 Rust tests. Binding
export also passed; all 304 generated files matched the tracked bindings. No dependency
or lockfile changes were needed. Windows installer and full tauri-driver E2E checks
were not run in this Linux task.

## Publication status

The cloud environment can read GitHub and its native Git route permits publishing a
task branch. The GitHub API is denied by the current network policy. Required
`api.github.com` and `plenipo.8westit.com` destinations were saved in the environment
draft; saving a draft does not apply it to the running machine.

The documented `coastline` SSH name has no configured host mapping and cannot resolve
here. No live website deploy or public-page acceptance has been performed. Follow the
[website deploy procedure](../development/website.md#reserve-deploy-and-roll-back)
after review and merge, using a configured Coastline connection. A website-only change
may require the existing updater's `--force` option. Do not treat a local build or a
GitHub branch as a production deployment.
