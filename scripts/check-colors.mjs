#!/usr/bin/env node
// Refuses raw color values in the desktop app's styles (ADR-029 §3). Every color comes from the
// design tokens: `var(--ui-…)`. Colors are written only in packages/ui/src/tokens.ts.
// TypeScript is checked by ESLint (eslint.config.js); this checks CSS, which ESLint does not read.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Where feature styles live. */
const ROOTS = ["apps/desktop/src", "packages/ui/src/styles"];

// #rgb, #rrggbb(aa); rgb()/rgba()/hsl()/hsla(); and CSS named colors used as values.
const NAMED =
  "white|black|red|green|blue|yellow|orange|purple|pink|gray|grey|silver|maroon|navy|teal|olive|lime|aqua|fuchsia|cyan|magenta";
const RAW = new RegExp(
  `(#[0-9a-fA-F]{3,8}\\b)|\\b(rgba?|hsla?)\\s*\\(|:\\s*(?:[^;{}]*\\s)?(${NAMED})\\s*[;!]`,
  "g",
);

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
    "Raw colors found. Use a design token instead, e.g. var(--ui-accent) (ADR-029):\n  " +
      problems.join("\n  "),
  );
  process.exit(1);
}
console.log("Color check passed: styles use design tokens only.");
