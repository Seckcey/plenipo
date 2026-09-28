import assert from "node:assert/strict";
import { test } from "node:test";

import { linkedPaths } from "./check-doc-links.mjs";

test("a relative link is read from the file's own folder", () => {
  assert.deepEqual(linkedPaths("docs/adr/README.md", "[ADR-038](ADR-038-updates.md#6-the-key)"), [
    "docs/adr/ADR-038-updates.md",
  ]);
  assert.deepEqual(linkedPaths("docs/roadmap.md", "[plan](../ROLLOUT_PLAN.md)"), [
    "ROLLOUT_PLAN.md",
  ]);
});

test("web links, links within the page, and code are left alone", () => {
  const text = [
    "[site](https://plenipo.8westit.com/) [mail](mailto:a@b.c) [top](#top)",
    "`[not](a-link.md)`",
    "```",
    "[not](a-link-either.md)",
    "```",
  ].join("\n");
  assert.deepEqual(linkedPaths("README.md", text), []);
});

test("a link with spaces written in angle brackets or as %20 is found", () => {
  assert.deepEqual(linkedPaths("README.md", "[a](<docs/a b.md>) [b](docs/a%20b.md)"), [
    "docs/a b.md",
    "docs/a b.md",
  ]);
});
