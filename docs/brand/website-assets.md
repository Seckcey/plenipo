# Website brand assets

The approved branding masters are in `assets/`, imported byte for byte from the maintainers' local
asset folder. The original 47-file kit contains 15 transparent Pip poses, horizontal and square
logos for light/dark surfaces, and matching P-only favicons. Its original README and manifests
describe that earlier asset-only delivery; this page documents the later website integration.

The horizontal logo uses the large three-rail blue P as the first letter of Plenipo, Sentient
Medium outlines for `lenipo`, a matching blue final `o`, and Pip coding on top of the `n`.
It has no parent-company subtitle. The square variants feature Pip sitting on the P.
The hybrid SVG masters combine native vector lettering with embedded raster Pip artwork.
The simpler P-only favicons retain legibility at browser-tab sizes. The desktop app icons are
unchanged by this website update.

`assets/providers/` extends the kit with four new transparent PNGs: Pip holding Anthropic's
AI symbol, OpenAI's Blossom, the current SpaceXAI symbol selected for Plenipo, and Moonshot AI's
striped sphere. Saved prompts, reference provenance, alpha checks, and a light/dark proof sheet
live beside the artwork. These identify tools and do not assert an official partnership.

The website uses alpha WebP exports: the approved horizontal logo in the header/footer, Pip
waving in the hero, the four provider poses in the AI tools section, and Pip with a rocket in
the download panel. Both light/dark horizontal and square exports are supplied; the white page
uses the light-surface palette. The favicon follows the browser's light/dark preference.
`public/plenipo-brand-kit.zip` contains the approved kit plus the four provider illustrations.
Older files remain available for compatibility but are no longer used by the page.

`apps/website/scripts/export-brand.mjs` converts the approved masters to web-sized assets using
a caller-supplied Sharp installation. It preserves source files, alpha, and aspect ratios. It
also makes the intentionally opaque `plenipo-pip-social.png` share card (1280 × 640), and updates
the web manifest. No image-processing dependency runs on the website. This social card changes
website sharing metadata only; GitHub repository settings are owned by a separate work item.

The landing page uses a restrained white, black, pale gray, and blue presentation inspired by
the supplied ui.com reference. It does not ship Ubiquiti trademarks, product images, or source.

The Inter variable font is self-hosted from the [official Inter project](https://github.com/rsms/inter).
Its SIL Open Font License is retained at `apps/website/public/fonts/Inter-LICENSE.txt`.

## Product screenshots

The interactive homepage sample uses five additional approved poses: presenting (03), thinking
(04), celebrating (05), quality check (06), and support (14). Each 240 × 240 alpha WebP in
`apps/website/public/brand/pip/` is a fit/contain, quality-85 export of its corresponding
unchanged `assets/mascots/` PNG. Pip explains the current view and reacts to a sample approval;
visitors can hide the tips. These are selected existing illustrations, not newly generated art.

The eight private September 28 desktop references informed the light/dark cards, reporting
lines, dotted canvas, and selected-person details. They are not published. Names, objectives,
messages, decisions, and times in the interactive sample are authored fictional examples.
The website combines diagram and conversation browsing for demonstration and does not claim
to be a running desktop instance. Existing lower-page app screenshots retain their earlier
release captions.

These WebP files are format conversions of repository acceptance screenshots; their content was
not redrawn or generated. They show prior versions and sample/test work, as the website caption
states. They are product media, not interactive application controls.

| Website media       | Repository source                                           |
| ------------------- | ----------------------------------------------------------- |
| `organization.webp` | `docs/phases/evidence/phase-8/development-organization.png` |
| `models.webp`       | `docs/phases/evidence/phase-6/models-role-choices.png`      |
| `approvals.webp`    | `docs/phases/evidence/phase-7/approval-card.png`            |
| `activity.webp`     | `docs/phases/evidence/phase-7/guard-trail.png`              |

The supplied current screenshots informed content and layout. The AI tools screenshot
contained personal local paths and is not published. Replace marketing screenshots with fresh
sanitized app captures when the UI changes, keeping real text and truthful feature/version labels.
