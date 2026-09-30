import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { buildWebsite, releaseNotes, rootPackage } from "./build.mjs";
import { renderDocument, renderReleaseNotes } from "./release-notes.mjs";

test("documents preserve heading structure and private email links while escaping unsafe content", () => {
  const html = renderDocument(
    "# Privacy statement\n\n## Contact\n\n[Email us](mailto:admin@8westventures.com)\n\n<script>alert(1)</script> [bad](javascript:alert(1))",
    "https://github.com/Seckcey/plenipo/blob/main/apps/website/legal/privacy.md",
  );
  assert.ok(html.includes("<h1>Privacy statement</h1>") && html.includes("<h2>Contact</h2>"));
  assert.ok(html.includes('<a href="mailto:admin@8westventures.com">Email us</a>'));
  assert.ok(!html.includes("<script>") && !html.includes("javascript:"));
  assert.ok(html.includes("&lt;script&gt;alert(1)&lt;/script&gt;"));
  const notes = renderReleaseNotes(
    "# v1.0.0\n\n[Email us](mailto:admin@8westventures.com)",
    "1.0.0",
  );
  assert.ok(!notes.includes('href="mailto:'), "Release note link rules stay unchanged");
});

test("release notes render their title, intro, and the rest under Read the full release notes", () => {
  const html = renderReleaseNotes(
    [
      "# v9.8.7 — A **big** one",
      "",
      "First line of the intro",
      "wraps here. Uses `code` and [a link](https://example.com/a).",
      "",
      "## Highlights",
      "",
      "- **One:** first item",
      "  continues here.",
      "  - a sub-item",
      "- Two, see [ADR-1](../adr/ADR-001.md).",
      "",
      "1. ordered",
      "",
      "```",
      "<raw> & code",
      "```",
    ].join("\n"),
    "9.8.7",
  );
  assert.match(html, /<h3 class="release-title">A <strong>big<\/strong> one<\/h3>/);
  assert.match(
    html,
    /<p>First line of the intro wraps here\. Uses <code>code<\/code> and <a href="https:\/\/example\.com\/a">a link<\/a>\.<\/p>/,
  );
  const [intro, rest] = html.split("<summary>Read the full release notes</summary>");
  assert.ok(!intro.includes("Highlights") && rest.includes("<h4>Highlights</h4>"));
  assert.match(
    rest,
    /<li><strong>One:<\/strong> first item continues here\.<ul><li>a sub-item<\/li><\/ul><\/li>/,
  );
  // A relative link points at the file in that release's own tag on GitHub.
  assert.ok(
    rest.includes('href="https://github.com/Seckcey/plenipo/blob/v9.8.7/docs/adr/ADR-001.md"'),
  );
  assert.match(rest, /<ol><li>ordered<\/li><\/ol>/);
  assert.ok(rest.includes("<pre><code>&lt;raw&gt; &amp; code</code></pre>"));
  assert.ok(html.includes('href="https://github.com/Seckcey/plenipo/releases/tag/v9.8.7"'));
});

test("release notes never pass HTML or unsafe links through", () => {
  const html = renderReleaseNotes(
    [
      "# v1.0.0",
      "",
      '<script>alert(1)</script> <img src=x onerror="y"> **<b>bold</b>**',
      '[click](javascript:alert(1)) [data](data:text/html,hi) ["quote](https://e.com/?a="b)',
    ].join("\n"),
    "1.0.0",
  );
  assert.ok(!/<script|<img|<b>|javascript:|data:text/.test(html));
  assert.ok(html.includes("&lt;script&gt;alert(1)&lt;/script&gt;"));
  assert.ok(html.includes("<strong>&lt;b&gt;bold&lt;/b&gt;</strong>"));
  // The unsafe links keep their words as plain text.
  assert.ok(html.includes("click") && html.includes("data"));
  assert.equal([...html.matchAll(/<a /g)].length, 2, "only the https link and the GitHub link");
  assert.ok(!html.includes('"b"'));
});

test("the page shows the notes of the version it is built for", async () => {
  const output = await mkdtemp(join(tmpdir(), "plenipo-website-"));
  try {
    const product = JSON.parse(await readFile(rootPackage, "utf8")).version;
    await buildWebsite(output, { env: {} });
    const html = await readFile(join(output, "index.html"), "utf8");
    const notes = await releaseNotes(product, { env: {} });
    assert.ok(notes, `docs/releases/v${product}.md exists for the product version`);
    const title = notes.split("\n")[0].replace(/^# */, "");
    assert.ok(html.includes(`What&rsquo;s new in v${product}`));
    assert.ok(html.includes(`<h3 class="release-title">`));
    assert.ok(html.includes(`<h3 class="release-title">${title.replace(`v${product} — `, "")}`));
    assert.ok(!html.includes("__RELEASE_NOTES__"));
    const release = JSON.parse(await readFile(join(output, "release.json"), "utf8"));
    assert.equal(release.releaseNotes, true);
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});

test("a build without notes for its version links to GitHub; a named notes file must exist", async () => {
  const output = await mkdtemp(join(tmpdir(), "plenipo-website-"));
  try {
    await buildWebsite(output, { env: { PLENIPO_VERSION: "9.8.7" } });
    let html = await readFile(join(output, "index.html"), "utf8");
    assert.ok(html.includes("The notes for v9.8.7 are on GitHub."));
    let release = JSON.parse(await readFile(join(output, "release.json"), "utf8"));
    assert.equal(release.releaseNotes, false);

    const file = join(output, "notes.md");
    await writeFile(file, "# v9.8.7 — Named notes\n\nFrom a named file.\n");
    await buildWebsite(output, { env: { PLENIPO_VERSION: "9.8.7", PLENIPO_RELEASE_NOTES: file } });
    html = await readFile(join(output, "index.html"), "utf8");
    assert.ok(
      html.includes('<h3 class="release-title">Named notes</h3>') &&
        html.includes("<p>From a named file.</p>"),
    );
    release = JSON.parse(await readFile(join(output, "release.json"), "utf8"));
    assert.equal(release.releaseNotes, true);

    await assert.rejects(
      buildWebsite(output, {
        env: { PLENIPO_VERSION: "9.8.7", PLENIPO_RELEASE_NOTES: join(output, "missing.md") },
      }),
      /PLENIPO_RELEASE_NOTES names/,
    );
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});
