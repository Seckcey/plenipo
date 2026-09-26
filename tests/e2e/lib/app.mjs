// Launch the real Plenipo binary under tauri-driver and drive it with WebdriverIO.

import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync } from "node:fs";
import net from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

import { remote } from "webdriverio";

const root = resolve(import.meta.dirname, "../../..");
const exe = process.platform === "win32" ? "plenipo-desktop.exe" : "plenipo-desktop";
export const APP = resolve(process.env.PLENIPO_APP ?? join(root, "target", "release", exe));
const PORT = 4444;

/** A fresh, isolated HOME so each run has its own app data. */
export function makeHome() {
  return mkdtempSync(join(tmpdir(), "plenipo-e2e-"));
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

/** Start tauri-driver + the app with `home` as its data root. */
export async function launch(home) {
  const env = {
    ...process.env,
    HOME: home,
    XDG_DATA_HOME: join(home, ".local", "share"),
    XDG_CONFIG_HOME: join(home, ".config"),
    XDG_CACHE_HOME: join(home, ".cache"),
    // Tauri only lets WebDriver attach to its webview when this is set (test harness only).
    TAURI_WEBVIEW_AUTOMATION: "true",
  };
  const driver = spawn("tauri-driver", ["--port", String(PORT)], {
    env,
    stdio: ["ignore", "inherit", "inherit"],
  });
  await waitForPort(PORT);
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
      driver.kill();
      await new Promise((done) => (driver.exitCode === null ? driver.once("exit", done) : done()));
    },
  };
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

export async function clickButton(browser, label) {
  const button = await browser.$(
    `//button[normalize-space()="${label}" or @aria-label="${label}"]`,
  );
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
}

/** Save a screenshot when PLENIPO_E2E_SCREENSHOTS names a directory (CI evidence). */
export async function screenshot(browser, name) {
  const dir = process.env.PLENIPO_E2E_SCREENSHOTS;
  if (!dir) return;
  mkdirSync(dir, { recursive: true });
  await browser.saveScreenshot(join(dir, `${name}.png`));
}

export const LOG = '[role="log"]';
export const DETAIL = ".detail__header";

export async function selectedPid(browser) {
  const text = await waitUntil(async () => {
    const t = await textOf(browser, DETAIL);
    return /PID \d+/.test(t) ? t : null;
  }, "PID in execution detail");
  return Number(/PID (\d+)/.exec(text)[1]);
}
