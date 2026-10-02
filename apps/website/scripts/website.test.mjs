import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { access, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { gzipSync } from "node:zlib";
import { buildWebsite, releaseVersion, rootPackage, websiteRoot } from "./build.mjs";

test("production build includes every local asset and valid internal destination", async () => {
  const output = await mkdtemp(join(tmpdir(), "plenipo-website-"));
  try {
    // The repository's own version (whatever the shell may have exported).
    await buildWebsite(output, { env: {} });
    const html = await readFile(join(output, "index.html"), "utf8");
    // All crawler entry points agree on the new public address.
    for (const file of [
      "index.html",
      "terms/index.html",
      "privacy/index.html",
      "robots.txt",
      "sitemap.xml",
    ]) {
      const text = await readFile(join(output, file), "utf8");
      assert.ok(!text.includes("plenipo.8westit.com"), `${file} still names the old website`);
      assert.ok(text.includes("https://getplenipo.com/"), `${file} misses the canonical website`);
    }
    // The page loader requests this bounded local island immediately on entry.
    // Keep its URLs in data attributes so failed loads can be retried explicitly.
    for (const type of ["module", "style"]) {
      const url = html.match(new RegExp(`data-demo-${type}="([^"]+)"`))?.[1];
      assert.ok(url?.startsWith("/demo/demo-"));
      const bytes = await readFile(join(output, url.slice(1)));
      assert.ok(gzipSync(bytes).length < (type === "module" ? 180_000 : 12_000));
      assert.ok(!html.includes(`src="${url}"`) && !html.includes(`href="${url}"`));
    }
    assert.match(html, /No real AI runs/);
    assert.match(html, /Read the sample without the interactive demo/);
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
    // Policies must be complete static pages in every build, including a
    // version override used by the standalone image; no JS or demo is needed.
    for (const [slug, title] of [
      ["terms", "Terms of service"],
      ["privacy", "Privacy statement"],
    ]) {
      const page = await readFile(join(output, slug, "index.html"), "utf8");
      assert.ok(html.includes(`href="/${slug}/"`), `Homepage is missing ${slug}`);
      assert.ok(page.includes(`<h1>${title}</h1>`));
      assert.equal([...page.matchAll(/<h1\b/g)].length, 1);
      assert.ok(page.includes(`href="https://getplenipo.com/${slug}/"`));
      assert.ok(page.includes('href="mailto:admin@8westventures.com"'));
      assert.ok(page.includes('href="/terms/"') && page.includes('href="/privacy/"'));
      assert.ok(!page.includes("<script"), "Reading a policy must not require scripts");
      for (const [, url] of page.matchAll(/(?:href|src)="([^"]+)"/g)) {
        if (url.startsWith("/") && !url.startsWith("//")) {
          await access(join(output, url.slice(1).split(/[?#]/)[0]));
        }
      }
      const sitemap = await readFile(join(output, "sitemap.xml"), "utf8");
      assert.ok(sitemap.includes(`https://getplenipo.com/${slug}/`));
      assert.equal(
        page.match(/href="(\/styles\.css\?v=[^"]+)"/)?.[1],
        html.match(/href="(\/styles\.css\?v=[^"]+)"/)?.[1],
        "Policies share the current homepage stylesheet",
      );
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
    // The version is the product's (the root package.json), filled in at build time.
    const product = JSON.parse(await readFile(rootPackage, "utf8")).version;
    assert.equal(release.version, product);
    assert.ok(!html.includes("__PLENIPO_VERSION__"), "every version mark is filled in");
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
  assert.match(html, /<link rel="canonical" href="https:\/\/getplenipo\.com\/"/);
  const data = JSON.parse(
    html.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/)[1],
  );
  assert.equal(data["@type"], "SoftwareApplication");
  assert.equal(data.url, "https://getplenipo.com/");
  assert.equal(data.image, "https://getplenipo.com/brand/plenipo-pip-social.png");
  assert.equal(data.operatingSystem, "Windows 11");
  assert.equal(data.offers.price, "0");
  assert.ok(!("aggregateRating" in data));
  assert.ok(!("review" in data));
  assert.match(data.license, /\/LICENSE$/);
  assert.ok(!/Free\s+<span>Planned<\/span>/.test(html));
  assert.ok(!/Pro\s+<span>Planned<\/span>/.test(html));
  for (const path of ["/", "/signin", "/signup", "/account"]) {
    assert.ok(html.includes(`href="https://account.getplenipo.com${path}"`));
  }
  assert.match(html, /Paid purchases are not open yet/);
  assert.ok(!html.includes("$9") && !html.includes("$99"));
});

test("the page carries no version typed by hand", async () => {
  const source = await readFile(resolve(websiteRoot, "index.html"), "utf8");
  assert.ok(!/\/v\d+\.\d+\.\d+\//.test(source), "a download link carries a typed version");
  assert.ok(!/data-version>v\d/.test(source), "a version label is typed");
  assert.ok(!/"softwareVersion":\s*"\d/.test(source), "the structured data's version is typed");
});

test("PLENIPO_VERSION wins over the root package.json and must be a release version", async () => {
  const product = JSON.parse(await readFile(rootPackage, "utf8")).version;
  assert.equal(await releaseVersion({ env: {} }), product);
  assert.equal(await releaseVersion({ env: { PLENIPO_VERSION: "  " } }), product);
  assert.equal(await releaseVersion({ env: { PLENIPO_VERSION: " 9.8.7 " } }), "9.8.7");
  assert.equal(await releaseVersion({ env: { PLENIPO_VERSION: "9.8.7-beta.1" } }), "9.8.7-beta.1");
  for (const bad of ["latest", "v9.8.7", "9.8", '9.8.7"><script>', "9.8.7-"]) {
    await assert.rejects(
      releaseVersion({ env: { PLENIPO_VERSION: bad } }),
      /not a release version/,
    );
  }
  await assert.rejects(
    releaseVersion({ env: {}, packageFile: join(tmpdir(), "no-such-plenipo-package.json") }),
    /pass PLENIPO_VERSION/,
  );
});

test("a container build fills the page with the version it is given", async () => {
  const output = await mkdtemp(join(tmpdir(), "plenipo-website-"));
  try {
    await buildWebsite(output, { env: { PLENIPO_VERSION: "9.8.7" } });
    const html = await readFile(join(output, "index.html"), "utf8");
    assert.ok(html.includes("/releases/download/v9.8.7/Plenipo_9.8.7_x64-setup.exe"));
    assert.equal([...html.matchAll(/data-version>v9\.8\.7</g)].length, 2);
    const release = JSON.parse(await readFile(join(output, "release.json"), "utf8"));
    assert.equal(release.version, "9.8.7");
    await access(join(output, "terms", "index.html"));
    await access(join(output, "privacy", "index.html"));
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});
