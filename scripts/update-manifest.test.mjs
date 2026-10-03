// The release's latest.json (ADR-038) has the layout Plenipo reads, and refuses broken input.
import assert from "node:assert/strict";
import { test } from "node:test";

import { manifest, notesBody } from "./update-manifest.mjs";

const windows = {
  system: "windows-x86_64",
  fileName: "Plenipo_1.10.0_x64-setup.exe",
  signature: "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==\n",
};
const linux = {
  system: "linux-x86_64",
  fileName: "Plenipo_1.10.0_amd64.AppImage",
  signature: "bGludXggc2lnbmF0dXJl\n",
};
const good = {
  version: "1.10.0",
  notes: "Fixes.",
  pubDate: "2026-10-01T12:00:00.000Z",
  downloads: [windows],
};

test("the manifest names the version, its notes, and the installer on GitHub Releases", () => {
  const m = manifest(good);
  assert.equal(m.version, "1.10.0");
  assert.equal(m.notes, "Fixes.");
  assert.equal(m.pub_date, "2026-10-01T12:00:00.000Z");
  assert.deepEqual(Object.keys(m.platforms), ["windows-x86_64"]);
  assert.equal(m.platforms["windows-x86_64"].signature, "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==");
  assert.equal(
    m.platforms["windows-x86_64"].url,
    "https://github.com/Seckcey/plenipo/releases/download/v1.10.0/Plenipo_1.10.0_x64-setup.exe",
  );
});

test("Phase 23: Linux's AppImage is listed next to Windows' installer", () => {
  const m = manifest({ ...good, downloads: [windows, linux] });
  assert.deepEqual(Object.keys(m.platforms), ["windows-x86_64", "linux-x86_64"]);
  assert.equal(m.platforms["linux-x86_64"].signature, "bGludXggc2lnbmF0dXJl");
  assert.equal(
    m.platforms["linux-x86_64"].url,
    "https://github.com/Seckcey/plenipo/releases/download/v1.10.0/Plenipo_1.10.0_amd64.AppImage",
  );
});

test("broken input is refused", () => {
  const bad = (change) => () => manifest({ ...good, ...change });
  assert.throws(bad({ version: "1.10.0-beta.1" }));
  assert.throws(bad({ version: "v1.10.0" }));
  assert.throws(bad({ downloads: [] }));
  assert.throws(bad({ downloads: [{ ...windows, fileName: "evil.exe" }] }));
  assert.throws(bad({ downloads: [{ ...windows, fileName: "../x-setup.exe/" }] }));
  assert.throws(bad({ downloads: [{ ...windows, fileName: "Plenipo_1.9.0_x64-setup.exe" }] }));
  assert.throws(bad({ downloads: [{ ...windows, signature: "  " }] }));
  assert.throws(bad({ downloads: [{ ...windows, signature: "abc def" }] }));
  assert.throws(bad({ downloads: [{ ...linux, fileName: "Plenipo_1.10.0_amd64.deb" }] }));
  assert.throws(bad({ downloads: [{ ...linux, system: "darwin-aarch64" }] }));
  assert.throws(bad({ downloads: [{ ...linux, system: "__proto__" }] }));
  assert.throws(bad({ downloads: [windows, windows] }));
});

test("the notes drop their title line", () => {
  assert.equal(notesBody("# v1.10.0 — Title\n\nBody line.\n"), "Body line.");
});
