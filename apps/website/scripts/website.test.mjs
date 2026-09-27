import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { access, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { buildWebsite, websiteRoot } from "./build.mjs";

test("production build includes every local asset and valid internal destination", async () => {
  const output = await mkdtemp(join(tmpdir(), "plenipo-website-"));
  try {
    await buildWebsite(output);
    const html = await readFile(join(output, "index.html"), "utf8");
    const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
    assert.equal(new Set(ids).size, ids.length, "HTML IDs must be unique");
    for (const [, url] of html.matchAll(/(?:href|src)="([^"]+)"/g)) {
      if (url.startsWith("#") && url.length > 1) {
        assert.ok(ids.includes(url.slice(1)), `Missing internal destination: ${url}`);
      }
      if (url.startsWith("/") && !url.startsWith("//")) {
        await access(join(output, url.slice(1).split(/[?#]/)[0]));
      }
    }
    for (const file of ["robots.txt", "sitemap.xml", "site.webmanifest", "main.js", "styles.css"]) {
      await access(join(output, file));
    }
    const manifest = JSON.parse(await readFile(join(output, "site.webmanifest"), "utf8"));
    for (const icon of manifest.icons) {
      await access(join(output, icon.src.slice(1)));
    }
    // A new stylesheet or script gets a new URL; returning visitors cannot keep
    // a previous release's cached controls or layout after the HTML updates.
    for (const file of ["styles.css", "main.js"]) {
      const content = await readFile(join(output, file));
      const expectedHash = createHash("sha256").update(content).digest("hex").slice(0, 12);
      assert.ok(html.includes(`"/${file}?v=${expectedHash}"`), `Missing content hash for ${file}`);
      assert.ok(!html.includes(`"/${file}"`), `Unversioned reference remains for ${file}`);
      const source = await readFile(resolve(websiteRoot, file));
      assert.deepEqual(content, source, `Build changed the contents of ${file}`);
    }
    const release = JSON.parse(await readFile(join(output, "release.json"), "utf8"));
    assert.match(release.version, /^\d+\.\d+\.\d+$/);
    const visibleVersions = [...html.matchAll(/data-version>v([^<]+)</g)];
    assert.equal(visibleVersions.length, 2);
    assert.ok(visibleVersions.every((match) => match[1] === release.version));
    assert.equal(release.architecture, "x64");
    const downloads = [...html.matchAll(/data-download\s+href="([^"]+)"/g)];
    assert.equal(downloads.length, 2);
    assert.ok(downloads.every((match) => match[1] === release.downloadUrl));
    assert.ok(
      release.downloadUrl.endsWith(`/v${release.version}/Plenipo_${release.version}_x64-setup.exe`),
    );
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});

test("search metadata describes the actual free Windows release without fabricated ratings", async () => {
  const html = await readFile(resolve(websiteRoot, "index.html"), "utf8");
  assert.equal([...html.matchAll(/<h1\b/g)].length, 1);
  assert.match(html, /<link rel="canonical" href="https:\/\/plenipo\.8westit\.com\/"/);
  const data = JSON.parse(
    html.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/)[1],
  );
  assert.equal(data["@type"], "SoftwareApplication");
  assert.equal(data.operatingSystem, "Windows 11");
  assert.equal(data.offers.price, "0");
  assert.ok(!("aggregateRating" in data));
  assert.ok(!("review" in data));
  assert.match(data.license, /\/LICENSE$/);
  assert.match(html, /Free\s+<span>Planned<\/span>/);
  assert.match(html, /Pro\s+<span>Planned<\/span>/);
});
