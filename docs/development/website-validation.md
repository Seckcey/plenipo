# Marketing website acceptance

## Pip branding update — September 27, 2026

The approved Plenipo logo now appears in the header/footer, with Pip in the welcome/download
areas and the four AI-provider illustrations. Existing sections, Inter page typography,
product screenshots, controls, pricing, and permission wording are retained. The narrow content
correction uses the published v1.6.0 installer and adds Kimi/Moonshot while keeping Ollama cloud
support. Release evidence is linked from [website operations](website.md).

Asset verification:

- All 47 original owner-supplied files match their actual Git archive bytes. Two receipt files
  retain original CRLF bytes through exact `.gitattributes` entries; no global renormalization.
- Four new source PNGs preserve image-generation output bytes and have genuine alpha 0–255.
  All six website Pip exports are 480 × 480 alpha WebP images. The four new provider images
  total 138,784 bytes. The Moonshot portrait is fitted with padding, preserving its antenna/feet.
- Actual saved source proof sheets were inspected on white and navy. Approved light/dark
  horizontal and square logos, P-only favicons, and the website share card were also inspected.
- The downloadable ZIP opens without errors and matches every original and new asset byte for
  byte. SHA-256: `570efa09cd907640a18ada2c8b13075b56250ba9ffaa79544012da9519d33f90`.
  Versioned manifest/ZIP links prevent reuse of their previous one-hour browser cache entries.

Browser flow: load the page, inspect the branding, switch a product view, open/close mobile
navigation, follow AI tools/download links, and expand the FAQ. Unified computer-use's in-app
browser supplied the Playwright controls; the separate Browser plugin is not installed.

| Check                 | Result                                                                         |
| --------------------- | ------------------------------------------------------------------------------ |
| Page identity/content | Correct title, heading, four labeled providers, no blank screen/error overlay  |
| Desktop/phone layout  | 1440, 900, 390, and 320 px; no horizontal page overflow                        |
| Mascots               | Complete artwork, transparent edges, all visible images loaded                 |
| Controls              | Product tab click/Home key, menu selection/Escape close, FAQ expansion passed  |
| Console               | No warning/error entries in tested page                                        |
| HTTP                  | 18 origin paths passed; served asset bytes matched; missing route returned 404 |
| Installer             | Published v1.6.0 asset resolved to final HTTP 200                              |

Actual saved captures are retained with the task's validation evidence: `preview-desktop.png`,
`preview-providers-desktop.png`, `preview-mobile-390.png`, and provider views at 390/320 px.
For durable PR review, exact copies of the inspected [before](../brand/website-evidence/pip-before-desktop.png),
[after](../brand/website-evidence/pip-after-desktop.png),
[provider section](../brand/website-evidence/pip-providers-desktop.png), and
[phone](../brand/website-evidence/pip-mobile.png) captures are included in the repository.
The approved artwork matches the references; website exports reduce resolution for page weight,
and decorative provider images use empty alt text because adjacent labels identify each tool.
Browser scope is Chromium at the tested sizes; this does not claim testing in every browser.

All five repository-required commands passed on the isolated Coastline source copy:
`pnpm check` (240 UI tests, 175 desktop tests, 2 website tests, plus versions/format/lint/types),
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked` (765 passed, none failed/ignored), and `pnpm bindings`
(189 export checks; the complete generated-file list and byte hashes stayed unchanged).
The Rust/application tree matches base `eb49e9f`; the imported branding source was checked at
`1088f62` with the final manifest/ZIP cache links. Later receipt/evidence-only additions receive
focused formatting/build verification. Final merge/release identity belongs in the deployment
receipt.

Checks used a 2 GiB/2 CPU container with one compilation job and serial Rust tests. Debian
Chromium was installed in that container for the repository's synthetic browser tests. The
preview used a separate Coastline task directory and a 64 MiB container. No desktop Docker,
other application data, app source, or GitHub repository settings were changed.

## Original website delivery

Validated on September 27, 2026, against desktop base `a3439987d803f04a2e323c1cceb151bf00e695b4`.
This change adds the website without modifying desktop source, its manifests, or generated bindings.
Container checks ran on Coastline in isolated task directories; no desktop Docker was used.

After PR #27 documented planned Pro pricing and PR #29 merged, the two pricing statements were
reconciled against main `189e6d61c5fbbf6cc217b5cb31666d6bf021dd3b`. Planned Pro is $9/month or
$99/year; v1.4.0 remains free and unrestricted. This follow-up changes website copy only.
The follow-up passed `pnpm check` in a disposable 1 GiB/1 CPU Coastline container, the two website
build/metadata tests, image health, and browser pricing/FAQ checks with no 320 px page overflow
or console errors. The completed original desktop CI also passed; desktop code, configuration,
lockfiles, and bindings are unchanged by the pricing reconciliation.

## Checks

- `pnpm check`: passed versions, formatting, lint, workspace type checks, 157 desktop tests,
  and 2 website build/metadata tests. Existing React test `act(...)` warnings were non-fatal.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 674 passed, none ignored.
- `pnpm bindings`: 168 export checks passed; generated files had no diff.
- Website production image: built successfully; non-root/read-only Compose container became healthy.
- Origin: home, CSS, JavaScript, brand/media assets, health, manifest, and sitemap available;
  an unknown path returned HTTP 404. Published Windows installer resolved to HTTP 200.
- Browser: desktop 1440 px, mobile 390 px and 320 px checked. No horizontal page overflow at 320 px
  after repairing the tab rail. Arrow/Home/End keys selected tabs; mobile navigation opened,
  closed on selection and Escape; FAQ expanded; in-page links reached their sections.
- Browser console: no warning/error entries in the tested page. Self-hosted Inter loaded.
- A stale stylesheet observed during iteration led to content-derived CSS/JS query hashes in
  the build. Tests verify those hashes and every local destination/asset. Final focused tests,
  lint and formatting passed after the visual fixes.
- Release freshness: the published Windows installer changed to v1.4.0 during review. Both
  download links, visible versions, structured metadata, and generated release metadata now agree;
  the v1.4.0 installer resolved to HTTP 200. Focused website tests, lint, formatting, container
  health, and browser version/download/FAQ checks passed after this update. Approval wording
  reflects the release's optional settings, which start off, for allowed websites.

The Rust validation container was capped at 2 GiB/2 CPUs with one compilation job; frontend checks
ran in a 1 GiB container. Both were removed after checks. Each temporary website preview
uses a 64 MiB limit and is retired after its acceptance checks. Host backup and unrelated apps
were preserved. Exact merged revision, image and public acceptance belong in the deployment
receipt; a preview test is not proof that the public hostname is live.

## Visual comparison

The generated concept and actual browser captures were viewed together during review. The
following five design anchors match the selected direction:

| Anchor                            | Browser result                                                                    |
| --------------------------------- | --------------------------------------------------------------------------------- |
| White canvas and restrained color | White background, dark text, blue actions, pale gray feature panel                |
| Compact navigation                | One consistent Plenipo/Product/AI tools/Free & Pro/Resources header               |
| Hero hierarchy                    | Centered two-line headline, paired primary/secondary actions, broad product media |
| Product exploration               | Pill-style preview tabs and an underlined workflow tab rail                       |
| Lower-page structure              | Open AI-tool row, split text/media, planned edition table, quiet footer           |

Intentional adaptations: actual app captures replace generated app mockups; the existing vector
P replaces the concept's approximate mark; the screenshot tab says Model choices to describe its
real view; provider names use typography; both editions are explicitly Planned. Native responsive
layouts, keyboard behavior, version labeling, and disclosure controls replace static concept art.
This is a close visual interpretation of the reference, not a claim of pixel identity with ui.com.

Desktop evidence:

![Desktop website](../brand/website-evidence/desktop.png)

Small-screen evidence:

![320px website](../brand/website-evidence/mobile.png)
