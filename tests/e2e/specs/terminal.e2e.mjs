// Phase 12 end-to-end: the owner's terminal (ADR-031, the terminal panel), in the real app. The
// panel opens and hides from the top bar and with Ctrl+`, resizes from the keyboard, moves to
// the right and back, and comes back the way the owner left it after a restart. A terminal on
// this PC runs what the owner types (the test display's shell, not PowerShell). A terminal on a
// synthetic SSH server (plenipo-test-sshd --shell on; no internet) opens only while Remote
// computers (SSH) is on and the server's ID is pinned, signs in with the owner's SSH agent, and
// runs what the owner types; a server that shows another ID is refused before Plenipo signs in.
// Real PowerShell, ConPTY, and real servers are checked by the owner on Windows.

import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  launch,
  makeHome,
  nav,
  openSettings,
  screenshot as save,
  waitForShell,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();

// ---- The synthetic SSH server, with a small shell for the owner ---------------------------------

const root = resolve(import.meta.dirname, "../../..");
const exe = (stem) => (process.platform === "win32" ? `${stem}.exe` : stem);
const SSHD = resolve(
  process.env.PLENIPO_TEST_SSHD ?? join(root, "target", "release", exe("plenipo-test-sshd")),
);
const AGENT_SOCK = join(home, "agent.sock");
const KEY = join(home, "id_e2e");
execFileSync("ssh-keygen", ["-q", "-t", "ed25519", "-N", "", "-C", "plenipo-e2e", "-f", KEY]);
const agent = spawn("ssh-agent", ["-D", "-a", AGENT_SOCK], { stdio: "ignore" });

function startSshd(seed, port = 0) {
  const child = spawn(
    SSHD,
    [
      "--seed",
      String(seed),
      "--port",
      String(port),
      "--user",
      "shop",
      "--authorized",
      `${KEY}.pub`,
      "--shell",
      "on",
    ],
    { stdio: ["ignore", "pipe", "inherit"] },
  );
  return new Promise((done, fail) => {
    child.once("error", fail);
    createInterface({ input: child.stdout }).once("line", (line) =>
      done({ child, ...JSON.parse(line) }),
    );
  });
}

let sshd;

// ---- Helpers -----------------------------------------------------------------------------------

const PANEL = 'section[aria-label="Terminal"]';
const HANDLE = `${PANEL} [role="separator"][aria-label="Resize the terminal panel"]`;

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

const panelOpen = (browser) =>
  browser.execute((s) => {
    const panel = document.querySelector(s);
    return panel !== null && !panel.hidden;
  }, PANEL);

const field = (browser, form, label, tag = "input") =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//${tag}`);

const screenshot = async (browser, name) => {
  await browser.pause(800);
  await save(browser, name);
};

/** The visible terminal's text (xterm draws rows as text). */
const screenText = (browser) =>
  textOf(browser, `${PANEL} .terminal-panel__tab:not([hidden]) .xterm-rows`);

async function type(browser, line) {
  await waitUntil(
    () =>
      browser.execute(
        (s) => document.activeElement?.closest(s) !== null,
        `${PANEL} .terminal-panel__tab:not([hidden])`,
      ),
    "the keyboard in the terminal",
  );
  await browser.keys([...line, "Enter"]);
}

async function newTerminal(browser, item) {
  await clickButton(browser, "New terminal");
  const entry = await browser.$(
    `//div[@role="menu"]//button[@role="menuitem"][contains(., "${item}")]`,
  );
  await entry.waitForClickable({ timeout: 10_000 });
  await entry.click();
}

// ---- Tests -------------------------------------------------------------------------------------

describe("Phase 12 terminal panel (real app, synthetic SSH server)", () => {
  let app;

  before(async () => {
    await waitUntil(() => {
      try {
        execFileSync("ssh-add", ["-q", KEY], {
          env: { ...process.env, SSH_AUTH_SOCK: AGENT_SOCK },
        });
        return true;
      } catch {
        return false;
      }
    }, "the SSH agent to take the key");
    sshd = await startSshd(3);
    app = await launch(home, { SSH_AUTH_SOCK: AGENT_SOCK });
    await app.browser.setWindowSize(1440, 960);
  });
  after(async () => {
    await app?.close();
    sshd?.child.kill();
    agent.kill();
  });

  it("opens from the top bar, runs what you type on this PC, resizes, moves, and hides with Ctrl+`", async () => {
    const { browser } = app;
    await waitForShell(browser);
    assert.equal(await panelOpen(browser), false, "hidden to start with");
    await clickButton(browser, "Terminal");
    await waitUntil(() => panelOpen(browser), "the panel to open");
    // Nothing open yet: Pip at the laptop, and a way to start.
    assert.ok(await exists(browser, `${PANEL} [data-pip="coding"]`));
    await screenshot(browser, "terminal-empty");

    await clickButton(browser, "Open a terminal on this PC");
    await waitForText(browser, `${PANEL} [role="tablist"]`, "This PC");
    await waitUntil(() => exists(browser, `${PANEL} .xterm-rows`), "the terminal to draw");
    await type(browser, "echo plenipo-$((6 * 7))");
    await waitUntil(
      async () => (await screenText(browser)).includes("plenipo-42"),
      "the shell's answer",
      20_000,
    );

    // Resize from the keyboard: the edge is a separator the arrow keys move.
    const before = Number(await (await browser.$(HANDLE)).getAttribute("aria-valuenow"));
    await browser.execute((s) => document.querySelector(s)?.focus(), HANDLE);
    await browser.keys(["ArrowUp", "ArrowUp", "ArrowUp"]);
    await waitUntil(
      async () => Number(await (await browser.$(HANDLE)).getAttribute("aria-valuenow")) > before,
      "the panel to grow",
    );
    await screenshot(browser, "terminal-this-pc");

    await clickButton(browser, "Move the terminal to the right");
    await waitUntil(() => exists(browser, ".terminal-panel--right"), "the panel on the right");
    await screenshot(browser, "terminal-right");
    await clickButton(browser, "Move the terminal to the bottom");
    await waitUntil(() => exists(browser, ".terminal-panel--bottom"), "the panel at the bottom");

    // Ctrl+` hides it, and shows it again; the terminal is still there.
    await browser.execute(() => document.querySelector("main")?.focus());
    await browser.keys(["Control", "`"]);
    await waitUntil(async () => !(await panelOpen(browser)), "Ctrl+` to hide the panel");
    await browser.keys(["Control", "`"]);
    await waitUntil(() => panelOpen(browser), "Ctrl+` to show the panel");
    assert.ok((await screenText(browser)).includes("plenipo-42"));

    // The Activity trail records that a terminal opened, never what was typed.
    await nav(browser, "Activity");
    await clickButton(browser, "Events");
    await waitForText(browser, "main", "You opened a terminal on this PC");
    assert.ok(!(await textOf(browser, "main")).includes("plenipo-"), "nothing typed is recorded");
  });

  it("comes back after a restart the way you left it; the terminals themselves end with Plenipo", async () => {
    let { browser } = app;
    const size = Number(await (await browser.$(HANDLE)).getAttribute("aria-valuenow"));
    // The panel's place is kept in the page's storage; give the webview time to write it.
    await waitUntil(
      () =>
        browser.execute(() => JSON.parse(localStorage.getItem("plenipo.terminal") ?? "{}").open),
      "the panel's place saved",
    );
    await browser.pause(1500);
    await app.close();
    app = await launch(home, { SSH_AUTH_SOCK: AGENT_SOCK });
    browser = app.browser;
    await browser.setWindowSize(1440, 960);
    await waitForShell(browser);
    await waitUntil(() => panelOpen(browser), "the panel open again");
    assert.equal(Number(await (await browser.$(HANDLE)).getAttribute("aria-valuenow")), size);
    assert.ok(await exists(browser, `${PANEL} [data-pip="coding"]`), "no terminal left open");
    await nav(browser, "Activity");
    await clickButton(browser, "Events");
    await waitForText(browser, "main", "Plenipo closed");
  });

  it("opens a terminal on a server whose ID is pinned, and refuses one whose ID changed", async () => {
    const { browser } = app;
    await openSettings(browser, "Servers");
    await clickButton(browser, "Add a server");
    const form = "Add a server";
    await (await field(browser, form, "Name")).setValue("Shop");
    await (await field(browser, form, "Address")).setValue("127.0.0.1");
    const port = await field(browser, form, "Port");
    await port.clearValue();
    await port.setValue(String(sshd.port));
    await (await field(browser, form, "Sign in as")).setValue("shop");
    await (await browser.$(`//form[@aria-label="${form}"]//input[@value="production"]`)).click();
    await (
      await browser.$(`//form[@aria-label="${form}"]//select[@aria-label="How Plenipo signs in"]`)
    ).selectByAttribute("value", "agent");
    await clickButton(browser, "Check the server ID");
    await waitForText(browser, '[aria-label="The server ID"]', sshd.fingerprint, 30_000);
    await clickButton(browser, "This is my server: pin this ID");
    const save = await browser.$(`form[aria-label="${form}"] button[type="submit"]`);
    await save.click();
    await waitUntil(
      () => exists(browser, 'li[aria-label="Shop, production server"]'),
      "the Shop server",
    );

    // Remote computers (SSH) is off to start with: the server is listed, but cannot be opened.
    await clickButton(browser, "New terminal");
    await waitForText(browser, '[role="menu"]', "Shop (Remote computers (SSH) is off)");
    const off = await browser.$('//div[@role="menu"]//button[contains(., "Shop")]');
    assert.equal(await off.isEnabled(), false);
    await browser.keys("Escape");
    await openSettings(browser, "Switches");
    const ssh = 'button[role="switch"][aria-label="Remote computers (SSH)"]';
    await (await browser.$(ssh)).click();
    await waitUntil(
      async () => (await (await browser.$(ssh)).getAttribute("aria-checked")) === "true",
      "Remote computers (SSH) on",
    );

    await newTerminal(browser, "Shop");
    await waitForText(browser, `${PANEL} [role="tablist"]`, "Shop");
    await waitForText(browser, `${PANEL} [role="tablist"]`, "PRODUCTION");
    await waitUntil(
      async () => (await screenText(browser)).includes("$"),
      "the server's prompt",
      30_000,
    );
    await type(browser, "whoami");
    await waitUntil(
      async () => /whoami\s+shop/.test(await screenText(browser)),
      "the server to answer",
      20_000,
    );
    await screenshot(browser, "terminal-server");

    // Another computer answers at the same address: refused before Plenipo signs in.
    const at = sshd.port;
    sshd.child.kill();
    await new Promise((done) => sshd.child.once("exit", done));
    sshd = await startSshd(8, at);
    await newTerminal(browser, "Shop");
    await waitForText(browser, PANEL, "This server's ID changed", 30_000);
    await screenshot(browser, "terminal-id-changed");
  });
});
