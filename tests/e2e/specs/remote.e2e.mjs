// Phase 14 end-to-end: Plenipo on your phone, in the real app (ADR-140 to ADR-147). A test browser
// plays the phone: Google Chrome with a phone-sized screen, and WebDriver's stand-in for a phone's
// face, fingerprint, or passcode check (a virtual authenticator, which makes real passkeys). The
// phone's page is the real one, built from apps/remote; a stand-in for 8 West's relay runs on this
// machine (`plenipo-test-relay`). Nothing here reaches the real relay, and every name is made up.
//
// The tests: Free sends nothing; Pro and the switch connect the PC to the relay; a cancelled or
// wrong code does not pair; pairing asks "Is this your phone?" on the PC and makes a passkey on
// the phone; the phone approves, and the PC records which phone; an approval answered on the PC
// first shows as answered on the phone; Stop all from the phone; signing in again with the
// passkey; every page on a phone's screen in both themes, with no errors; the PC going offline;
// and a removed phone cut off at once, even when it kept its keys. The relay never sees what the
// phone and the PC say.
//
// It needs a copy built for the tests (CI's E2E job): one that trusts the license contract's test
// key (`--features license-test-keys`) and uses the stand-ins on this machine
// (PLENIPO_LICENSE_STAND_IN=http://127.0.0.1:8768, PLENIPO_REMOTE_STAND_IN=http://127.0.0.1:8769,
// PLENIPO_REMOTE_PAGE=http://localhost:8771); the stand-in relay
// (`cargo build --release -p plenipo-remote --features stand-in --bin plenipo-test-relay`); the
// phone's page built for that relay
// (PLENIPO_RELAY_PHONE=ws://127.0.0.1:8769/plenipo/v1/phone pnpm --filter @plenipo/remote build);
// and Google Chrome with its chromedriver (the runner's, from CHROMEWEBDRIVER).

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  createDecipheriv,
  createECDH,
  createPrivateKey,
  createPublicKey,
  hkdfSync,
  randomBytes,
  sign,
  verify,
} from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, normalize, resolve, sep } from "node:path";
import { after, before, describe, it } from "node:test";

import { remote } from "webdriverio";

import {
  PRO_TEST_KEY,
  appPids,
  clickButton,
  installFakeTools,
  launch,
  makeHome,
  nav,
  objectiveBox,
  openSettings,
  pidAlive,
  screenshot as save,
  waitForShell,
  waitPidGone,
  waitUntil,
} from "../lib/app.mjs";

const root = resolve(import.meta.dirname, "../../..");
const exe = (stem) => (process.platform === "win32" ? `${stem}.exe` : stem);
const RELAY_BIN = resolve(
  process.env.PLENIPO_TEST_RELAY ?? join(root, "target", "release", exe("plenipo-test-relay")),
);
const PAGE_DIR = join(root, "apps", "remote", "dist");
const LICENSE_PORT = 8768;
const RELAY_PORT = 8769;
const PAGE_PORT = 8771;
const NOTICES_PORT = 8772;
const PAGE = `http://localhost:${PAGE_PORT}`;
const PHONE_NAME = "Test phone";
/** An Android phone's Chrome: the page calls it "Android phone", in "Chrome on Android phone". */
const ANDROID =
  "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) " +
  "Chrome/141.0.0.0 Mobile Safari/537.36";

const home = makeHome();
const env = installFakeTools(home);
mkdirSync(join(home, ".plenipo-fake-agent"), { recursive: true });
writeFileSync(join(home, ".plenipo-fake-agent", "auth"), "subscription");
const folder = join(home, "website");
mkdirSync(join(folder, "src"), { recursive: true });
writeFileSync(join(folder, "README.md"), "# Website\nThe company website.\n");

const screenshot = async (browser, name) => {
  await browser.pause(400);
  await save(browser, name);
};

// ---- The PC (the real app) -----------------------------------------------------------------------

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

const exists = (browser, selector) =>
  browser.execute((s) => document.querySelector(s) !== null, selector);

/**
 * Close Plenipo and make sure it is gone: a copy that does not quit in time says why (the end of
 * its own log) and is stopped, so it never stays behind for the next suite.
 */
async function closePc(current) {
  if (!current) return;
  const pids = appPids();
  await current.close();
  for (const pid of pids) {
    try {
      await waitUntil(() => !pidAlive(pid), `Plenipo (${pid}) to quit`, 30_000);
    } catch (error) {
      const log = join(home, ".local", "share", "com.eightwest.plenipo", "logs", "plenipo.log");
      if (existsSync(log)) {
        console.error(
          `--- the end of Plenipo's log ---\n${readFileSync(log, "utf8").slice(-8000)}`,
        );
      }
      process.kill(pid, "SIGKILL");
      throw error;
    }
  }
}

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

const remoteNow = async (browser) => (await invoke(browser, "get_remote")).ok;

/** Everything the PC's Ledger holds, newest first, page by page (Activity → All events). */
async function allEvents(browser) {
  const all = [];
  let before = null;
  for (;;) {
    const page =
      (await invoke(browser, "get_scope_events", { scope: { kind: "all" }, limit: 200, before }))
        .ok ?? [];
    all.push(...page);
    if (page.length < 200) return all;
    before = page[page.length - 1].seq;
  }
}

/** Find `needle` in Activity → All events, pressing "Show older events" as a person would. */
async function findInAllEvents(browser, needle) {
  const all = 'ol[aria-label="All events"]';
  for (let page = 0; page < 10; page += 1) {
    if ((await textOf(browser, all)).includes(needle)) return;
    const older = await browser.$('//button[normalize-space()="Show older events"]');
    if (!(await older.isExisting())) break;
    await older.click();
    await browser.pause(300);
  }
  await waitForText(browser, all, needle);
}

const SWITCH = 'button[role="switch"][aria-label="Use Plenipo from another device"]';

async function setSwitch(browser, on) {
  await openSettings(browser, "Switches");
  const toggle = await browser.$(SWITCH);
  await toggle.waitForExist({ timeout: 10_000 });
  await browser.execute((el) => el.scrollIntoView({ block: "center" }), toggle);
  if ((await toggle.getAttribute("aria-checked")) !== String(on)) await toggle.click();
  await waitUntil(
    async () => (await toggle.getAttribute("aria-checked")) === String(on),
    `the switch ${on ? "on" : "off"}`,
  );
}

// The organization chart, as the Guard tests drive it.
const nodeXPath = (label) => `//button[@data-node-id and starts-with(@aria-label, "${label}")]`;
const nodes = (browser) =>
  browser.execute(() =>
    [...document.querySelectorAll("button[data-node-id]")].map((b) => b.getAttribute("aria-label")),
  );
const waitForNode = (browser, label, timeoutMs = 20_000) =>
  waitUntil(
    async () => (await nodes(browser)).find((l) => l.startsWith(label)) ?? null,
    `node "${label}"`,
    timeoutMs,
  );

async function settle(browser) {
  let last = "";
  let steady = 0;
  await waitUntil(async () => {
    const now = await browser.execute(
      () => document.querySelector(".topology__world")?.style.transform ?? "",
    );
    steady = now === last ? steady + 1 : 0;
    last = now;
    return steady >= 7;
  }, "the camera to settle");
}

async function select(browser, title) {
  await clickButton(browser, "Fit to screen");
  await settle(browser);
  const node = await browser.$(nodeXPath(`${title},`));
  await node.waitForExist({ timeout: 10_000 });
  try {
    await node.click();
  } catch {
    await browser.execute((el) => el.click(), node);
  }
  await waitForText(browser, "aside.inspector h2", title);
  await settle(browser);
}

const field = (browser, form, label, tag = "input") =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//${tag}`);

async function submit(browser, form) {
  const button = await browser.$(`${form} button[type="submit"]`);
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
  await waitUntil(async () => !(await exists(browser, form)), `${form} to close`);
}

/** A Plenipo tool call in the fake agent's objective. */
const tool = (name, args) => `<<tool:${name} ${JSON.stringify(args)}>>`;

// ---- The phone (a test browser) ------------------------------------------------------------------

const phoneText = (phone, selector = "body") =>
  phone.execute((s) => document.querySelector(s)?.innerText.replace(/\s+/g, " ") ?? "", selector);

const phoneSays = async (phone, needle, timeoutMs = 20_000, selector = "body") => {
  try {
    return await waitUntil(
      async () => (await phoneText(phone, selector)).includes(needle),
      `the phone to show "${needle}"`,
      timeoutMs,
    );
  } catch (error) {
    console.error(`--- the phone showed ---\n${await phoneText(phone, selector)}`);
    throw error;
  }
};

/** Tap a button on the phone, by its words or its label, inside `scope` (an XPath) if given. */
async function tap(phone, label, scope = "") {
  const button = await phone.$(
    `${scope}//button[normalize-space()="${label}" or @aria-label="${label}"]`,
  );
  await button.waitForClickable({ timeout: 15_000 });
  // In the middle of the screen, clear of the page's bars along the top and bottom.
  await phone.execute((el) => el.scrollIntoView({ block: "center" }), button);
  await button.click();
}

/** The phone page's text box with this label (the design system's field: a label, then its box). */
const phoneField = (phone, label) =>
  phone.$(`//label[normalize-space()="${label}"]/following-sibling::input[1]`);

/** One of the pages along the bottom, once it has loaded. */
async function page(phone, label, id) {
  await tap(phone, label, '//nav[@aria-label="Pages"]');
  const heading = await phone.$(`#${id}`);
  await heading.waitForExist({ timeout: 20_000 });
}

/** On the phone: Work → a worker's page → Give objective, in words. */
async function giveFromPhone(phone, worker, objective) {
  await page(phone, "Work", "work-title");
  await tap(phone, "Their work", `//li[.//strong[normalize-space()="${worker}"]]`);
  const box = await phone.$(
    `//label[normalize-space()="Objective for ${worker}"]/following-sibling::textarea[1]`,
  );
  await box.waitForExist({ timeout: 15_000 });
  await box.setValue(objective);
  await tap(phone, "Give objective");
}

/** The PC's newest task with this objective, once `ok` says it is in the state wanted. */
const taskOnPc = (browser, objective, ok, what, timeoutMs = 30_000) =>
  waitUntil(
    async () => {
      const tasks = (await invoke(browser, "list_tasks")).ok ?? [];
      const task = tasks.find((t) => t.objective.startsWith(objective));
      return task && ok(task) ? task : null;
    },
    what,
    timeoutMs,
  );

/** The phone's own storage, in the page (the phone's key cannot be read out, only copied). */
const copyKept = (phone, from, to) =>
  phone.executeAsync(
    (a, b, done) => {
      const r = indexedDB.open("plenipo-remote", 1);
      r.onerror = () => done(String(r.error));
      r.onsuccess = () => {
        const db = r.result;
        const tx = db.transaction("keep", "readwrite");
        const store = tx.objectStore("keep");
        const got = store.get(a);
        got.onsuccess = () => {
          if (got.result) store.put(got.result, b);
        };
        tx.oncomplete = () => {
          db.close();
          done(true);
        };
        tx.onerror = () => done(String(tx.error));
      };
    },
    from,
    to,
  );

/** Errors the phone's page wrote to the console since the last look. */
async function pageErrors(phone) {
  const logs = await phone.getLogs("browser");
  return logs.filter((l) => l.level === "SEVERE").map((l) => l.message);
}

async function startPhone() {
  const driverDir = process.env.CHROMEWEBDRIVER;
  const driver = driverDir ? join(driverDir, exe("chromedriver")) : null;
  const phone = await remote({
    logLevel: "error",
    capabilities: {
      browserName: "chrome",
      // The WebAuthn commands (the stand-in for the phone's check) are classic WebDriver.
      "wdio:enforceWebDriverClassic": true,
      ...(driver && existsSync(driver) ? { "wdio:chromedriverOptions": { binary: driver } } : {}),
      "goog:chromeOptions": {
        args: ["--headless=new", "--no-first-run", "--no-default-browser-check"],
        prefs: { "profile.default_content_setting_values.notifications": 1 },
        mobileEmulation: {
          deviceMetrics: { width: 390, height: 844, pixelRatio: 2 },
          userAgent: ANDROID,
        },
      },
      "goog:loggingPrefs": { browser: "ALL" },
    },
  });
  await phone.url(PAGE);
  // A phone's own check (face, fingerprint, or passcode), built in: it always says yes.
  const authenticator = await phone.addVirtualAuthenticator(
    "ctap2",
    "internal",
    true,
    true,
    true,
    true,
  );
  return { phone, authenticator };
}

// ---- A stand-in for 8 West's weekly license check ----------------------------------------------

const LICENSE = join(root, "contracts", "license-check", "v1");
const SIGNER = JSON.parse(readFileSync(join(LICENSE, "test-signing-key.json"), "utf8"));
const signingKey = createPrivateKey({
  key: Buffer.concat([
    Buffer.from("302e020100300506032b657004220420", "hex"),
    Buffer.from(SIGNER.seedHex, "hex"),
  ]),
  format: "der",
  type: "pkcs8",
});
const b64url = (buf) => Buffer.from(buf).toString("base64url");

function licenseAnswer(keyId) {
  const now = Math.floor(Date.now() / 1000);
  const body = b64url(
    JSON.stringify({
      v: 1,
      key_id: keyId,
      state: "active",
      paid_through: now + 30 * 86_400,
      ends_at: null,
      as_of: now,
      signer: SIGNER.id,
    }),
  );
  const signature = b64url(
    sign(null, Buffer.from(`plenipo-license-answer.v1.${body}`), signingKey),
  );
  return JSON.stringify({ answer: body, signature });
}

const license = { checks: 0, server: null };

function startLicense() {
  license.server = createServer((req, res) => {
    const chunks = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      let asked = {};
      try {
        asked = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      } catch {
        // Answered below with 400.
      }
      if (req.method !== "POST" || req.url !== "/v1/check" || !asked.key_id) {
        res.writeHead(400).end();
        return;
      }
      license.checks += 1;
      res.writeHead(200, { "content-type": "application/json" });
      res.end(licenseAnswer(asked.key_id));
    });
  });
  return new Promise((done) => license.server.listen(LICENSE_PORT, "127.0.0.1", done));
}

// ---- The stand-in relay --------------------------------------------------------------------------

const relay = { process: null, lines: [] };

function startRelay() {
  assert.ok(
    existsSync(RELAY_BIN),
    `the relay is not built: ${RELAY_BIN} (cargo build --release -p plenipo-relay --features test-hooks, and PLENIPO_TEST_RELAY to it; or -p plenipo-remote --features stand-in --bin plenipo-test-relay)`,
  );
  relay.process = spawn(RELAY_BIN, [String(RELAY_PORT)], {
    stdio: ["pipe", "pipe", "inherit"],
    // Every test connection comes from this one machine: Plenipo's own relay must not take the
    // suite for one address flooding it (its defaults suit the internet, not a test run).
    env: {
      ...process.env,
      PLENIPO_RELAY_MAX_PER_ADDRESS: "500",
      PLENIPO_RELAY_MAX_NEW_PER_MINUTE: "5000",
      PLENIPO_RELAY_MAX_TRIES_PER_MINUTE: "5000",
    },
  });
  let partial = "";
  relay.process.stdout.on("data", (chunk) => {
    partial += chunk.toString("utf8");
    const lines = partial.split(/\r?\n/);
    partial = lines.pop() ?? "";
    relay.lines.push(...lines.filter(Boolean));
  });
  // The stand-in relay says "the stand-in relay listens on …"; Plenipo's own relay, built for the
  // tests (PLENIPO_TEST_RELAY, ADR-149), says "the relay listens on …". Both print the same lines
  // after that.
  return waitUntil(
    () => relay.lines.some((l) => /^the (stand-in )?relay listens/.test(l)),
    "the relay",
  );
}

/** How many times the relay has said `line` so far. */
const relayCount = (line) => relay.lines.filter((l) => l === line).length;

const relaySays = (line, atLeast = 1, timeoutMs = 30_000) =>
  waitUntil(() => relayCount(line) >= atLeast, `the relay to say "${line}"`, timeoutMs);

/** Whether the relay ever saw `words` in a sealed message it passed. */
async function relaySaw(words) {
  const before = relay.lines.length;
  relay.process.stdin.write(`look ${words}\n`);
  const answer = await waitUntil(
    () => relay.lines.slice(before).find((l) => l === `saw ${words}` || l === `never saw ${words}`),
    `the relay's answer about "${words}"`,
  );
  return answer === `saw ${words}`;
}

// ---- A stand-in for the phones' notice services (part 14C) -----------------------------------

const notices = { got: [], server: null };

function startNotices() {
  notices.server = createServer((req, res) => {
    const chunks = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      notices.got.push({
        method: req.method,
        path: req.url,
        headers: req.headers,
        body: Buffer.concat(chunks),
      });
      res.writeHead(201).end();
    });
  });
  return new Promise((done) => notices.server.listen(NOTICES_PORT, "127.0.0.1", done));
}

/** The phone's own notice keys, as its browser makes them (RFC 8291). */
const phoneNoticeKeys = (() => {
  const ecdh = createECDH("prime256v1");
  ecdh.generateKeys();
  return { ecdh, auth: randomBytes(16) };
})();

/**
 * The phone's notice service is the stand-in here: the browser's own sign-up is replaced by one
 * that gives the stand-in's address and the test's keys (a test browser has no service). It is put
 * back after each page load, already signed up when `signedUp`.
 */
async function standInNoticeService(phone, signedUp) {
  await phone.execute(
    (endpoint, p256dh, auth, signedUp) => {
      const sub = {
        endpoint,
        toJSON: () => ({ endpoint, keys: { p256dh, auth } }),
        unsubscribe: () => {
          current = null;
          return Promise.resolve(true);
        },
      };
      let current = signedUp ? sub : null;
      PushManager.prototype.subscribe = function (options) {
        window.plenipoNoticeKey = new Uint8Array(options.applicationServerKey);
        current = sub;
        return Promise.resolve(sub);
      };
      PushManager.prototype.getSubscription = () => Promise.resolve(current);
      Notification.requestPermission = () => Promise.resolve("granted");
    },
    `http://127.0.0.1:${NOTICES_PORT}/push/phone-1`,
    phoneNoticeKeys.ecdh.getPublicKey().toString("base64url"),
    phoneNoticeKeys.auth.toString("base64url"),
    signedUp,
  );
}

/**
 * The phone opens Plenipo's page at `target`, as a tapped notice does when the page is closed: a
 * new page load (a change after `#` alone would not load the page again).
 */
async function openFromNotice(phone, target) {
  await phone.url("about:blank");
  await phone.url(`${PAGE}/#open=${encodeURIComponent(target)}`);
  await standInNoticeService(phone, true);
}

/** Open a sealed notice the way the phone does (RFC 8291 and RFC 8188), with node's own crypto. */
function openNotice(body) {
  const salt = body.subarray(0, 16);
  assert.equal(body.readUInt32BE(16), 4096, "one record of 4096");
  assert.equal(body[20], 65);
  const theirs = body.subarray(21, 86);
  const ours = phoneNoticeKeys.ecdh.getPublicKey();
  const shared = phoneNoticeKeys.ecdh.computeSecret(theirs);
  const keyInfo = Buffer.concat([Buffer.from("WebPush: info\0"), ours, theirs]);
  const ikm = Buffer.from(hkdfSync("sha256", shared, phoneNoticeKeys.auth, keyInfo, 32));
  const cek = Buffer.from(
    hkdfSync("sha256", ikm, salt, Buffer.from("Content-Encoding: aes128gcm\0"), 16),
  );
  const nonce = Buffer.from(
    hkdfSync("sha256", ikm, salt, Buffer.from("Content-Encoding: nonce\0"), 12),
  );
  const sealed = body.subarray(86);
  const decipher = createDecipheriv("aes-128-gcm", cek, nonce);
  decipher.setAuthTag(sealed.subarray(sealed.length - 16));
  const plain = Buffer.concat([
    decipher.update(sealed.subarray(0, sealed.length - 16)),
    decipher.final(),
  ]);
  assert.equal(plain[plain.length - 1], 2, "the last record");
  return JSON.parse(plain.subarray(0, plain.length - 1).toString("utf8"));
}

/** Check `vapid t=<token>, k=<key>`: the token is signed by that key, for that service. */
function checkSignature(authorization, key, audience) {
  const m = /^vapid t=([^,]+), k=(.+)$/.exec(authorization);
  assert.ok(m, authorization);
  const [, token, k] = m;
  assert.equal(k, key, "signed with the PC's notice key, the one the phone signed up with");
  const [head, claims, signature] = token.split(".");
  const raw = Buffer.from(k, "base64url");
  const publicKey = createPublicKey({
    key: {
      kty: "EC",
      crv: "P-256",
      x: raw.subarray(1, 33).toString("base64url"),
      y: raw.subarray(33, 65).toString("base64url"),
    },
    format: "jwk",
  });
  assert.ok(
    verify(
      "sha256",
      Buffer.from(`${head}.${claims}`),
      { key: publicKey, dsaEncoding: "ieee-p1363" },
      Buffer.from(signature, "base64url"),
    ),
    "the signature is the PC's",
  );
  const said = JSON.parse(Buffer.from(claims, "base64url").toString("utf8"));
  assert.equal(said.aud, audience);
  assert.equal(said.sub, "https://getplenipo.com");
  assert.ok(said.exp * 1000 > Date.now() && said.exp * 1000 < Date.now() + 25 * 3600_000);
}

// ---- The phone's page, served as remote.getplenipo.com would serve it ------------------------------

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".webmanifest": "application/manifest+json",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
};

let pageServer = null;
/** When the last approval the PC showed a notice for appeared (see the notices test). */
let lastApprovalAt = 0;

function startPage() {
  const index = join(PAGE_DIR, "index.html");
  assert.ok(existsSync(index), `the phone's page is not built: ${index}`);
  assert.ok(
    readFileSync(index, "utf8").includes(`ws://127.0.0.1:${RELAY_PORT}`),
    "the phone's page was built for the stand-in relay (PLENIPO_RELAY_PHONE)",
  );
  pageServer = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url ?? "/", PAGE).pathname);
    let file = normalize(join(PAGE_DIR, path === "/" ? "index.html" : path));
    if (!file.startsWith(PAGE_DIR + sep) || !existsSync(file)) file = index;
    res.writeHead(200, {
      "content-type": TYPES[extname(file)] ?? "application/octet-stream",
      "cache-control": "no-store",
    });
    res.end(readFileSync(file));
  });
  return new Promise((done) => pageServer.listen(PAGE_PORT, "127.0.0.1", done));
}

// ---- The tests -----------------------------------------------------------------------------------

describe("Phase 14 Plenipo on your phone (real app, a test browser as the phone)", () => {
  let app;
  let phone;
  let authenticator;

  before(async () => {
    await startLicense();
    await startNotices();
    await startRelay();
    await startPage();
    app = await launch(home, env);
    await app.browser.setWindowSize(1400, 900);
    ({ phone, authenticator } = await startPhone());
  });
  after(async () => {
    await phone?.deleteSession().catch(() => undefined);
    await closePc(app);
    relay.process?.kill();
    license.server?.close();
    notices.server?.close();
    pageServer?.close();
  });

  it("on Free, Devices says it is part of Pro, and the PC sends the relay nothing", async () => {
    const { browser } = app;
    await waitForShell(browser, undefined, { edition: "free" });
    const before = await remoteNow(browser);
    assert.equal(before.pro, false);
    assert.equal(before.comingSoon, false, "a copy built for the tests uses the stand-in relay");
    await openSettings(browser, "Devices");
    await waitForText(browser, ".devices", "Using Plenipo from your phone is part of Pro");
    const add = await browser.$('//button[normalize-space()="Add a phone"]');
    assert.equal(await add.isEnabled(), false);
    await screenshot(browser, "phone-devices-free");
    const refused = await invoke(browser, "start_phone_pairing");
    assert.match(refused.refused ?? "", /part of Pro/);
    // Even with the switch saved on, a Free copy never contacts the relay.
    await invoke(browser, "set_remote_switch", { on: true });
    await browser.pause(6_000);
    assert.equal(relayCount("a PC connected"), 0, "a Free copy sends the relay nothing");
    assert.deepEqual(
      relay.lines.filter((l) => l.startsWith("refused")),
      [],
      "nothing reached the relay to refuse",
    );
    await invoke(browser, "set_remote_switch", { on: false });
  });

  it("on Pro, the switch connects this PC to the relay", async () => {
    const { browser } = app;
    const entered = await invoke(browser, "enter_license_key", { key: PRO_TEST_KEY });
    assert.equal(entered.ok?.edition, "pro", JSON.stringify(entered));
    // The PC shows the relay 8 West's signed weekly answer, so it waits for the first check.
    await waitUntil(
      async () => (await invoke(browser, "get_license")).ok?.lastChecked != null,
      "the first weekly check",
      40_000,
    );
    await setSwitch(browser, true);
    await screenshot(browser, "phone-switch-on");
    await relaySays("a PC connected");
    await openSettings(browser, "Devices");
    await waitForText(browser, ".devices", "Your PC is connected to 8 West’s relay");
    assert.equal((await remoteNow(browser)).remote.connected, true);
  });

  it("a cancelled code or a wrong code does not pair, and nothing is added", async () => {
    const { browser } = app;
    await clickButton(browser, "Add a phone");
    await waitUntil(async () => (await remoteNow(browser)).remote.pairing?.code, "a code");
    const cancelled = (await remoteNow(browser)).remote.pairing.code;
    await clickButton(browser, "Cancel");
    await waitUntil(async () => !(await remoteNow(browser)).remote.pairing, "the code to go");
    for (const code of [cancelled, "0000-0000-0000-0000"]) {
      await phone.url(PAGE);
      const box = await phoneField(phone, "Or type the code");
      await box.waitForExist({ timeout: 15_000 });
      await box.setValue(code);
      await tap(phone, "Pair this phone");
      // A closed mailbox (ADR-212): the page says to look at the PC first, because the code may
      // have been used by another phone the PC is now asking about.
      await phoneSays(
        phone,
        "This code was already used, or mistyped, or your PC stopped adding a phone.",
      );
      await phoneSays(phone, "Look at your PC");
    }
    await screenshot(phone, "phone-pair-wrong-code");
    assert.ok(relayCount("refused mailbox_closed") >= 2);
    assert.deepEqual((await remoteNow(browser)).remote.devices, []);
  });

  it("pairs the phone: the PC asks Is this your phone?, and the phone makes its passkey", async () => {
    const { browser } = app;
    await clickButton(browser, "Add a phone");
    const pairing = await waitUntil(
      async () => (await remoteNow(browser)).remote.pairing,
      "the picture code",
    );
    assert.equal(pairing.step, "showing");
    assert.match(pairing.code, /^[0-9A-Z]{4}-[0-9A-Z]{4}-[0-9A-Z]{4}-[0-9A-Z]{4}$/);
    assert.equal(pairing.link, `${PAGE}/#pair=${pairing.code.replaceAll("-", "")}`);
    await waitForText(browser, ".pairing", pairing.code);
    assert.ok(await exists(browser, 'svg[role="img"][aria-label^="Picture code (QR code)"]'));
    await screenshot(browser, "phone-devices-code");

    // The phone opens the link (what scanning the picture code does), as a fresh page.
    await phone.url("about:blank");
    await phone.url(pairing.link);
    const name = await phoneField(phone, "What to call this phone");
    await name.waitForExist({ timeout: 15_000 });
    assert.equal(await name.getValue(), "Android phone", "the page guesses what the phone is");
    await name.clearValue();
    await name.setValue(PHONE_NAME);
    const typed = await phoneField(phone, "Or type the code");
    assert.equal(await typed.getValue(), pairing.code.replaceAll("-", ""));
    await screenshot(phone, "phone-pair");
    await tap(phone, "Pair this phone");
    await phoneSays(phone, "Your PC is asking: Is this your phone?");
    assert.equal(await phone.getUrl(), `${PAGE}/`, "the code is taken out of the address");

    // The PC asks, and adds nothing until the owner says yes.
    await waitForText(browser, '[role="alertdialog"][aria-labelledby="pairing-ask"]', PHONE_NAME);
    await waitForText(
      browser,
      '[role="alertdialog"][aria-labelledby="pairing-ask"]',
      "Chrome on Android phone",
    );
    // Both screens show the same six digits, from the meeting itself (ADR-212): the PC's
    // "Is this your phone?" and the phone's "Your PC is asking".
    const asking = (await remoteNow(browser)).remote.pairing;
    assert.equal(asking.step, "asking");
    assert.match(asking.check, /^[0-9]{6}$/, "six check digits");
    const digits = `${asking.check.slice(0, 3)} ${asking.check.slice(3)}`;
    await waitForText(browser, '[role="alertdialog"][aria-labelledby="pairing-ask"]', digits);
    await phoneSays(phone, digits);
    assert.deepEqual((await remoteNow(browser)).remote.devices, []);
    await screenshot(browser, "phone-devices-ask");
    await screenshot(phone, "phone-pair-waiting");
    await clickButton(browser, "Add");

    // The phone makes its passkey with its own check, and is signed in.
    await tap(phone, "Set up Face ID, fingerprint, or passcode");
    await (await phone.$("#home-title")).waitForExist({ timeout: 30_000 });
    const credentials = await phone.getCredentials(authenticator);
    assert.equal(credentials.length, 1, "one passkey, on the phone");
    assert.equal(credentials[0].rpId, "localhost");
    await screenshot(phone, "phone-home-dark");

    const devices = await waitUntil(async () => {
      const d = (await remoteNow(browser)).remote.devices;
      return d.length === 1 && d[0].signedIn ? d : null;
    }, "the phone on the PC's list");
    assert.equal(devices[0].name, PHONE_NAME);
    assert.equal(devices[0].browser, "Chrome on Android phone");
    await waitForText(browser, ".devices__list", PHONE_NAME);
    await waitForText(browser, ".devices__list", "Signed in");
    await screenshot(browser, "phone-devices-listed");
  });

  it("the phone approves; the PC records which phone; the relay never sees the words", async () => {
    const { browser } = app;
    await nav(browser, "Organization");
    await clickButton(browser, "Create a department");
    await (await field(browser, "New department", "Name")).setValue("Development");
    await submit(browser, 'form[aria-label="New department"]');
    await waitForNode(browser, "Development Manager, Idle");
    await clickButton(browser, "+ Project");
    await (await field(browser, "New project", "Name")).setValue("Website");
    await (await field(browser, "New project", "Project folder (optional)")).setValue(folder);
    await submit(browser, 'form[aria-label="New project"]');
    await waitForNode(browser, "Website Supervisor, Idle");
    await select(browser, "Website Supervisor");
    await clickButton(browser, "Hire Senior Developer");
    await submit(browser, 'form[aria-label="Hire"]');
    await waitForNode(browser, "Senior Developer,");
    // Two pushes: each waits for an approval, one after the other.
    const work = [tool("git_push", {}), tool("git_push", {})].join(" ");
    await (
      await objectiveBox(browser)
    ).setValue(`Publish the website {{handoff:role:Senior Developer|${work}}}`);
    await clickButton(browser, "Give objective");
    await waitForText(browser, ".banner--approval", "is waiting for your approval", 45_000);

    // The phone hears that something changed, and shows the approval.
    await page(phone, "Approvals", "approvals-title");
    const card = '//li[contains(normalize-space(.), "git push origin")]';
    await waitUntil(() => phone.$(card).isExisting(), "the approval on the phone", 20_000);
    await phoneSays(phone, "Senior Developer");
    await screenshot(phone, "phone-approvals-dark");
    await tap(phone, "Approve", card);
    // Answered: the card moves to "Answered lately", with the PC's own record of who answered.
    await phoneSays(phone, `Approved by you, from ${PHONE_NAME}.`);
    await screenshot(phone, "phone-approvals-answered");

    const queue = await waitUntil(async () => {
      const q = (await invoke(browser, "get_approvals")).ok;
      return q.recent.length >= 1 ? q : null;
    }, "the answer on the PC");
    assert.equal(queue.recent[0].status, "approved");
    assert.equal(queue.recent[0].note, `Approved by you, from ${PHONE_NAME}.`);

    // The relay passed every word sealed: it never saw them.
    for (const words of ["git push", "approve", PHONE_NAME, "Senior Developer", "Website"]) {
      assert.equal(await relaySaw(words), false, `the relay never saw "${words}"`);
    }
  });

  it("an approval answered on the PC first shows as answered on the phone", async () => {
    const { browser } = app;
    // The second push waits for an approval; the phone shows it too.
    await waitUntil(
      async () => (await invoke(browser, "get_approvals")).ok.pending.length === 1,
      "the second approval",
      45_000,
    );
    lastApprovalAt = Date.now();
    await phoneSays(phone, "git push origin");
    await waitUntil(
      () => phone.$('//button[normalize-space()="Approve"]').isExisting(),
      "the phone's Approve",
    );
    // The PC answers first.
    await clickButton(browser, "Review");
    const pcCard = 'article[aria-label^="Senior Developer wants to git push origin"]';
    await waitUntil(() => exists(browser, pcCard), "the approval card");
    await clickButton(browser, "Approve");
    await waitForText(
      browser,
      '[aria-labelledby="waiting-title"]',
      "Nothing is waiting for your approval.",
    );
    // The phone shows it answered, with nothing left to approve.
    await phoneSays(phone, "Nothing is waiting for you.");
    assert.equal(await phone.$('//button[normalize-space()="Approve"]').isExisting(), false);
    const recent = (await invoke(browser, "get_approvals")).ok.recent;
    assert.deepEqual(
      recent.map((a) => a.note),
      ["Approved by you.", `Approved by you, from ${PHONE_NAME}.`],
    );
    await waitForText(browser, '[aria-labelledby="answered-title"]', `from ${PHONE_NAME}`);
    await screenshot(browser, "phone-pc-approvals-answered");
  });

  it("notices: the phone signs up, and the PC seals each notice for it alone, signed with its key (14C)", async () => {
    const { browser } = app;
    // The page's background part is in place.
    const worker = await waitUntil(
      () =>
        phone.executeAsync((done) => {
          navigator.serviceWorker.getRegistration().then(
            (r) => done(r?.active?.scriptURL ?? null),
            () => done(null),
          );
        }),
      "the page's background part",
    );
    assert.equal(worker, `${PAGE}/sw.js`);
    await standInNoticeService(phone, false);
    await page(phone, "More", "more-title");
    const toggle = await phone.$('button[role="switch"][aria-label="Notices on this phone"]');
    await toggle.waitForClickable({ timeout: 15_000 });
    // In the middle of the screen, clear of the page's bar along the bottom.
    await phone.execute((el) => el.scrollIntoView({ block: "center" }), toggle);
    await screenshot(phone, "phone-notices-off");
    await toggle.click();
    await waitUntil(async () => (await toggle.getAttribute("aria-checked")) === "true", "on");
    await waitUntil(
      async () => (await remoteNow(browser)).remote.devices[0].notices,
      "the PC has the phone's notice address",
    );
    await screenshot(phone, "phone-notices-on");
    const signedUpWith = Buffer.from(
      await phone.execute(() => Array.from(window.plenipoNoticeKey ?? [])),
    ).toString("base64url");

    // Notices come while the PC's window is in front too (the PC's own choice).
    await openSettings(browser, "Notifications");
    await waitForText(browser, '[aria-labelledby="notices-phones"]', "1 phone gets");
    const away = await browser.$(
      'button[role="switch"][aria-label="Only while Plenipo\'s window is not in front"]',
    );
    await browser.execute((el) => el.scrollIntoView({ block: "center" }), away);
    if ((await away.getAttribute("aria-checked")) === "true") await away.click();
    await screenshot(browser, "phone-pc-notifications");

    // Something needs the owner: the PC seals a notice for the phone. The PC shows the same
    // words at most once a minute, and the approvals above said the same thing: wait that out.
    const repeat = lastApprovalAt + 61_000 - Date.now();
    if (repeat > 0) await new Promise((done) => setTimeout(done, repeat));
    const before = notices.got.length;
    await nav(browser, "Organization");
    await select(browser, "Website Supervisor");
    await (
      await objectiveBox(browser)
    ).setValue(`Publish once more {{handoff:role:Senior Developer|${tool("git_push", {})}}}`);
    await clickButton(browser, "Give objective");
    await waitUntil(() => notices.got.length > before, "the notice", 60_000);
    const sent = notices.got[notices.got.length - 1];
    assert.equal(sent.method, "POST");
    assert.equal(sent.path, "/push/phone-1");
    assert.equal(sent.headers["content-encoding"], "aes128gcm");
    assert.equal(sent.headers.ttl, "600");
    assert.equal(sent.headers.urgency, "high");
    assert.match(sent.headers.topic, /^[A-Za-z0-9_-]{32}$/);
    assert.equal(sent.headers.cookie, undefined);
    checkSignature(sent.headers.authorization, signedUpWith, `http://127.0.0.1:${NOTICES_PORT}`);
    // Only the phone's keys open it, and it says what the PC's notice says.
    assert.ok(!sent.body.includes(Buffer.from("git push")), "nothing in the clear");
    const notice = openNotice(sent.body);
    assert.equal(notice.v, 1);
    assert.equal(notice.kind, "approvals");
    assert.equal(notice.title, "Senior Developer is waiting for your OK");
    assert.match(notice.body, /^Git push origin/);
    assert.equal(notice.about.kind, "approval");
    const pending = (await invoke(browser, "get_approvals")).ok.pending;
    assert.equal(notice.about.id, pending[0].id, "about the approval that waits");
    assert.equal(notice.tag, `approval:${notice.org}:${notice.about.id}`);
    // Opening the notice opens that approval on the phone.
    await openFromNotice(phone, `approval:${notice.org}:${notice.about.id}`);
    const focused = await phone.$(".approval--focused");
    await focused.waitForExist({ timeout: 30_000 });
    assert.match(await focused.getText(), /git push origin/);
    await screenshot(phone, "phone-notice-opened");
    // Answered on the PC: the phone's page shows it as answered, if the notice is opened again.
    await clickButton(browser, "Review");
    await clickButton(browser, "Approve");
    await waitUntil(
      async () => (await invoke(browser, "get_approvals")).ok.pending.length === 0,
      "answered on the PC",
    );
    await openFromNotice(phone, `approval:${notice.org}:${notice.about.id}`);
    await phoneSays(phone, "Already answered.", 30_000);
    await screenshot(phone, "phone-notice-already-answered");
  });

  it("gives an objective from the phone, and stops the worker from the phone (14B)", async () => {
    const { browser } = app;
    await giveFromPhone(phone, "Website Supervisor", "Count the pages slowly [slow]");
    // Its conversation opens on the phone, with the worker on it.
    await phoneSays(phone, "Count the pages slowly");
    await waitUntil(
      () => phone.$('//button[normalize-space()="Stop the worker"]').isExisting(),
      "the worker on it, on the phone",
      30_000,
    );
    await taskOnPc(browser, "Count the pages slowly", (t) => t.state === "running", "it running");
    await screenshot(phone, "phone-conversation-running");
    await tap(phone, "Stop the worker");
    await tap(phone, "Stop the worker", '//*[@role="alertdialog"]');
    await taskOnPc(
      browser,
      "Count the pages slowly",
      (t) => !["running", "queued"].includes(t.state),
      "the task stopped on the PC",
    );
    await waitUntil(
      async () => !(await phone.$('//button[normalize-space()="Stop the worker"]').isExisting()),
      "the phone to show it stopped",
    );
  });

  it("Stop all from the phone stops all work on the PC", async () => {
    const { browser } = app;
    await page(phone, "Home", "home-title");
    await tap(phone, "Stop all", "//header");
    await (
      await phone.$('[role="alertdialog"][aria-label="Stop all?"]')
    ).waitForExist({
      timeout: 10_000,
    });
    await screenshot(phone, "phone-stop-all-ask");
    await tap(phone, "Stop all", '//*[@role="alertdialog"]');
    await phoneSays(phone, "All work is stopped.");
    await waitForText(browser, ".banner--control", "All work is stopped.");
    // The phone, which takes notices, is told too, sealed for it alone (part 14C).
    const stoppedNotice = await waitUntil(
      () => notices.got.map((n) => openNotice(n.body)).find((n) => n.about?.kind === "stopped"),
      "Stop all's notice",
      30_000,
    );
    assert.equal(stoppedNotice.title, "Stopped: everything");
    await screenshot(browser, "phone-pc-stopped");
    assert.equal((await invoke(browser, "get_control_status")).ok.stopped, true);
    // Allow again, from the phone too (14B).
    await screenshot(phone, "phone-stopped-allow-again");
    await tap(phone, "Allow again", "//header");
    await waitUntil(
      async () => (await invoke(browser, "get_control_status")).ok.stopped === false,
      "work allowed again",
    );
    await (
      await phone.$('//header//button[normalize-space()="Stop all"]')
    ).waitForExist({
      timeout: 15_000,
    });
  });

  it("signs out, then signs in again with the phone's passkey, checked by the PC", async () => {
    const { browser } = app;
    await page(phone, "More", "more-title");
    await tap(phone, "Sign out");
    await phoneSays(phone, "Sign in to");
    await screenshot(phone, "phone-sign-in");
    await waitUntil(
      async () => !(await remoteNow(browser)).remote.devices[0].signedIn,
      "signed out on the PC",
    );
    await tap(phone, "Check it’s you");
    await (await phone.$("#home-title")).waitForExist({ timeout: 30_000 });
    await waitUntil(
      async () => (await remoteNow(browser)).remote.devices[0].signedIn,
      "signed in on the PC",
    );
  });

  it("every page fits a phone's screen in both themes, with no errors", async () => {
    const pages = [
      ["Home", "home-title", "home"],
      ["Approvals", "approvals-title", "approvals"],
      ["Work", "work-title", "work"],
      ["Activity", "activity-title", "activity"],
      ["More", "more-title", "more"],
    ];
    await pageErrors(phone); // what came before is not this test's
    for (const theme of ["dark", "light"]) {
      if (theme === "light") {
        await page(phone, "More", "more-title");
        await tap(phone, "Switch to the light theme");
        await waitUntil(
          () => phone.execute(() => document.documentElement.dataset.theme === "light"),
          "the light theme",
        );
      }
      for (const [label, id, name] of pages) {
        await page(phone, label, id);
        await phone.pause(600);
        const sideways = await phone.execute(
          () => document.documentElement.scrollWidth - window.innerWidth,
        );
        assert.ok(sideways <= 0, `${label} (${theme}) fits the screen's width`);
        await screenshot(phone, `phone-${name}-${theme}`);
      }
    }
    assert.deepEqual(await pageErrors(phone), [], "no errors on the phone's page");
  });

  it("when the PC is off, the phone says so and changes nothing; then the PC comes back", async () => {
    await page(phone, "Home", "home-title");
    const goneBefore = relayCount("no PC connected");
    // The PC is turned off: Plenipo closes, and the relay tells the phone.
    await closePc(app);
    app = null;
    await relaySays("no PC connected", goneBefore + 1);
    await phoneSays(phone, "Your PC can’t be reached. Nothing was changed.");
    await screenshot(phone, "phone-offline");
    // Trying again changes nothing while the PC is off.
    await tap(phone, "Try again");
    await phoneSays(phone, "Your PC can’t be reached. Nothing was changed.");
    // The PC is back: Plenipo starts, still on Pro, with the phone on its list.
    const connectedBefore = relayCount("a PC connected");
    app = await launch(home, env);
    await app.browser.setWindowSize(1400, 900);
    await waitForShell(app.browser, 60_000, { edition: "pro" });
    await relaySays("a PC connected", connectedBefore + 1, 60_000);
    const devices = (await remoteNow(app.browser)).remote.devices;
    assert.deepEqual(
      devices.map((d) => [d.name, d.signedIn]),
      [[PHONE_NAME, false]],
      "kept on the PC's list; sign-ins end when Plenipo closes",
    );
    await tap(phone, "Try again");
    await tap(phone, "Check it’s you");
    await (await phone.$("#home-title")).waitForExist({ timeout: 30_000 });
  });

  it("after Plenipo stops unexpectedly, the phone can run its work again, or leave it stopped (14B)", async () => {
    await giveFromPhone(phone, "Website Supervisor", "Read every page slowly [slow]");
    await taskOnPc(
      app.browser,
      "Read every page slowly",
      (t) => t.state === "running",
      "it running",
    );
    // Plenipo stops with no warning at all.
    const [pid] = appPids();
    process.kill(pid, "SIGKILL");
    await waitPidGone(pid);
    await app.close();
    app = null;
    await phoneSays(phone, "Your PC can’t be reached. Nothing was changed.");
    const connectedBefore = relayCount("a PC connected");
    app = await launch(home, env);
    await app.browser.setWindowSize(1400, 900);
    await waitForShell(app.browser, 60_000, { edition: "pro" });
    await relaySays("a PC connected", connectedBefore + 1, 60_000);
    await tap(phone, "Try again");
    await tap(phone, "Check it’s you");
    // Home says what happened, with the task that stopped.
    const notice = '//section[@aria-labelledby="stopped-title"]';
    await (await phone.$(notice)).waitForExist({ timeout: 30_000 });
    await phoneSays(phone, "Read every page slowly");
    await screenshot(phone, "phone-stopped-unexpectedly");
    await tap(phone, "Run again", notice);
    await phoneSays(phone, "Started again");
    await waitUntil(
      async () =>
        ((await invoke(app.browser, "list_tasks")).ok ?? []).filter((t) =>
          t.objective.startsWith("Read every page slowly"),
        ).length >= 2,
      "the work started again on the PC",
      30_000,
    );
    await tap(phone, "Leave stopped", notice);
    await waitUntil(
      async () => !(await phone.$(notice).isExisting()),
      "the notice to be put away",
      20_000,
    );
    // Put away on the PC too.
    await waitUntil(
      async () => !(await exists(app.browser, '[aria-label="How Plenipo last stopped"]')),
      "the PC's notice to be put away",
      20_000,
    );
  });

  it("a removed phone is cut off at once, and stays refused even with its keys", async () => {
    const { browser } = app;
    // Keep a copy of what the phone holds, as a stolen phone would still have it.
    assert.equal(await copyKept(phone, "phone", "copy"), true);
    await openSettings(browser, "Devices");
    await clickButton(browser, "Remove");
    await waitForText(
      browser,
      `[role="alertdialog"][aria-label="Remove ${PHONE_NAME}?"]`,
      "cut off",
    );
    await screenshot(browser, "phone-devices-remove");
    const confirm = await browser.$$(
      `//*[@role="alertdialog" and @aria-label="Remove ${PHONE_NAME}?"]//button[normalize-space()="Remove"]`,
    );
    await confirm[0].click();
    await waitUntil(async () => (await remoteNow(browser)).remote.devices.length === 0, "removed");
    // Told at once: the phone forgets the PC.
    await (await phone.$("#pair-title")).waitForExist({ timeout: 15_000 });

    // The kept copy comes back: the relay refuses its pass.
    const refusedBefore = relayCount("refused bad_pass");
    assert.equal(await copyKept(phone, "copy", "phone"), true);
    await phone.refresh();
    await phoneSays(phone, "This phone is no longer on your PC’s list");
    await relaySays("refused bad_pass", refusedBefore + 1);
    await screenshot(phone, "phone-removed");

    // The relay keeps nothing: after it meets the PC again it would take the pass once more,
    // so the PC, which no longer knows the phone, has the relay refuse it.
    const goneBefore = relayCount("no PC connected");
    const connectedBefore = relayCount("a PC connected");
    await setSwitch(browser, false);
    // The PC leaves the relay (it looks at the switch every 2 seconds), then meets it again.
    await relaySays("no PC connected", goneBefore + 1);
    await setSwitch(browser, true);
    await relaySays("a PC connected", connectedBefore + 1);
    await waitUntil(async () => (await remoteNow(browser)).remote.connected, "connected again");
    await tap(phone, "Try again");
    await relaySays("refused bad_pass", refusedBefore + 2);
    await phoneSays(phone, "This phone is no longer on your PC’s list");
    assert.deepEqual((await remoteNow(browser)).remote.devices, []);
  });

  it("Activity on the PC shows every request with the phone that sent it", async () => {
    const { browser } = app;
    // The whole record: the phone was added long before the newest 200 events.
    const events = await allEvents(browser);
    const types = (t) => events.filter((e) => e.eventType === t);
    assert.ok(types("remote.device_added").some((e) => e.payload.name === PHONE_NAME));
    const asked = types("remote.request").map((e) => `${e.payload.name}: ${e.payload.kind}`);
    assert.ok(asked.includes(`${PHONE_NAME}: approve`), asked.join("\n"));
    assert.ok(asked.includes(`${PHONE_NAME}: stop all`), asked.join("\n"));
    for (const kind of [
      "send an objective",
      "stop a task",
      "allow again",
      "run a task again",
      "leave a task stopped",
    ]) {
      assert.ok(asked.includes(`${PHONE_NAME}: ${kind}`), `${kind}\n${asked.join("\n")}`);
    }
    assert.ok(types("remote.signed_in").length >= 2, "signed in twice, each recorded");
    assert.ok(types("remote.device_removed").some((e) => e.payload.by === "pc"));
    const sentNotices = types("remote.notice_sent");
    assert.ok(
      sentNotices.some((e) => e.payload.name === PHONE_NAME && e.payload.kind === "approvals"),
    );
    assert.ok(!JSON.stringify(sentNotices).includes("git push"), "never what a notice said");
    await nav(browser, "Activity");
    await (await browser.$('//button[@role="tab" and normalize-space()="All events"]')).click();
    const all = 'ol[aria-label="All events"]';
    await waitForText(browser, all, `You removed ${PHONE_NAME}`);
    // The phone's first requests are older than the newest 200 events the list starts with.
    await findInAllEvents(browser, `${PHONE_NAME} asked to stop all`);
    await findInAllEvents(browser, `Approved: git push origin (from ${PHONE_NAME})`);
    await findInAllEvents(browser, `${PHONE_NAME} asked to approve`);
    await screenshot(browser, "phone-pc-activity");
  });
});
