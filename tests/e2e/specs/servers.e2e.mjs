// Phase 11 end-to-end: servers, in the real app, against fake `claude` and `codex` CLIs
// (plenipo-fake-agent) that call Plenipo's tools over MCP through the real relay, and a
// synthetic SSH server on 127.0.0.1 (plenipo-test-sshd; no internet). The owner adds a
// production server in Settings → Servers — checks and pins its identity, and tests the
// connection — then an Operations Engineer runs commands there: each one waits for the owner's
// approval on a card marked PRODUCTION; the sign on every page shows the worker connected, and
// Disconnect stops it; the Activity trail keeps the connection, the commands, and their output
// with a password hidden; and a server that shows another identity is blocked before
// Plenipo signs in. Real servers and Windows are checked by the owner (Phase 11 checklist).

import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  installFakeTools,
  launch,
  makeHome,
  nav,
  screenshot as save,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();
const env = installFakeTools(home);
mkdirSync(join(home, ".plenipo-fake-agent"), { recursive: true });
writeFileSync(join(home, ".plenipo-fake-agent", "auth"), "subscription");

// ---- The synthetic SSH server ------------------------------------------------------------------

const root = resolve(import.meta.dirname, "../../..");
const exe = (stem) => (process.platform === "win32" ? `${stem}.exe` : stem);
const SSHD = resolve(
  process.env.PLENIPO_TEST_SSHD ?? join(root, "target", "release", exe("plenipo-test-sshd")),
);
const PASSWORD = "e2e-shop-password-4417";

// Plenipo signs in with the owner's SSH agent here: a real `ssh-agent` holding a new key. (On
// Linux the kernel keyring that stands in for Windows Credential Manager belongs to each thread,
// which containers do not always give; keys and passwords in the Vault are covered by the Rust
// tests, and on Windows by the owner's check.)
const AGENT_SOCK = join(home, "agent.sock");
const KEY = join(home, "id_e2e");
execFileSync("ssh-keygen", ["-q", "-t", "ed25519", "-N", "", "-C", "plenipo-e2e", "-f", KEY]);
const agent = spawn("ssh-agent", ["-D", "-a", AGENT_SOCK], { stdio: "ignore" });

/** Start the synthetic server; resolves with its port and identity once it listens. */
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
      "--password",
      PASSWORD,
      "--authorized",
      `${KEY}.pub`,
      "--file",
      `/home/shop/config.txt=DB_PASSWORD=${PASSWORD}`,
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

const DETAILS = "aside.inspector";
const CONTROL = '[aria-label="Browser, desktop, and server work"]';
const SHOP_CARD = 'li[aria-label="Shop, production server"]';

function textOf(browser, selector) {
  return browser.execute(
    (s) => document.querySelector(s)?.innerText.replace(/\s+/g, " ") ?? "",
    selector,
  );
}

/** Wait for `needle` in `selector`; on a timeout, say what it showed instead. */
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

const nodeXPath = (label) => `//button[@data-node-id and starts-with(@aria-label, "${label}")]`;

const waitForNode = (browser, label, timeoutMs = 20_000) =>
  waitUntil(
    async () =>
      (
        await browser.execute(() =>
          [...document.querySelectorAll("button[data-node-id]")].map((b) =>
            b.getAttribute("aria-label"),
          ),
        )
      ).find((l) => l.startsWith(label)) ?? null,
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
  await waitForText(browser, `${DETAILS} h2`, title);
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

const screenshot = async (browser, name) => {
  await browser.pause(800);
  await save(browser, name);
};

/** A Plenipo tool call in the fake agent's objective. */
const tool = (name, args) => `<<tool:${name} ${JSON.stringify(args)}>>`;
const onShop = (program, args = []) => tool("ssh_run", { server: "Shop", program, args });

/** Give the Servers Supervisor an objective that it hands to the Operations Engineer. */
async function delegate(browser, objective, work) {
  await nav(browser, "Organization");
  await select(browser, "Servers Supervisor");
  const form = 'form[aria-label="Give an objective"]';
  await (
    await browser.$(`${form} textarea`)
  ).setValue(`${objective} {{handoff:role:Operations Engineer|${work.join(" ")}}}`);
  await clickButton(browser, "Give objective");
}

/** Open the approval card that says `summary`, check it, and answer. */
async function answer(browser, summary, approve, check) {
  await nav(browser, "Approvals");
  const card = `article[aria-label="Operations Engineer wants to ${summary}"]`;
  await waitUntil(() => exists(browser, card), `the card "${summary}"`, 60_000);
  if (check) await check(card);
  const button = await browser.$(
    `//article[@aria-label="Operations Engineer wants to ${summary}"]//button[normalize-space()="${
      approve ? "Approve" : "Deny"
    }"]`,
  );
  await button.click();
  await waitUntil(async () => !(await exists(browser, card)), "the card to be answered", 30_000);
}

describe("Phase 11 servers: settings, production approvals, the sign, and identity (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    await waitUntil(async () => {
      try {
        execFileSync("ssh-add", ["-q", KEY], {
          env: { ...process.env, SSH_AUTH_SOCK: AGENT_SOCK },
        });
        return true;
      } catch {
        return false;
      }
    }, "the SSH agent to take the key");
    sshd = await startSshd(1);
    app = await launch(home, { ...env, SSH_AUTH_SOCK: AGENT_SOCK });
    await app.browser.setWindowSize(1600, 1000);
  });
  after(async () => {
    await app?.close();
    sshd?.child.kill();
    agent.kill();
  });

  it("Settings → Servers: add a production server, check and pin its identity, and test it", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    await nav(browser, "Settings");
    await waitUntil(() => exists(browser, "#servers-title"), "the Servers section");
    await waitForText(browser, ".servers", "Production servers are marked in red.");
    await clickButton(browser, "Add a server");
    const form = "Add a server";
    await (await field(browser, form, "Name")).setValue("Shop");
    await (await field(browser, form, "Address")).setValue("127.0.0.1");
    const port = await field(browser, form, "Port");
    await port.clearValue();
    await port.setValue(String(sshd.port));
    await (await field(browser, form, "Sign in as")).setValue("shop");
    await (await browser.$(`//form[@aria-label="${form}"]//input[@value="production"]`)).click();
    await waitForText(browser, `form[aria-label="${form}"]`, "Production server.");
    await (
      await browser.$(`//form[@aria-label="${form}"]//select[@aria-label="How Plenipo signs in"]`)
    ).selectByAttribute("value", "agent");
    await waitForText(browser, `form[aria-label="${form}"]`, "never forwarded to the server");
    await clickButton(browser, "Check the server's identity");
    const identity = '[aria-label="The server\'s identity"]';
    await waitForText(browser, identity, sshd.fingerprint, 30_000);
    await clickButton(browser, "This is my server: pin this identity");
    await (
      await browser.$(
        `//form[@aria-label="${form}"]//label[.//span[contains(normalize-space(), "Operations Engineer")]]//input`,
      )
    ).click();
    await browser.execute(() =>
      document.querySelector('form[aria-label="Add a server"]')?.scrollIntoView({ block: "start" }),
    );
    await screenshot(browser, "server-form");
    await submit(browser, `form[aria-label="${form}"]`);
    await waitUntil(() => exists(browser, SHOP_CARD), "the Shop server");
    await waitForText(browser, SHOP_CARD, "PRODUCTION");
    await waitForText(browser, SHOP_CARD, sshd.fingerprint);
    await waitForText(browser, SHOP_CARD, "Before every command (production)");
    await waitForText(
      browser,
      SHOP_CARD,
      "with your SSH agent (it signs in for Plenipo, and is never forwarded",
    );
    await clickButton(browser, "Test the connection");
    await waitForText(browser, SHOP_CARD, "Connected to Shop as shop and signed in", 30_000);
    await browser.execute(() =>
      document.querySelector("#servers-title")?.scrollIntoView({ block: "start" }),
    );
    await screenshot(browser, "servers-settings");
  });

  it("builds an Operations team with an Operations Engineer", async () => {
    const { browser } = app;
    await nav(browser, "Organization");
    await clickButton(browser, "Create a department");
    await (await field(browser, "New department", "Name")).setValue("Operations");
    await submit(browser, 'form[aria-label="New department"]');
    await waitForNode(browser, "Operations Manager, Idle");
    await clickButton(browser, "+ Project");
    await (await field(browser, "New project", "Name")).setValue("Servers");
    await submit(browser, 'form[aria-label="New project"]');
    await waitForNode(browser, "Servers Supervisor, Idle");
    await select(browser, "Servers Supervisor");
    await clickButton(browser, "Hire Operations Engineer");
    await submit(browser, 'form[aria-label="Hire"]');
    await waitForNode(browser, "Operations Engineer,");
    await select(browser, "Operations Engineer");
    await waitForText(browser, DETAILS, "Looks after the servers you set up");
  });

  it("acceptance: every production command waits on a PRODUCTION card, the sign shows the connection, and Disconnect stops it", async () => {
    const { browser } = app;
    await delegate(browser, "Check the shop server", [
      onShop("uptime"),
      onShop("cat", ["config.txt"]),
      onShop("sleep", ["60"]),
    ]);
    await answer(browser, "run uptime on Shop", true, async (card) => {
      const text = await textOf(browser, card);
      assert.match(text, /PRODUCTION/);
      assert.match(text, /Runs: uptime/);
      assert.match(text, /a production server: check exactly what will run/);
      assert.match(text, /every command there waits for your approval/);
      await screenshot(browser, "server-approval-card");
    });
    await answer(browser, "run cat config.txt on Shop", true);
    // The third command waits too; meanwhile the worker is connected, and the sign says so.
    await nav(browser, "Organization");
    await waitForText(browser, ".banner--approval", "is waiting for your approval", 60_000);
    await waitForText(browser, CONTROL, "Operations Engineer is connected to Shop (production)");
    await waitForText(browser, CONTROL, "PRODUCTION");
    await waitForText(browser, ".shell__footer", "Operations Engineer is connected to Shop");
    await screenshot(browser, "server-sign");
    await clickButton(browser, "Disconnect");
    await waitForText(
      browser,
      CONTROL,
      "You disconnected Operations Engineer from Shop (production). It stopped.",
    );
    await waitUntil(
      async () => !(await exists(browser, ".banner--approval")),
      "its waiting request to be refused",
      30_000,
    );
    await screenshot(browser, "server-disconnected");
    await clickButton(browser, "Dismiss");
    await waitUntil(async () => !(await exists(browser, CONTROL)), "the sign to go", 30_000);
  });

  it("the Activity trail keeps the connection, each command, and its output, with the password hidden", async () => {
    const { browser } = app;
    await nav(browser, "Activity");
    const task = await browser.$('//button[starts-with(@aria-label, "Check the shop server")]');
    await task.waitForExist({ timeout: 10_000 });
    await task.click();
    await (await browser.$('(//ol[@aria-label="Delegation tree"]//button)[2]')).click();
    const trail = '[aria-label="Activity trail"]';
    await waitForText(browser, trail, "Operations Engineer connected to Shop (PRODUCTION)", 30_000);
    await waitForText(browser, trail, "Operations Engineer ran on Shop (PRODUCTION): uptime");
    await waitForText(browser, trail, "load average");
    await waitForText(browser, trail, "The command on Shop finished");
    await waitForText(browser, trail, "DB_PASSWORD=[hidden by Plenipo: secret setting]");
    await waitForText(browser, trail, "You disconnected Operations Engineer from its servers");
    assert.ok(!(await textOf(browser, trail)).includes(PASSWORD), "the password is hidden");
    await browser.execute(() =>
      document
        .querySelector('[aria-label="Output from the server"]')
        ?.scrollIntoView({ block: "center" }),
    );
    await screenshot(browser, "server-trail");
  });

  it("a server showing another identity is blocked before Plenipo signs in, and Settings says so", async () => {
    const { browser } = app;
    const port = sshd.port;
    sshd.child.kill();
    await new Promise((done) => sshd.child.once("exit", done));
    // Another computer answers at the same address.
    sshd = await startSshd(9, port);
    await delegate(browser, "Check the shop server again", [onShop("uptime")]);
    await nav(browser, "Settings");
    await waitUntil(() => exists(browser, SHOP_CARD), "the Shop server");
    await waitForText(browser, SHOP_CARD, "This server's identity changed", 60_000);
    await waitForText(browser, SHOP_CARD, sshd.fingerprint);
    await waitForText(browser, SHOP_CARD, "Plenipo did not sign in");
    await browser.execute(() =>
      document
        .querySelector('li[aria-label="Shop, production server"]')
        ?.scrollIntoView({ block: "start" }),
    );
    await screenshot(browser, "server-identity-changed");
    // No approval was even asked: the command was blocked first.
    assert.ok(!(await exists(browser, ".banner--approval")));
    await nav(browser, "Activity");
    const task = await browser.$(
      '//button[starts-with(@aria-label, "Check the shop server again")]',
    );
    await task.waitForExist({ timeout: 10_000 });
    await task.click();
    await (await browser.$('(//ol[@aria-label="Delegation tree"]//button)[2]')).click();
    await waitForText(
      browser,
      '[aria-label="Activity trail"]',
      "Blocked: Shop's identity changed",
      30_000,
    );
  });
});
