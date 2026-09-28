// Phase 19 end-to-end: the AI tools page in the real app (ADR-058, signing in in a terminal tab;
// ADR-059, Plenipo keeps the AI tools up to date between tasks; ADR-060, usage, "plan left", and
// new models), against the fake AI tools (plenipo-fake-agent) and a stand-in for the release
// lists Anthropic, OpenAI, and Ollama publish.
//
// The release-list checks need a copy built with PLENIPO_AI_TOOL_RELEASES pointing at the
// stand-in (CI's E2E job builds one); without it, those checks are skipped. Real AI tools with
// real sign-ins are checked by the owner on Windows (see the Phase 19 acceptance report).

import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  installFakeTools,
  launch,
  makeHome,
  nav,
  openSettings,
  screenshot as save,
  waitForShell,
  waitUntil,
} from "../lib/app.mjs";

const RELEASES = process.env.PLENIPO_AI_TOOL_RELEASES;
const RELEASES_PORT = 8766;
/** What the stand-in serves at `/<host><path>` for each published list (ADR-059 §2). */
const PUBLISHED = {
  "/registry.npmjs.org/@anthropic-ai/claude-code/latest": { version: "2.2.0" },
  "/registry.npmjs.org/@openai/codex/latest": { version: "0.99.0" },
  "/api.github.com/repos/ollama/ollama/releases/latest": { tag_name: "v0.35.0" },
};

const home = makeHome();
const env = installFakeTools(home);
const stateDir = join(home, ".plenipo-fake-agent");
const state = (name) => join(stateDir, name);
const setState = (name, value) => {
  mkdirSync(stateDir, { recursive: true });
  writeFileSync(state(name), value);
};
const readState = (name) => (existsSync(state(name)) ? readFileSync(state(name), "utf8") : null);

const PAGE = 'ul[aria-label="AI tools"]';
const PANEL = 'section[aria-label="Terminal"]';
const card = (label) => `li[aria-label="${label} AI tool"]`;
const NEW_TASK = 'form[aria-label="New task"]';
const ALL_EVENTS = 'ol[aria-label="All events"]';
const AUTO_UPDATE = '//button[@role="switch"][@aria-label="Update AI tools by themselves"]';

const textOf = (browser, selector) =>
  browser.execute((s) => document.querySelector(s)?.innerText.replace(/\s+/g, " ") ?? "", selector);

const waitForText = async (browser, selector, needle, timeoutMs = 30_000) => {
  try {
    return await waitUntil(
      async () => {
        const text = await textOf(browser, selector);
        return typeof needle === "string" ? text.includes(needle) : needle.test(text);
      },
      `"${needle}" in ${selector}`,
      timeoutMs,
    );
  } catch (error) {
    console.error(`--- ${selector} showed ---\n${await textOf(browser, selector)}`);
    throw error;
  }
};

const screenshot = async (browser, name) => {
  await browser.pause(400);
  await save(browser, name);
};

/** A button on one AI tool's card. */
async function cardButton(browser, label, name) {
  const button = await browser.$(
    `//li[@aria-label="${label} AI tool"]//button[normalize-space()="${name}" or @aria-label="${name}"]`,
  );
  await button.waitForClickable({ timeout: 15_000 });
  return button;
}

const pressOnCard = async (browser, label, name) =>
  (await cardButton(browser, label, name)).click();

/** Show one of a card's tabs: Overview, Usage, or Models. */
async function cardTab(browser, label, tab) {
  const button = await browser.$(
    `//li[@aria-label="${label} AI tool"]//button[@role="tab"][normalize-space()="${tab}"]`,
  );
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
  await waitUntil(async () => (await button.getAttribute("aria-selected")) === "true", tab);
}

/** Bring a card into view for a screenshot. */
const showCard = (browser, label) =>
  browser.execute(
    (s) => document.querySelector(s)?.scrollIntoView({ block: "start" }),
    card(label),
  );

async function openAiTools(browser) {
  await nav(browser, "AI tools");
  await waitUntil(
    async () => (await textOf(browser, PAGE)).match(/Ready/g)?.length >= 4,
    "every AI tool ready",
  );
}

async function startTask(browser, runtimeLabel, objective) {
  await nav(browser, "Workers");
  const radio = await browser.$(`//label[.//span[normalize-space()="${runtimeLabel}"]]//input`);
  await radio.waitForExist({ timeout: 10_000 });
  await radio.click();
  await waitUntil(
    async () => (await textOf(browser, NEW_TASK)).includes("Ready"),
    `${runtimeLabel} ready`,
  );
  const box = await browser.$(`${NEW_TASK} textarea`);
  await box.setValue(objective);
  await clickButton(browser, "Start task");
  await waitForText(browser, ".detail__header h2", objective.replace(/\s*\[.*$/, ""));
}

/** The owner's own keys, one at a time, in the terminal tab on show. */
async function type(browser, keys) {
  await waitUntil(
    () =>
      browser.execute(
        (s) => document.activeElement?.closest(s) !== null,
        `${PANEL} .terminal-panel__tab:not([hidden])`,
      ),
    "the keyboard in the terminal",
  );
  for (const key of keys) {
    await browser.keys(key);
    await browser.pause(40);
  }
}

const screenText = (browser) =>
  textOf(browser, `${PANEL} .terminal-panel__tab:not([hidden]) .xterm-rows`);

describe("Phase 19 the AI tools page (real app, fake AI tools)", () => {
  let app;
  let releases;

  before(async () => {
    setState("auth", "subscription");
    releases = createServer((req, res) => {
      const body = PUBLISHED[decodeURIComponent((req.url ?? "/").split("?")[0])];
      if (body)
        res.writeHead(200, { "content-type": "application/json" }).end(JSON.stringify(body));
      else res.writeHead(404).end();
    });
    await new Promise((done) => releases.listen(RELEASES_PORT, "127.0.0.1", done));
    app = await launch(home, env);
  });
  after(async () => {
    await app?.close();
    releases?.close();
    rmSync(home, { recursive: true, force: true });
  });

  it("shows each AI tool's versions, sign-in, and how it is paid for, with the paid key locked", async () => {
    const { browser } = app;
    await waitForShell(browser);
    await openAiTools(browser);
    const codex = await textOf(browser, card("Codex"));
    assert.match(codex, /Installed 0\.99\.0/);
    assert.match(codex, /Subscription/);
    // The paid-key switch is shown, and cannot be turned on before spending caps (Phase 16).
    const paid = await browser.$(
      '//li[@aria-label="Codex AI tool"]//button[@role="switch"][@aria-label="Paid AI key (pay per use)"]',
    );
    await paid.waitForExist({ timeout: 10_000 });
    assert.equal(await paid.isEnabled(), false, "the paid-key switch is locked");
    // Kimi has no sign-out command, and its card says so.
    await waitForText(browser, card("Kimi"), "Kimi has no sign-out command");
    assert.equal(
      await (
        await browser.$('//li[@aria-label="Kimi AI tool"]//button[normalize-space()="Sign out"]')
      ).isExisting(),
      false,
    );
    await showCard(browser, "Claude Code");
    await screenshot(browser, "ai-tools-page");
  });

  it("signs Codex out and back in from its card, in a tab that runs only Codex's own command", async () => {
    const { browser } = app;
    await openAiTools(browser);
    await pressOnCard(browser, "Codex", "Sign out");
    // A tab named for what it does, running `codex logout`, and nothing else.
    await waitForText(browser, PANEL, "Sign out · Codex");
    await waitForText(browser, card("Codex"), "Not signed in");
    assert.deepEqual(JSON.parse(readState("last-args.json")), ["logout"]);

    await pressOnCard(browser, "Codex", "Sign in");
    await waitForText(browser, PANEL, "Sign in · Codex");
    await waitUntil(
      async () => (await screenText(browser)).includes("ABCD-1234"),
      "the sign-in program's code in the tab",
    );
    assert.deepEqual(JSON.parse(readState("last-args.json")), ["login"]);
    // Plenipo typed nothing into the tab: the program has received no key yet.
    assert.equal(readState("sign-in-input"), null, "nothing was typed for the owner");
    await screenshot(browser, "ai-tools-sign-in-tab");

    // The owner finishes the sign-in with their own key.
    await (await browser.$(`${PANEL} .terminal-panel__tab:not([hidden]) .xterm`)).click();
    await type(browser, ["Enter"]);
    await waitUntil(async () => (await screenText(browser)).includes("Signed in."), "signed in");
    // Only the owner's key reached the program.
    assert.match(readState("sign-in-input"), /^[\r\n]+$/);
    // The card checks again by itself when the program ends.
    await nav(browser, "AI tools");
    await waitForText(browser, card("Codex"), "Reconnect");
    await waitForText(browser, card("Codex"), "Signed in");
    await showCard(browser, "Codex");
    await screenshot(browser, "ai-tools-signed-in-again");

    // The Activity trail says what happened, never who signed in.
    await nav(browser, "Activity");
    await clickButton(browser, "All events");
    await waitForText(browser, ALL_EVENTS, "You opened Codex's sign-in");
    await waitForText(browser, ALL_EVENTS, "Codex: signed out");
    await waitForText(browser, ALL_EVENTS, "Codex: signed in");
    assert.doesNotMatch(await textOf(browser, ALL_EVENTS), /ABCD-1234|owner@example\.com/);
  });

  it("shows this week's usage for Claude Code by model, and how much of the plan is left", async () => {
    const { browser } = app;
    await startTask(browser, "Claude Code", "Say hello [plan:9]");
    await waitForText(browser, '[aria-label="Tasks"]', "Turn 1:");
    await openAiTools(browser);
    // Claude Code reports how much of the plan is used in its own task messages.
    await waitForText(browser, card("Claude Code"), "91% of your plan left");
    await waitForText(browser, card("Claude Code"), "Reported by Claude Code");
    // Grok does not report it, and its card says so.
    await waitForText(browser, card("Grok"), "Grok doesn't report how much of your plan is left");
    await showCard(browser, "Claude Code");
    await screenshot(browser, "ai-tools-plan-left");

    await cardTab(browser, "Claude Code", "Usage");
    await waitForText(browser, card("Claude Code"), "This week");
    await waitForText(browser, card("Claude Code"), "Tokens are pieces of words");
    const usage = await textOf(browser, card("Claude Code"));
    assert.match(usage, /Today/);
    assert.match(usage, /Last week/);
    await showCard(browser, "Claude Code");
    await screenshot(browser, "ai-tools-usage");
    await cardTab(browser, "Claude Code", "Overview");
  });

  it("shows a usage limit and its reset time on the card, with Try again now", async () => {
    const { browser } = app;
    await startTask(browser, "Codex", "Try once [usage-limit]");
    await openAiTools(browser);
    await waitForText(browser, card("Codex"), "Usage limit reached");
    await showCard(browser, "Codex");
    await screenshot(browser, "ai-tools-usage-limit");
    await pressOnCard(browser, "Codex", "Try again now");
    await waitForText(browser, card("Codex"), "No usage limit reached");
    // Settings → AI models no longer lists the AI tools; it links to this page.
    await openSettings(browser, "AI models");
    await clickButton(browser, "Open the AI tools page");
    await waitForText(browser, ".ui-topbar__title", "AI tools");
  });

  it("updates Grok with one click while no task uses it, and marks the model that came with it", async () => {
    const { browser } = app;
    // Grok's own check reports a new version; the new version lists one more model.
    setState("newest-grok", "1.1.0");
    setState("models-grok", "grok-5");
    await openAiTools(browser);
    await clickButton(browser, "Check for new versions");
    await waitForText(browser, card("Grok"), "Newest version: 1.1.0");
    if (RELEASES) {
      // The published lists, read through Guard from the stand-in.
      await waitForText(browser, card("Claude Code"), "Newest version: 2.2.0");
      await waitForText(browser, card("Ollama"), "0.35.0");
      await waitForText(browser, card("Codex"), "Up to date");
    }
    await showCard(browser, "Grok");
    await screenshot(browser, "ai-tools-update-ready");

    await pressOnCard(browser, "Grok", "Update to 1.1.0");
    await waitForText(browser, card("Grok"), "Updated to 1.1.0");
    // Grok's own update command, run by Plenipo with nothing typed into it.
    assert.match(readState("update-log") ?? "", /^grok update$/m);
    // Checked again: the new version is newer than the one Plenipo was checked with.
    await waitForText(browser, card("Grok"), "Installed 1.1.0");
    await waitForText(browser, card("Grok"), "newer than the one Plenipo was checked with");
    await showCard(browser, "Grok");
    await screenshot(browser, "ai-tools-updated");

    await cardTab(browser, "Grok", "Models");
    await waitForText(browser, card("Grok"), "grok-5");
    await waitForText(browser, card("Grok"), "new — not checked yet");
    await showCard(browser, "Grok");
    await screenshot(browser, "ai-tools-new-model");
    await cardTab(browser, "Grok", "Overview");

    // The new model can be chosen, marked, in the model menus.
    await openSettings(browser, "AI models");
    await clickButton(browser, "Add a model");
    const form = '//form[@aria-label="Add a model"]';
    await (await browser.$(form)).waitForExist({ timeout: 10_000 });
    await (
      await browser.$(`${form}//label[.//span[normalize-space()="AI tool"]]//select`)
    ).selectByVisibleText("Grok");
    const MENU = `${form}//label[.//span[normalize-space()="Model"]]//select`;
    const options = () =>
      browser.execute((xpath) => {
        const el = document.evaluate(
          xpath,
          document,
          null,
          XPathResult.FIRST_ORDERED_NODE_TYPE,
          null,
        ).singleNodeValue;
        return el ? [...el.options].map((o) => o.textContent) : [];
      }, MENU);
    await waitUntil(
      async () => (await options()).includes("grok-5 — new, not checked yet"),
      "grok-5 offered as new in the menu",
    );
    await (await browser.$(MENU)).selectByAttribute("value", "grok-5");
    assert.equal(await (await browser.$(MENU)).getValue(), "grok-5", "the new model is chosen");
    await screenshot(browser, "ai-tools-new-model-menu");
    await (await browser.$(`${form}//button[normalize-space()="Cancel"]`)).click();
  });

  it("an update waits while a task uses Grok, and a failed one leaves the old version working", async () => {
    const { browser } = app;
    setState("newest-grok", "1.2.0");
    setState("update-grok", "fail");
    const updates = () => (readState("update-log") ?? "").split("\n").filter(Boolean).length;
    const before = updates();
    await startTask(browser, "Grok", "Take a while [delay:15000]");
    await openAiTools(browser);
    await clickButton(browser, "Check for new versions");
    await pressOnCard(browser, "Grok", "Update to 1.2.0");
    // The update never starts while the task runs; it waits.
    await waitForText(browser, card("Grok"), "Waiting: 1 task is using Grok");
    await showCard(browser, "Grok");
    await screenshot(browser, "ai-tools-update-waiting");
    assert.equal(updates(), before, "Grok's update command has not run");
    // When the task ends, the update runs, fails, and the old version still works.
    await waitForText(browser, card("Grok"), "your old version (1.1.0) still works", 60_000);
    await waitForText(browser, card("Grok"), "Installed 1.1.0");
    await showCard(browser, "Grok");
    await screenshot(browser, "ai-tools-update-failed");
    rmSync(state("update-grok"));
  });

  it("the switch to update AI tools by themselves is off to begin with, here and in Settings", async () => {
    const { browser } = app;
    await openAiTools(browser);
    const toggle = await browser.$(AUTO_UPDATE);
    await toggle.waitForExist({ timeout: 10_000 });
    assert.equal(await toggle.getAttribute("aria-checked"), "false");
    await toggle.click();
    await waitUntil(async () => (await toggle.getAttribute("aria-checked")) === "true", "on");
    await openSettings(browser, "Switches");
    await waitForText(browser, "main", "Update AI tools by themselves");
    const same = await browser.$(AUTO_UPDATE);
    await same.waitForExist({ timeout: 10_000 });
    assert.equal(await same.getAttribute("aria-checked"), "true", "one setting, two places");
    await screenshot(browser, "ai-tools-switch-in-settings");
    await same.click();
    await waitUntil(async () => (await same.getAttribute("aria-checked")) === "false", "off");
  });
});
