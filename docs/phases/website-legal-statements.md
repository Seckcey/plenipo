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
- [x] Obtain the owner's publication approval and merge the statements.
- [x] Verify both pages, their complete policy content, and navigation on the public site.

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

The owner approved publication. [Pull request #114](https://github.com/Seckcey/plenipo/pull/114)
merged as `6776e0c767feb3c980155b28102e3a69436eef5a`. Its full GitHub checks passed,
including Windows and app E2E. Initial API access was blocked; it became available
before the merge.

On September 30, 2026, public `/terms/` and `/privacy/` requests returned HTTP 200.
Both contained the complete content rendered from their canonical Markdown, the correct
canonical URL, and links to Home and both policies. Cloudflare's email protection
encodes the public email links; decoding them confirmed `admin@8westventures.com`,
and the full policy HTML matched after accounting for that rewrite. The
homepage and sitemap linked to both pages. Public `release.json` reported the merge
revision above and published installer version `1.16.0`.

This establishes public publication; the cloud session did not itself run the server
deploy command. The documented `coastline` SSH name still has no mapping here, and no
laptop or Tailscale connection is attached. The separate
[automatic website-change update](website-automatic-updates.md) still needs its
one-time server installation. Repository publication alone cannot replace that
installed script.
