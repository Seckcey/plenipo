// One-time asset export. Sharp is supplied by the caller; it is not a website dependency.
import { createRequire } from "node:module";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
if (!process.argv[2]) throw new Error("Usage: node export-brand.mjs /absolute/path/to/sharp");
const sharp = require(resolve(process.argv[2]));
const root = fileURLToPath(new URL("../../..", import.meta.url));
const assets = resolve(root, "assets");
const output = resolve(root, "apps/website/public/brand");
const providers = ["pip-16-anthropic", "pip-17-openai", "pip-18-xai", "pip-19-moonshot"];
await mkdir(resolve(output, "pip"), { recursive: true });

for (const surface of ["light", "dark"]) {
  for (const shape of ["horizontal", "square"]) {
    const name = `plenipo-${shape}-on-${surface}`;
    await sharp(resolve(assets, `logos/${name}.png`))
      .resize({ width: 600 })
      .webp({ quality: 92, alphaQuality: 100 })
      .toFile(resolve(output, `${name}.webp`));
  }
  await copyFile(
    resolve(assets, `favicons/plenipo-favicon-on-${surface}.svg`),
    resolve(output, `pip-favicon-on-${surface}.svg`),
  );
}
await copyFile(
  resolve(assets, "favicons/plenipo-favicon-on-light.ico"),
  resolve(output, "pip-favicon.ico"),
);
for (const size of [180, 192, 512]) {
  await sharp(resolve(assets, "favicons/plenipo-favicon-on-light.svg"))
    .resize(size, size)
    .png()
    .toFile(resolve(output, `pip-icon-${size}.png`));
}
for (const [folder, name] of [
  ["mascots", "pip-02-welcome"],
  ["mascots", "pip-11-launch"],
  ...providers.map((name) => ["providers", name]),
]) {
  await sharp(resolve(assets, folder, `${name}.png`))
    .resize(480, 480, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .webp({ quality: 90, alphaQuality: 100 })
    .toFile(resolve(output, "pip", `${name}.webp`));
}

// A separate opaque share card; production logos and mascot exports retain alpha.
const logo = await sharp(resolve(assets, "logos/plenipo-horizontal-on-light.png"))
  .resize({ width: 830 })
  .png()
  .toBuffer();
const caption = Buffer.from(`<svg width="1280" height="640" xmlns="http://www.w3.org/2000/svg">
  <text x="640" y="498" text-anchor="middle" font-family="Arial, sans-serif" font-size="36" fill="#0B1833">Your AI workforce. On your desktop.</text>
  <text x="640" y="554" text-anchor="middle" font-family="Arial, sans-serif" font-size="21" fill="#5D6575">plenipo.8westit.com</text>
</svg>`);
await sharp({ create: { width: 1280, height: 640, channels: 4, background: "#f5f8ff" } })
  .composite([{ input: logo, left: 225, top: 78 }, { input: caption }])
  .png()
  .toFile(resolve(output, "plenipo-pip-social.png"));

// Diagnostic contact sheet, deliberately presented on light and dark backgrounds.
const panels = [];
for (let row = 0; row < 2; row++) {
  for (let col = 0; col < providers.length; col++) {
    const mascot = await sharp(resolve(assets, "providers", `${providers[col]}.png`))
      .resize(280, 280, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png()
      .toBuffer();
    panels.push({ input: mascot, left: col * 320 + 20, top: row * 350 + 15 });
  }
}
const labels = ["Anthropic", "OpenAI", "xAI / SpaceXAI", "Moonshot AI"];
const text = Buffer.from(
  `<svg width="1280" height="700" xmlns="http://www.w3.org/2000/svg">${[0, 1].map((row) => labels.map((label, col) => `<text x="${col * 320 + 160}" y="${row * 350 + 326}" text-anchor="middle" font-family="Arial, sans-serif" font-size="19" fill="${row ? "#f3f7ff" : "#0b1833"}">${label}</text>`).join("")).join("")}</svg>`,
);
const dark = await sharp({
  create: { width: 1280, height: 350, channels: 4, background: "#0b1833" },
})
  .png()
  .toBuffer();
await sharp({ create: { width: 1280, height: 700, channels: 4, background: "#ffffff" } })
  .composite([{ input: dark, left: 0, top: 350 }, ...panels, { input: text }])
  .png()
  .toFile(resolve(assets, "providers/pip-provider-preview.png"));

const validation = [];
for (const name of providers) {
  const file = resolve(assets, "providers", `${name}.png`);
  const metadata = await sharp(file).metadata();
  const { data, info } = await sharp(file)
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });
  let transparent = 0;
  let minimum = 255;
  let maximum = 0;
  for (let i = 3; i < data.length; i += info.channels) {
    const alpha = data[i];
    minimum = Math.min(minimum, alpha);
    maximum = Math.max(maximum, alpha);
    if (alpha === 0) transparent++;
  }
  if (!metadata.hasAlpha || minimum !== 0 || maximum !== 255)
    throw new Error(`Missing genuine alpha: ${name}`);
  validation.push({
    file: `${name}.png`,
    width: metadata.width,
    height: metadata.height,
    hasAlpha: metadata.hasAlpha,
    alphaRange: [minimum, maximum],
    transparentPixels: transparent,
  });
}
await writeFile(
  resolve(assets, "providers/validation.json"),
  `${JSON.stringify(validation, null, 2)}\n`,
);
const manifest = JSON.parse(
  await readFile(resolve(root, "apps/website/public/site.webmanifest"), "utf8"),
);
manifest.icons = [192, 512].map((size) => ({
  src: `/brand/pip-icon-${size}.png`,
  sizes: `${size}x${size}`,
  type: "image/png",
}));
await writeFile(
  resolve(root, "apps/website/public/site.webmanifest"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);
console.log(
  `Exported the Plenipo logos, Pip web images, favicon set, share card and provider proof sheet. All ${validation.length} new source PNGs have genuine alpha.`,
);
