// Phase 11A end-to-end: Free and Pro in the real app (ADR-021, ADR-022, ADR-110, ADR-115,
// ADR-116). A new copy is on Free and never contacts 8 West; a second organization and
// Connections say they are part of Pro; entering a license key in Settings → License turns Pro
// on at once, and the weekly check sends exactly the key's ID and the version; an ended
// subscription goes back to Free with nothing taken away; removing the key is Free too.
//
// It needs a copy built for the tests (CI's E2E job): one that trusts the license contract's test
// key (`--features license-test-keys`) and sends its weekly check to a stand-in for 8 West on this
// machine (PLENIPO_LICENSE_STAND_IN=http://127.0.0.1:8768). The stand-in here signs its answers
// with the contract's published test key; a released copy trusts neither.

import assert from "node:assert/strict";
import { createPrivateKey, sign } from "node:crypto";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { join, resolve } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  TEST_LICENSE_KEY,
  clickButton,
  launch,
  makeHome,
  openSettings,
  screenshot as save,
  waitForShell,
  waitUntil,
} from "../lib/app.mjs";

const root = resolve(import.meta.dirname, "../../..");
const CONTRACT = join(root, "contracts", "license-check", "v1");
const SIGNER = JSON.parse(readFileSync(join(CONTRACT, "test-signing-key.json"), "utf8"));
const PORT = 8768;
/** Plenipo's first look for a due check waits this long after it starts (license_host.rs). */
const FIRST_LOOK_MS = 30_000;

const home = makeHome();

const screenshot = async (browser, name) => {
  await browser.pause(600);
  await save(browser, name);
};

const textOf = (browser, selector) =>
  browser.execute((s) => document.querySelector(s)?.innerText.replace(/\s+/g, " ") ?? "", selector);

const waitForText = async (browser, selector, needle, timeoutMs = 20_000) => {
  try {
    return await waitUntil(
      async () => (await textOf(browser, selector)).includes(needle),
      `"${needle}" in ${selector}`,
      timeoutMs,
    );
  } catch (error) {
    console.error(`--- ${selector} showed ---\n${await textOf(browser, selector)}`);
    throw error;
  }
};

/** Call one of Plenipo's commands from the page: its answer or its refusal. */
const invoke = (browser, cmd, args = {}) =>
  browser.executeAsync(
    (c, a, done) => {
      window.__TAURI_INTERNALS__.invoke(c, a).then(
        (ok) => done({ ok }),
        (e) => done({ refused: typeof e === "string" ? e : JSON.stringify(e) }),
      );
    },
    cmd,
    args,
  );

// ---- A stand-in for 8 West's weekly check -------------------------------------------------

const b64url = (buf) => Buffer.from(buf).toString("base64url");
const signingKey = createPrivateKey({
  key: Buffer.concat([
    Buffer.from("302e020100300506032b657004220420", "hex"),
    Buffer.from(SIGNER.seedHex, "hex"),
  ]),
  format: "der",
  type: "pkcs8",
});

/** A signed answer, as the account service sends it (contracts/license-check/v1/README.md). */
function answer(keyId, state) {
  const now = Math.floor(Date.now() / 1000);
  const payload = {
    v: 1,
    key_id: keyId,
    state,
    paid_through: state === "ended" ? now - 86_400 : now + 30 * 86_400,
    ends_at: state === "ended" ? now - 86_400 : null,
    as_of: now,
    signer: SIGNER.id,
  };
  const body = b64url(JSON.stringify(payload));
  const signature = b64url(
    sign(null, Buffer.from(`plenipo-license-answer.v1.${body}`), signingKey),
  );
  return JSON.stringify({ answer: body, signature });
}

const service = {
  /** Every request, as it arrived. */
  seen: [],
  /** What 8 West says next. */
  state: "active",
  server: null,
};

function startService() {
  service.server = createServer((req, res) => {
    const chunks = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      const body = Buffer.concat(chunks).toString("utf8");
      service.seen.push({ method: req.method, url: req.url, headers: req.headers, body });
      let asked = {};
      try {
        asked = JSON.parse(body);
      } catch {
        // Answered below with 400.
      }
      if (req.method !== "POST" || req.url !== "/v1/check" || !asked.key_id) {
        res.writeHead(400).end();
        return;
      }
      res.writeHead(200, { "content-type": "application/json" });
      res.end(answer(asked.key_id, service.state));
    });
  });
  return new Promise((done) => service.server.listen(PORT, "127.0.0.1", done));
}

// ---- The tests ----------------------------------------------------------------------------------

describe("Phase 11A Free and Pro (real app)", () => {
  let app;
  let startedAt;
  let keyId;
  let version;

  before(async () => {
    await startService();
    startedAt = Date.now();
    app = await launch(home, {});
    await app.browser.setWindowSize(1400, 900);
  });
  after(async () => {
    await app?.close();
    service.server?.close();
  });

  it("starts on Free, says what Free includes, and never contacts 8 West", async () => {
    const { browser } = app;
    await waitForShell(browser, undefined, { edition: "free" });
    version = (await invoke(browser, "get_app_info")).ok.version;
    const license = (await invoke(browser, "get_license")).ok;
    assert.equal(license.edition, "free");
    assert.equal(license.reason, "noKey");
    assert.equal(license.testBuild, true, "a copy built for the tests says so");
    await openSettings(browser, "License");
    await waitForText(browser, ".settings-license", "You're on Free: 1 organization");
    await waitForText(browser, ".settings-license", "A Free copy never contacts 8 West");
    await screenshot(browser, "license-free");
    // Plenipo's first look for a due check has passed, and nothing was sent.
    const wait = FIRST_LOOK_MS + 5_000 - (Date.now() - startedAt);
    if (wait > 0) await browser.pause(wait);
    const now = (await invoke(browser, "check_license_now")).ok;
    assert.equal(now.edition, "free");
    assert.deepEqual(service.seen, [], "a Free copy sends nothing to 8 West");
  });

  it("says a second organization and Connections are part of Pro, with nothing made", async () => {
    const { browser } = app;
    const refused = await invoke(browser, "create_organization", {
      name: "Client Co",
      start: { kind: "scratch" },
    });
    assert.match(refused.refused ?? "", /"kind":"partOfPro"/);
    assert.match(refused.refused, /Free has one organization/);
    const listing = (await invoke(browser, "get_organizations")).ok;
    assert.equal(listing.organizations.length, 1, "nothing was made");
    await openSettings(browser, "Organization");
    await waitForText(browser, ".settings-layout__panel", "More than one organization is part");
    await screenshot(browser, "license-free-organizations");
    await openSettings(browser, "Connections");
    await waitForText(browser, ".settings-layout__panel", "Part of Plenipo Pro.");
    await screenshot(browser, "license-free-connections");
  });

  it("turns Pro on with a license key, and the check sends only the key's ID and the version", async () => {
    const { browser } = app;
    await openSettings(browser, "License");
    const box = await browser.$('//label[.//span[normalize-space()="License key"]]//input');
    await box.waitForExist({ timeout: 10_000 });
    assert.equal(await box.getAttribute("type"), "password", "the key is never shown");
    await box.setValue(TEST_LICENSE_KEY);
    await clickButton(browser, "Enter the key");
    // Pro at once, then the first check with 8 West's stand-in.
    await waitUntil(() => service.seen.length === 1, "the first check", 30_000);
    const [check] = service.seen;
    keyId = (await invoke(browser, "get_license")).ok.keyId;
    assert.match(keyId, /^lk_[0-9A-Z]{26}$/);
    assert.equal(check.method, "POST");
    assert.equal(check.url, "/v1/check");
    assert.equal(
      check.body,
      `{"key_id":"${keyId}","app_version":"${version}"}`,
      "byte for byte: the key's ID and the version, nothing else",
    );
    assert.ok(!check.body.includes(TEST_LICENSE_KEY.slice(9, 40)), "never the key itself");
    assert.equal(check.headers["content-type"], "application/json");
    assert.equal(check.headers.cookie, undefined);
    await waitForText(browser, ".settings-license", "Pro is paid through");
    await waitForText(browser, ".settings-license", keyId);
    assert.equal(await box.getValue(), "", "the box is emptied");
    const page = await textOf(browser, ".settings-license");
    assert.ok(!page.includes(TEST_LICENSE_KEY.slice(9, 40)), "the key is never shown");
    await screenshot(browser, "license-pro");
    // Pro: a second organization can be made.
    const made = await invoke(browser, "create_organization", {
      name: "Client Co",
      start: { kind: "scratch" },
    });
    assert.ok(made.ok, JSON.stringify(made));
  });

  it("goes back to Free when the subscription ends, and nothing is taken away", async () => {
    const { browser } = app;
    service.state = "ended";
    await clickButton(browser, "Check now");
    await waitForText(browser, ".settings-license", "Your Pro subscription ended");
    assert.equal(service.seen.length, 2);
    const license = (await invoke(browser, "get_license")).ok;
    assert.equal(license.edition, "free");
    assert.equal(license.reason, "ended");
    const listing = (await invoke(browser, "get_organizations")).ok;
    assert.equal(listing.organizations.length, 2, "the organization made on Pro is kept");
    await screenshot(browser, "license-ended");
    // 8 West says it is paid again: Pro comes back with the same key.
    service.state = "active";
    await clickButton(browser, "Check now");
    await waitForText(browser, ".settings-license", "Pro is paid through");
  });

  it("removes the key after asking: Free, with the key gone from the Vault", async () => {
    const { browser } = app;
    await clickButton(browser, "Remove the key");
    await waitForText(browser, ".settings-license", "Nothing you made is deleted");
    const confirm = await browser.$$('//button[normalize-space()="Remove the key"]');
    await confirm[confirm.length - 1].click();
    await waitForText(browser, ".settings-license", "You're on Free");
    const license = (await invoke(browser, "get_license")).ok;
    assert.equal(license.keyId, null);
    const before = service.seen.length;
    await invoke(browser, "check_license_now");
    assert.equal(service.seen.length, before, "no key, no check");
  });
});
