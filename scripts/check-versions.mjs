#!/usr/bin/env node
// Verifies that every versioned manifest carries the same SemVer version.
// Source of truth: root package.json. See docs/development/versioning.md.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(join(root, p), "utf8");
const semver = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

const expected = JSON.parse(read("package.json")).version;
const found = {
  "package.json": expected,
  "apps/desktop/package.json": JSON.parse(read("apps/desktop/package.json")).version,
  "packages/types/package.json": JSON.parse(read("packages/types/package.json")).version,
  "tests/e2e/package.json": JSON.parse(read("tests/e2e/package.json")).version,
  "Cargo.toml [workspace.package]": read("Cargo.toml").match(
    /\[workspace\.package\][^[]*?\nversion\s*=\s*"([^"]+)"/,
  )?.[1],
};

// tauri.conf.json must defer to apps/desktop/package.json rather than hard-code a version.
const tauriVersion = JSON.parse(read("apps/desktop/src-tauri/tauri.conf.json")).version;

const errors = [];
if (!semver.test(expected)) errors.push(`root version "${expected}" is not valid SemVer`);
for (const [file, version] of Object.entries(found)) {
  if (version !== expected) errors.push(`${file}: "${version}" (expected "${expected}")`);
}
if (tauriVersion !== "../package.json") {
  errors.push(`tauri.conf.json version must be "../package.json", found "${tauriVersion}"`);
}

if (errors.length) {
  console.error("Version check FAILED:\n  " + errors.join("\n  "));
  process.exit(1);
}
console.log(`Version check passed: all manifests at ${expected}`);
