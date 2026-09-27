# Plenipo + Pip brand kit

Pip is Plenipo's friendly blue-and-silver robot. This kit contains 15 individual transparent illustrations, finished horizontal and text-free square logos for light and dark backgrounds, and matching favicons.

## Ready-to-use files

| Folder | Contents |
| --- | --- |
| `mascots/` | 15 RGBA PNG illustrations, each 1254 x 1254 px. The same full-color Pip artwork works on light and dark surfaces. |
| `logos/` | Four logo designs, each supplied as PNG and self-contained SVG. Horizontal PNGs are 3000 x 1194 px; square PNGs are 2000 x 2000 px. |
| `favicons/` | Light/dark P-symbol SVGs; transparent PNGs at 16, 32, 48 and 256 px; ICO files containing the exact 16/32/48 px PNG exports. |
| `previews/` | Presentation sheets showing all poses on light and dark surfaces, all four logos, and actual-size favicon checks. These preview sheets intentionally have backgrounds. |
| `source/` | Approved reference artwork, the outlined wordmark master, complete generation prompts, native logo build script, and package validation script. Reference artwork is retained as received. |

Use `on-light` logos on white or pale backgrounds. Use `on-dark` logos on navy, charcoal or black backgrounds. The background is not part of either version: every deliverable in `mascots/`, `logos/` and `favicons/` is transparent. Opaque preview sheets and the original reference are not production cutouts.

The horizontal logo uses the prominent blue P symbol as the initial capital letter, followed by `lenipo`; the last `o` matches the primary blue. Pip sits on the `n` with his laptop. The square logo uses Pip seated on the large P without a wordmark. There is no parent-company attribution in either logo.

## Pip pose catalogue

| No. | File | Activity / typical use |
| --- | --- | --- |
| 01 | `pip-01-coding.png` | Seated at a laptop; product, development, logo accent |
| 02 | `pip-02-welcome.png` | Waving; welcome and onboarding |
| 03 | `pip-03-presenting.png` | Pointing; feature callouts and guidance |
| 04 | `pip-04-thinking.png` | Considering a problem; planning and ideas |
| 05 | `pip-05-celebrating.png` | Arms raised; milestones and success |
| 06 | `pip-06-quality-check.png` | Inspecting a circuit board; QA and review |
| 07 | `pip-07-security.png` | Holding a shield; protection and trust |
| 08 | `pip-08-planning.png` | Checking a clipboard; tasks and workflows |
| 09 | `pip-09-analytics.png` | Holding a chart; insights and reporting |
| 10 | `pip-10-teamwork.png` | Joining puzzle pieces; integrations and collaboration |
| 11 | `pip-11-launch.png` | Holding a small rocket; launches and getting started |
| 12 | `pip-12-maintenance.png` | Wrench and gear; setup and maintenance |
| 13 | `pip-13-learning.png` | Reading a book; documentation and learning |
| 14 | `pip-14-support.png` | Headset and helpful gesture; support |
| 15 | `pip-15-recharging.png` | Resting with a battery; pauses and recharge |

## Logo construction and color

| Element | On light | On dark |
| --- | --- | --- |
| Wordmark ink | `#0B1833` | `#F3F7FF` |
| Primary P rail and final o | `#2463EB` | `#5B97FF` |
| Middle P rail | `#5898F3` | `#8CBCFF` |
| Inner P rail | `#123C88` | `#D8E8FF` |

The approved name lettering is Sentient Medium 500, supplied as vector outlines. No font installation is required. Typeface source: [Fontshare Sentient](https://www.fontshare.com/fonts/sentient); [Fontshare license information](https://www.fontshare.com/licenses). No font binary is distributed in this kit.

The four mascot-bearing SVGs are **hybrid SVGs**: vector P/lettering plus an embedded transparent raster Pip. They have no linked image, external font, script, or network dependency. Pip is not vectorized. The P-only favicon SVGs are entirely vector. Use those simpler marks at browser-icon sizes, where a full mascot cannot retain its detail.

Keep the supplied aspect ratios and transparent padding. Use the original PNGs for Pip illustrations; avoid enlarging the 1254 px artwork beyond its native size when crisp raster detail matters. Review contrast when placing a logo over photography or a new background color.

## Generation and reproducibility

All 15 Pip illustrations were created with the **built-in `image_gen.imagegen` tool**, with `transparent_background: true`. The approved Pip Robot reference guided pose 01; pose 01 was the character reference for poses 02-15. Exact prompts, reference roles and original generated basenames are in `source/prompts.json`. The generated mascot PNGs were copied without altering their pixels or alpha channels.

`source/build-kit.cjs` composes the established native vector wordmark with Pip, renders the PNG exports with Sharp, and creates the diagnostic preview boards. From this kit folder, run:

```sh
node source/build-kit.cjs /path/to/sharp
python source/validate-kit.py
```

The second command requires Pillow. It builds the ICO containers from the exact existing PNG sizes, validates transparency/structure/dimensions, and writes `validation.json` and the SHA-256 `manifest.json`. Optional `--archive /absolute/path/Plenipo-Pip-Brand-Kit.zip` creates and verifies a ZIP after validation. Regenerating mascot poses requires the built-in image tool and the saved prompts; the build script does not call an image API.

## Verification and scope

The actual saved pose contact sheets were visually inspected on both white and navy backgrounds: all 15 characters, their antennae, hands, feet and props are complete and distinct. The actual saved logo-family and favicon-size sheets were also opened and inspected. Pip's seated placements, the complete name, the oversized P, matching blue final o and both palettes were checked. The native 16/32/48 px favicons were inspected alongside enlarged diagnostic views.

Every production PNG has genuine alpha transparency. Pip and full-size logo PNGs span alpha 0 to 255; the smallest favicon exports retain antialiased strokes. Generated cutouts retain the tool's original antialiasing; pose 01 includes a negligible 1/255-alpha corner pixel, recorded rather than silently changing the source. There is no baked white or checkerboard backdrop. `validation.json` records visible bounds using an alpha threshold of 8/255 as well as raw alpha and corner values.

This is an asset-only delivery. The website, application code, earlier concepts, and archived attribution version are preserved. Nothing has been integrated, published or deployed. No preview server, browser tab or container was started for this kit.
