// Phase 13 end-to-end: keeping Plenipo dependable (ADR-037, background work; ADR-038, updates),
// in the real app. The top bar shows the version; Settings → Start and close keeps its choices,
// with Start with Windows off to begin with; after Plenipo is ended the hard way, the next start
// says how it last stopped; Diagnostics backs up the Ledger, lists the backups, and saves a
// diagnostics file that holds its log but nothing typed in the terminal; and Settings → Updates
// finds a new version, says so in the top bar, and a failed install leaves this version working.
//
// The update test needs a copy built with a throwaway updater key that looks for updates on this
// machine only (CI's E2E job builds one: PLENIPO_UPDATER_PUBLIC_KEY, PLENIPO_UPDATE_ENDPOINT, and
// PLENIPO_E2E_UPDATER_KEY for the private half). Installing is Windows only: the Windows job's
// installer tests install a real update.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { basename, join, resolve } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  APP,
  appPids,
  clickButton,
  launch,
  makeHome,
  nav,
  openSettings,
  screenshot as save,
  waitForShell,
  waitPidGone,
  waitUntil,
  WORDS,
} from "../lib/app.mjs";

const root = resolve(import.meta.dirname, "../../..");
const VERSION = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const UPDATER_KEY = process.env.PLENIPO_E2E_UPDATER_KEY;
const UPDATE_PORT = 8765;

const home = makeHome();

const textOf = (browser, selector) =>
  browser.execute((s) => document.querySelector(s)?.innerText.replace(/\s+/g, " ") ?? "", selector);

const waitForText = async (browser, selector, needle, timeoutMs) => {
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

const screenshot = async (browser, name) => {
  await browser.pause(600);
  await save(browser, name);
};

const radio = (browser, label) =>
  browser.$(
    `//label[contains(@class, "check")][.//span[starts-with(normalize-space(), "${label}")]]//input[@type="radio"]`,
  );

const PANEL = 'section[aria-label="Terminal"]';

/** Type a line into the visible terminal, one key at a time, as a person types. */
async function type(browser, line) {
  await waitUntil(
    () =>
      browser.execute(
        (s) => document.activeElement?.closest(s) !== null,
        `${PANEL} .terminal-panel__tab:not([hidden])`,
      ),
    "the keyboard in the terminal",
  );
  for (const key of [...line, "Enter"]) {
    await browser.keys(key);
    await browser.pause(40);
  }
}

/** Every file in a zip, as text (the diagnostics file). */
function zipEntries(path) {
  const script = [
    "import sys, zipfile, json",
    "z = zipfile.ZipFile(sys.argv[1])",
    "print(json.dumps({n: z.read(n).decode('utf-8', 'replace') for n in z.namelist()}))",
  ].join("\n");
  return JSON.parse(execFileSync("python3", ["-c", script, path], { encoding: "utf8" }));
}

describe("Phase 13 keeping Plenipo dependable (real app)", () => {
  let app;

  before(async () => {
    app = await launch(home);
    await app.browser.setWindowSize(1440, 960);
  });
  after(async () => {
    await app?.close();
  });

  it("shows its version, and keeps the Start and close choices (Start with Windows off)", async () => {
    const { browser } = app;
    await waitForShell(browser);
    await waitForText(browser, '[aria-label="Application version"]', `v${VERSION}`);

    await openSettings(browser, "Start and close");
    await waitForText(browser, ".settings-start", WORDS.startAtSignIn);
    const toggle = await browser.$('.settings-start [role="switch"]');
    assert.equal(await toggle.getAttribute("aria-checked"), "false", "off until you turn it on");
    assert.equal(
      await (await radio(browser, `Keep Plenipo in ${WORDS.waitsIn} while work`)).isSelected(),
      true,
    );

    await (await radio(browser, `Always keep Plenipo in ${WORDS.waitsIn}`)).click();
    await waitUntil(
      async () => (await radio(browser, `Always keep Plenipo in ${WORDS.waitsIn}`)).isSelected(),
      "Always keep to be chosen",
    );
    // Kept: another section and back.
    await openSettings(browser, "Updates");
    await openSettings(browser, "Start and close");
    assert.equal(
      await (await radio(browser, `Always keep Plenipo in ${WORDS.waitsIn}`)).isSelected(),
      true,
    );
    await (await radio(browser, `Keep Plenipo in ${WORDS.waitsIn} while work`)).click();
    await waitUntil(
      async () =>
        (await radio(browser, `Keep Plenipo in ${WORDS.waitsIn} while work`)).isSelected(),
      "the starting choice again",
    );
    await screenshot(browser, "settings-start-and-close");

    await openSettings(browser, "About Plenipo");
    await waitForText(browser, "main", VERSION);
  });

  it("says how Plenipo last stopped after it was ended the hard way", async () => {
    // Ended the hard way: the app alone is killed at once, with no chance to stop cleanly.
    // (Closing the test driver also sends the app the signal Quit uses, after which it may stop
    // cleanly, and rightly say nothing.) Only the app's own process: the driver's group is
    // closed as usual below.
    const [pid] = appPids();
    assert.ok(pid, "found the Plenipo process");
    process.kill(pid, "SIGKILL");
    await waitPidGone(pid);
    await app.close();
    app = await launch(home);
    const { browser } = app;
    await app.browser.setWindowSize(1440, 960);
    await waitForShell(browser);
    await waitForText(browser, ".banner--recovery", "Plenipo closed unexpectedly", 30_000);
    await waitForText(browser, ".banner--recovery", "Nothing was running");
    await screenshot(browser, "recovery-closed-unexpectedly");
    await clickButton(browser, "OK");
    await waitUntil(async () => !(await exists(browser, ".banner--recovery")), "the note to go");
    // The choice made before is still there.
    await openSettings(browser, "Start and close");
    assert.equal(
      await (await radio(browser, `Keep Plenipo in ${WORDS.waitsIn} while work`)).isSelected(),
      true,
    );
  });

  it("backs up the Ledger, and saves a diagnostics file without what you typed", async () => {
    const { browser } = app;
    // Something typed in the terminal, which must never reach a log or a diagnostics file.
    const typed = "plenipo-typed-secret";
    await clickButton(browser, "Terminal");
    await clickButton(browser, `Open a terminal on ${WORDS.thisComputer}`);
    await waitUntil(() => exists(browser, `${PANEL} .xterm-rows`), "the terminal to draw");
    await type(browser, `echo ${typed}-$((6 * 7))`);
    await waitForText(browser, `${PANEL} .xterm-rows`, `${typed}-42`, 20_000);
    await clickButton(browser, "Terminal"); // hide the panel

    await nav(browser, "Diagnostics");
    await waitForText(browser, ".backups", "Backups of the Ledger");
    await clickButton(browser, "Create backup");
    await waitForText(browser, ".backups__table", "Made by you", 20_000);
    await screenshot(browser, "diagnostics-backups");

    await clickButton(browser, "Save a diagnostics file");
    await waitForText(browser, ".diagnostics-file", "Saved", 30_000);
    const path = await textOf(browser, ".diagnostics-file code");
    assert.match(basename(path), /^plenipo-diagnostics-.*\.zip$/);
    await screenshot(browser, "diagnostics-file");

    const files = zipEntries(path);
    const names = Object.keys(files);
    for (const name of ["README.txt", "about.json", "recent-events.json"]) {
      assert.ok(names.includes(name), `${name} in ${names.join(", ")}`);
    }
    assert.ok(
      names.some((n) => n.startsWith("logs/")),
      "the log files",
    );
    assert.match(
      files["about.json"],
      new RegExp(`"version":\\s*"${VERSION.replaceAll(".", "\\.")}"`),
    );
    for (const [name, text] of Object.entries(files)) {
      assert.ok(!text.includes(typed), `${name} holds nothing typed in the terminal`);
    }
    // Nor does the log itself.
    const logs = join(home, ".local", "share", "com.eightwest.plenipo", "logs", "plenipo.log");
    assert.ok(!readFileSync(logs, "utf8").includes(typed), "the log holds nothing typed");
  });

  it("finds a new version and says so; a copy not in an AppImage downloads it by hand", async (t) => {
    if (!UPDATER_KEY) {
      t.skip("needs a copy built with a throwaway updater key (PLENIPO_E2E_UPDATER_KEY)");
      return;
    }
    const { browser } = app;
    const [major, minor, patch] = VERSION.split(".").map(Number);
    const next = `${major}.${minor}.${patch + 1}`;
    const dir = join(home, "releases");
    mkdirSync(dir, { recursive: true });
    const installer = `Plenipo_${next}_x64-setup.exe`;
    copyFileSync(APP, join(dir, installer)); // any file: it is signed, then refused to run here
    const tauri = join(root, "apps", "desktop", "node_modules", "@tauri-apps", "cli", "tauri.js");
    execFileSync(
      process.execPath,
      [
        tauri,
        "signer",
        "sign",
        "-f",
        UPDATER_KEY,
        "--password=",
        "--app-version",
        next,
        join(dir, installer),
      ],
      { stdio: "ignore" },
    );
    const manifest = {
      version: next,
      notes: `Plenipo ${next}: a test update.\n\n- One fix.`,
      pub_date: new Date().toISOString(),
      platforms: {
        // The system this test runs on, as Plenipo names it (Phase 23).
        [`${{ win32: "windows", darwin: "darwin", linux: "linux" }[process.platform]}-${process.arch === "arm64" ? "aarch64" : "x86_64"}`]:
          {
            signature: readFileSync(join(dir, `${installer}.sig`), "utf8").trim(),
            url: `http://127.0.0.1:${UPDATE_PORT}/${installer}`,
          },
      },
    };
    writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
    const server = createServer((req, res) => {
      const name = decodeURIComponent((req.url ?? "/").slice(1));
      try {
        const body = readFileSync(join(dir, basename(name)));
        res.writeHead(200).end(body);
      } catch {
        res.writeHead(404).end();
      }
    });
    await new Promise((done) => server.listen(UPDATE_PORT, "127.0.0.1", done));
    t.after(() => server.close());

    await openSettings(browser, "Updates");
    await waitForText(browser, ".settings-updates", `This version ${VERSION}`);
    await clickButton(browser, "Check now");
    // Not in an AppImage: a copy updated by hand (Phase 23, ADR-152).
    await waitForText(browser, ".settings-updates", `Plenipo ${next} is ready to download`, 30_000);
    await waitForText(browser, ".settings-updates__notes", "One fix.");
    // The top bar follows the Ledger's "a new version is ready" event.
    await waitUntil(() => exists(browser, "button.shell__update"), "Update ready in the top bar");
    await screenshot(browser, "settings-updates-ready");

    const button = async (label) =>
      (
        await browser.$(
          `//*[contains(@class, "settings-updates")]//button[normalize-space()="${label}"]`,
        )
      ).isExisting();
    await waitUntil(() => button("Download the new version"), "the Download button");
    assert.equal(await button("Install now"), false);
    // This version keeps working.
    await waitForText(browser, '[aria-label="Application version"]', `v${VERSION}`);
    await nav(browser, "Home");
    await waitForShell(browser);
  });
});
