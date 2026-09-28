// Launch the real Plenipo binary under tauri-driver and drive it with WebdriverIO.

import { spawn, execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync } from "node:fs";
import net from "node:net";
import { tmpdir } from "node:os";
import { delimiter, join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

import { remote } from "webdriverio";

const root = resolve(import.meta.dirname, "../../..");
const exeName = (stem) => (process.platform === "win32" ? `${stem}.exe` : stem);
export const APP = resolve(
  process.env.PLENIPO_APP ?? join(root, "target", "release", exeName("plenipo-desktop")),
);
const FAKE_AGENT = resolve(
  process.env.PLENIPO_FAKE_AGENT ?? join(root, "target", "release", exeName("plenipo-fake-agent")),
);
const PORT = 4444;

/** A fresh, isolated HOME so each run has its own app data. */
export function makeHome() {
  return mkdtempSync(join(tmpdir(), "plenipo-e2e-"));
}

/**
 * Put the fake AI tools (plenipo-fake-agent) in `<home>/bin`: one copy for each AI tool it
 * stands in for (`plenipo-fake-agent --personas`), and for each other program (`--helpers`:
 * GitHub's `gh` and a project test, `verify`). Returns the variables for `launch` that put them
 * first on PATH.
 */
export function installFakeTools(home) {
  const bin = join(home, "bin");
  mkdirSync(bin, { recursive: true });
  const names = (flag) =>
    execFileSync(FAKE_AGENT, [flag], { encoding: "utf8" }).split(/\r?\n/).filter(Boolean);
  const personas = [...names("--personas"), ...names("--helpers")];
  for (const name of personas) {
    copyFileSync(FAKE_AGENT, join(bin, exeName(name)));
    chmodSync(join(bin, exeName(name)), 0o755);
  }
  return { PATH: `${bin}${delimiter}${process.env.PATH ?? ""}` };
}

async function waitForPort(port, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const open = await new Promise((done) => {
      const socket = net.connect(port, "127.0.0.1", () => {
        socket.end();
        done(true);
      });
      socket.on("error", () => done(false));
    });
    if (open) return;
    await sleep(100);
  }
  throw new Error(`tauri-driver did not listen on ${port}`);
}

async function waitForPortFree(port, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const inUse = await new Promise((done) => {
      const socket = net.connect(port, "127.0.0.1", () => {
        socket.end();
        done(true);
      });
      socket.on("error", () => done(false));
    });
    if (!inUse) return;
    await sleep(100);
  }
  throw new Error(`port ${port} is still in use`);
}

/** Start tauri-driver + the app with `home` as its data root. `extraEnv` overrides variables
 * (e.g. a PATH that puts fake agent CLIs first). */
export async function launch(home, extraEnv = {}) {
  const env = {
    ...process.env,
    ...extraEnv,
    HOME: home,
    XDG_DATA_HOME: join(home, ".local", "share"),
    XDG_CONFIG_HOME: join(home, ".config"),
    XDG_CACHE_HOME: join(home, ".cache"),
    // Tauri only lets WebDriver attach to its webview when this is set (test harness only).
    TAURI_WEBVIEW_AUTOMATION: "true",
  };
  await waitForPortFree(PORT);
  await waitForPortFree(PORT + 1); // tauri-driver's native WebDriver
  // Own process group so close() can stop tauri-driver, its native driver, and the app together.
  const driver = spawn("tauri-driver", ["--port", String(PORT)], {
    env,
    stdio: ["ignore", "inherit", "inherit"],
    detached: process.platform !== "win32",
  });
  await waitForPort(PORT);
  // tauri-driver starts the native WebDriver alongside it; a session request that arrives
  // before that one listens is refused.
  await waitForPort(PORT + 1);
  const browser = await remote({
    hostname: "127.0.0.1",
    port: PORT,
    logLevel: "error",
    connectionRetryCount: 0,
    capabilities: {
      browserName: "wry",
      "tauri:options": { application: APP },
      // WebKitWebDriver speaks classic WebDriver only (no BiDi).
      "wdio:enforceWebDriverClassic": true,
    },
  });
  return {
    browser,
    async close() {
      try {
        await browser.deleteSession();
      } catch {
        // The app may already have exited.
      }
      const exited = new Promise((done) =>
        driver.exitCode === null && driver.signalCode === null ? driver.once("exit", done) : done(),
      );
      try {
        if (process.platform === "win32") driver.kill();
        else process.kill(-driver.pid, "SIGTERM"); // whole group
      } catch {
        // Already gone.
      }
      await exited;
      await waitForPortFree(PORT);
      await waitForPortFree(PORT + 1);
      stopLeftoverBrowser(home);
    },
  };
}

/**
 * Plenipo closes its browser when it quits, but close() can stop the app before that finishes.
 * The browser runs in its own process group, so it would outlive the app, stay on the test
 * display, and keep the keyboard focus from the next suite's window (which then draws no focus
 * outlines). Stop any browser still running on this test's profile.
 */
function stopLeftoverBrowser(home) {
  if (process.platform === "win32") return; // there, the app's children die with it (Job Object)
  const profile = join(home, ".local", "share", "com.eightwest.plenipo", "browser-profile");
  const out = execFileSync("ps", ["-eo", "pid=,args="], { encoding: "utf8" });
  for (const line of out.split("\n")) {
    const [pid, ...args] = line.trim().split(/\s+/);
    if (!pid || !args.join(" ").includes(profile)) continue;
    try {
      process.kill(Number(pid), "SIGKILL");
    } catch {
      // Already gone.
    }
  }
}

/** PIDs of running Plenipo app processes (excluding diagnostic children). */
export function appPids() {
  const out = execFileSync("ps", ["-eo", "pid=,args="], { encoding: "utf8" });
  return out
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.includes(APP) && !l.includes("--plenipo-diagnostic"))
    .map((l) => Number(l.split(/\s+/, 1)[0]));
}

export function pidAlive(pid) {
  try {
    process.kill(pid, 0);
  } catch {
    return false;
  }
  try {
    // A zombie has exited; it is only waiting to be reaped.
    const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
    return stat.slice(stat.lastIndexOf(")") + 2, stat.lastIndexOf(")") + 3) !== "Z";
  } catch {
    return true;
  }
}

export async function waitUntil(predicate, what, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await predicate();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`Timed out waiting for ${what}`);
    await sleep(100);
  }
}

export const waitPidGone = (pid) => waitUntil(() => !pidAlive(pid), `process ${pid} to exit`);

// ---- UI helpers -------------------------------------------------------------

export async function textOf(browser, selector) {
  const el = await browser.$(selector);
  return (await el.isExisting()) ? await el.getText() : "";
}

export const waitForText = (browser, selector, needle, timeoutMs) =>
  waitUntil(
    async () => (await textOf(browser, selector)).includes(needle),
    `"${needle}" in ${selector}`,
    timeoutMs,
  );

export async function nav(browser, label) {
  await (await browser.$(`//nav//button[.//span[normalize-space()="${label}"]]`)).click();
}

/** The app has drawn its frame: the left strip, with Plenipo's logo at its top. */
export const waitForShell = (browser, timeoutMs) =>
  waitUntil(
    () =>
      browser.execute(
        () =>
          document.querySelector(
            'nav[aria-label="Main"] .ui-rail__brand [aria-label="Plenipo"]',
          ) !== null,
      ),
    "Plenipo's frame",
    timeoutMs,
  );

/**
 * Open a section of Settings (Phase 12: one section at a time, from the list on the left), e.g.
 * "Servers", "Permissions", "AI models", "Switches", "Personalization".
 */
export async function openSettings(browser, section) {
  await nav(browser, "Settings");
  const tab = await browser.$(
    `//div[@role="tablist" and @aria-label="Settings sections"]//button[@role="tab"][normalize-space()="${section}"]`,
  );
  await tab.waitForClickable({ timeout: 10_000 });
  await tab.click();
  await waitUntil(
    async () => (await tab.getAttribute("aria-selected")) === "true",
    `Settings → ${section}`,
  );
}

export async function clickButton(browser, label) {
  const button = await browser.$(
    `//button[normalize-space()="${label}" or @aria-label="${label}"]`,
  );
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
}

/** Open a tab of the details panel (Phase 17): "Overview", "Job", "AI model", "Work", "Team",
 * or "Manage". */
export async function detailsTab(browser, name) {
  const tab = await browser.$(
    `//div[@role="tablist" and @aria-label="Details"]//button[@role="tab" and normalize-space()="${name}"]`,
  );
  await tab.waitForClickable({ timeout: 10_000 });
  await tab.click();
}

/** Save a screenshot when PLENIPO_E2E_SCREENSHOTS names a directory (CI evidence). */
export async function screenshot(browser, name) {
  const dir = process.env.PLENIPO_E2E_SCREENSHOTS;
  if (!dir) return;
  mkdirSync(dir, { recursive: true });
  // Pictures (Pip) are loaded and drawn first, so a screenshot never shows an empty space
  // (the test display draws without a graphics card, and a large picture takes a moment).
  try {
    await browser.executeAsync((done) => {
      Promise.all([...document.images].map((img) => img.decode().catch(() => undefined))).then(() =>
        requestAnimationFrame(() => requestAnimationFrame(() => done(true))),
      );
    });
  } catch {
    // A picture that never loads is the page's problem to show, not the screenshot's.
  }
  await browser.saveScreenshot(join(dir, `${name}.png`));
}

/** Whether a picture has loaded and has something to show (a broken one has no width). */
export const pictureShown = (browser, selector) =>
  browser.execute((s) => {
    const img = document.querySelector(s);
    return img instanceof HTMLImageElement && img.complete && img.naturalWidth > 0;
  }, selector);

export const LOG = '[role="log"]';
export const DETAIL = ".detail__header";

export async function selectedPid(browser) {
  const text = await waitUntil(async () => {
    const t = await textOf(browser, DETAIL);
    return /PID \d+/.test(t) ? t : null;
  }, "PID in execution detail");
  return Number(/PID (\d+)/.exec(text)[1]);
}
