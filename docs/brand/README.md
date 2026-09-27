# Brand

Plenipo's look: dark base, one electric blue accent, thin outlined shapes, no gradients on text, no
drop shadows.

## Colors

| Use                     | Hex       |
| ----------------------- | --------- |
| Background, darkest     | `#080D16` |
| Background, panel       | `#101B2C` |
| Border, hairline        | `#23344D` |
| Accent (electric blue)  | `#2F7BF6` |
| Accent, light (on dark) | `#6AA6FF` |
| Text, brightest         | `#F1F6FD` |
| Text, body              | `#B9C9DD` |
| Text, quiet             | `#6E86A3` |
| Text, quietest          | `#4E637D` |

Type is Segoe UI on Windows, falling back to Inter and then the system sans-serif.

## Files

| File                                       | What it is                                      | Size     |
| ------------------------------------------ | ----------------------------------------------- | -------- |
| [`social-preview.svg`](social-preview.svg) | Repository social preview and the README banner | 1280×640 |

## Setting the GitHub social preview

The social preview is the image that shows up when the repository is shared on X, LinkedIn, Slack,
Discord, or iMessage. Without one, those all show a grey GitHub placeholder. GitHub only takes PNG,
JPG, or GIF here, so the SVG has to be exported first.

1. Open `docs/brand/social-preview.svg` in Illustrator.
2. **File → Export → Export As…**, pick **PNG**, name it `social-preview.png`.
3. In the export dialog set **Width 1280**, **Height 640** (Resolution: Screen, 72 ppi). Keep the
   background — do not tick Transparent.
4. Check the file is under 1 MB. If it is not, run it through **File → Export → Save for Web
   (Legacy)** as PNG-24 instead.
5. In the browser, go to
   [Settings → General](https://github.com/Seckcey/plenipo/settings) on the repository, scroll to
   **Social preview**, click **Edit → Upload an image…**, and pick the PNG.
6. Paste the repository URL into a Slack or Discord message to check the card renders.

Keep the exported PNG out of git — it is generated from the SVG, and the SVG is the source.

## Editing the artwork

The SVG is hand-written and readable; text is real text, not paths, so it can be edited in any
editor. If you re-draw it in Illustrator, keep the 1280×640 canvas, keep the type as live text, and
save back as plain SVG (**File → Save As → SVG**, Styling: _Presentation Attributes_, Fonts: _SVG_)
so it keeps rendering on GitHub.
