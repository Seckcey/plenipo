// The release's latest.json (ADR-038) has the layout Plenipo reads, and refuses broken input.
import assert from "node:assert/strict";
import { test } from "node:test";

import { manifest, notesBody, PLATFORM } from "./update-manifest.mjs";

const good = {
  version: "1.10.0",
  installerName: "Plenipo_1.10.0_x64-setup.exe",
  signature: "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==\n",
  notes: "Fixes.",
  pubDate: "2026-10-01T12:00:00.000Z",
};

test("the manifest names the version, its notes, and the installer on GitHub Releases", () => {
  const m = manifest(good);
  assert.equal(m.version, "1.10.0");
  assert.equal(m.notes, "Fixes.");
  assert.equal(m.pub_date, "2026-10-01T12:00:00.000Z");
  assert.deepEqual(Object.keys(m.platforms), [PLATFORM]);
  assert.equal(m.platforms[PLATFORM].signature, "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==");
  assert.equal(
    m.platforms[PLATFORM].url,
    "https://github.com/Seckcey/plenipo/releases/download/v1.10.0/Plenipo_1.10.0_x64-setup.exe",
  );
});

test("broken input is refused", () => {
  assert.throws(() => manifest({ ...good, version: "1.10.0-beta.1" }));
  assert.throws(() => manifest({ ...good, version: "v1.10.0" }));
  assert.throws(() => manifest({ ...good, installerName: "evil.exe" }));
  assert.throws(() => manifest({ ...good, installerName: "../x-setup.exe/" }));
  assert.throws(() => manifest({ ...good, signature: "  " }));
  assert.throws(() => manifest({ ...good, signature: "abc def" }));
});

test("the notes drop their title line", () => {
  assert.equal(notesBody("# v1.10.0 — Title\n\nBody line.\n"), "Body line.");
});
