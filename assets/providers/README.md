# Pip with AI tools

Four new transparent source PNGs extend the original 15-pose Pip kit:

| File                   | Held mark                                                 |
| ---------------------- | --------------------------------------------------------- |
| `pip-16-anthropic.png` | Anthropic AI symbol                                       |
| `pip-17-openai.png`    | OpenAI Blossom                                            |
| `pip-18-xai.png`       | Current SpaceXAI symbol, explicitly selected by the owner |
| `pip-19-moonshot.png`  | Moonshot AI striped sphere                                |

The original generated PNG bytes and alpha are preserved. The first three images are 1254 × 1254; Moonshot is 1218 × 1292. They work on light and dark surfaces. `pip-provider-preview.png` shows both backgrounds for review; that preview deliberately has an opaque background. `validation.json` records the source dimensions and actual alpha checks.

Created with the built-in `image_gen.imagegen` tool, with transparent backgrounds. `prompts.json` contains the exact prompts and reference roles. Pip's approved welcome pose supplied character continuity. `references/sources.json` records the company artwork sources. These company marks identify supported tools; they do not replace Plenipo's own logo or imply an official partnership.

The original kit's README and manifests describe its earlier asset-only delivery and remain unchanged. Website integration is documented in `docs/brand/website-assets.md` and `docs/development/website-validation.md` at the repository root.

Web exports are 480 × 480 alpha WebP images, fitted with transparent padding so the original antennae, hands, feet, and signs remain complete. Regenerate them and the light/dark proof sheet with:

```sh
node apps/website/scripts/export-brand.mjs /absolute/path/to/sharp
```

Sharp is an export-time tool, not a production website dependency. The PNG originals remain the branding masters.
