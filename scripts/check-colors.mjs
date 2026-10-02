#!/usr/bin/env node
// Refuses raw color values in the desktop app's styles (ADR-030 §3). Every color comes from the
// design tokens: `var(--ui-…)`. Colors are written only in packages/ui/src/tokens.ts.
// TypeScript is checked by ESLint (eslint.config.js); this checks CSS, which ESLint does not read.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { cssRawColors } from "./colors.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Where feature styles live. */
const ROOTS = ["apps/desktop/src", "apps/remote/src", "packages/ui/src/styles"];

// Hex colors (also URL-encoded, %23…), color functions (rgb() to oklch()), and every CSS named
// color used as a value. The patterns live in scripts/colors.mjs, shared with ESLint.
const RAW = cssRawColors();

function* cssFiles(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* cssFiles(path);
    else if (name.endsWith(".css")) yield path;
  }
}

const problems = [];
for (const base of ROOTS) {
  for (const file of cssFiles(join(root, base))) {
    const lines = readFileSync(file, "utf8").split(/\r?\n/);
    lines.forEach((line, i) => {
      const code = line.replace(/\/\*.*?\*\//g, "");
      for (const match of code.matchAll(RAW)) {
        problems.push(`${relative(root, file)}:${i + 1}: raw color "${match[0].trim()}"`);
      }
    });
  }
}

if (problems.length) {
  console.error(
    "Raw colors found. Use a design token instead, e.g. var(--ui-accent) (ADR-030):\n  " +
      problems.join("\n  "),
  );
  process.exit(1);
}
console.log("Color check passed: styles use design tokens only.");
