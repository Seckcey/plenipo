// The raw-color patterns (scripts/colors.mjs) catch what the docs say, and nothing else.
import assert from "node:assert/strict";
import { test } from "node:test";

import { COLOR_KEYS, cssRawColors, TS_NAMED, TS_RAW } from "./colors.mjs";

const found = (line) => [...line.matchAll(cssRawColors())].length > 0;

test("CSS: hex, encoded hex, color functions, and named colors are refused", () => {
  for (const line of [
    ".a { color: #fff; }",
    ".a { background: url(\"data:image/svg+xml,<svg stroke='%238a9ab3'/>\"); }",
    ".a { color: rgba(0, 0, 0, 0.5); }",
    ".a { background: oklch(0.7 0.1 250); }",
    ".a { color: lab(50% 40 59); }",
    ".a { color: red }",
    ".a{color:red}",
    ".a { color: Crimson; }",
    ".a { border: 1px solid tomato; }",
    ".a { color: var(--ui-x, red); }",
    ".a { color: white !important; }",
  ])
    assert.ok(found(line), line);
});

test("CSS: tokens, keywords, and names that are not colors pass", () => {
  for (const line of [
    ".a { color: var(--ui-ok); }",
    ".a { background: transparent; }",
    ".a { color: currentColor; }",
    ".a:not(.red) { grid-area: map; }",
    ".a { animation: topo-flow 1.1s linear infinite; }",
    ".a { color-scheme: dark; }",
    '.a { font-family: "Segoe UI", system-ui; }',
    ".a { background: color-mix(in srgb, var(--ui-ok) 20%, transparent); }",
  ])
    assert.ok(!found(line), line);
});

test("TypeScript: strings with hex or color functions, and named colors where a color goes", () => {
  for (const value of ["#fff", "%23123456", "rgb(1,2,3)", "oklch(0.7 0.1 250)", "hsl(0 0% 0%)"])
    assert.ok(TS_RAW.test(value), value);
  assert.ok(!TS_RAW.test("var(--ui-accent)"));
  for (const value of ["red", "Crimson", "1px solid tomato", "white !important"])
    assert.ok(TS_NAMED.test(value), value);
  for (const value of ["var(--ui-text-primary)", "none", "transparent", "reduced"])
    assert.ok(!TS_NAMED.test(value), value);
  for (const key of ["color", "backgroundColor", "borderLeftColor", "fill", "stroke", "stopColor"])
    assert.ok(COLOR_KEYS.test(key), key);
  // "navy" is a title set's id, not a color: only style keys are checked for names.
  for (const key of ["title", "id", "set", "label"]) assert.ok(!COLOR_KEYS.test(key), key);
});
