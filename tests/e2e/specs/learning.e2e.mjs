// v1.4 end-to-end: Settings → Switches (ADR-021) and workers learning from their work (ADR-022),
// in the real app with the stand-in AI tools. The owner turns a switch off and on again (it
// stays as set); a Supervisor's answer carries a lesson, which waits on the Approvals page; the
// owner keeps it in their own words; the role's details show it, and the role can learn on its
// own.

import assert from "node:assert/strict";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
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
const agentDir = join(home, ".plenipo-fake-agent");
mkdirSync(agentDir, { recursive: true });
writeFileSync(join(agentDir, "auth"), "subscription");

/** The Supervisor's next answer (the stand-in AI tool follows this script). */
function answer(text) {
  rmSync(join(agentDir, "script-used"), { recursive: true, force: true });
  writeFileSync(
    join(agentDir, "script.json"),
    JSON.stringify({ "Shop Supervisor": [{ say: text }] }),
  );
}

const DETAILS = "aside.inspector";

function textOf(browser, selector) {
  return browser.execute(
    (s) => document.querySelector(s)?.innerText.replace(/\s+/g, " ") ?? "",
    selector,
  );
}

const waitForText = (browser, selector, needle, timeoutMs) =>
  waitUntil(
    async () => (await textOf(browser, selector)).includes(needle),
    `"${needle}" in ${selector}`,
    timeoutMs,
  );

const exists = (browser, selector) =>
  browser.execute((s) => document.querySelector(s) !== null, selector);

/** Evidence, once the window has drawn what the test just saw. */
const screenshot = async (browser, name) => {
  await browser.pause(800);
  await save(browser, name);
};

const switchState = (browser, name) =>
  browser.execute(
    (n) => document.querySelector(`button[role="switch"][aria-label="${n}"]`)?.ariaChecked ?? "",
    name,
  );

async function flip(browser, name) {
  const before = await switchState(browser, name);
  const button = await browser.$(`button[role="switch"][aria-label="${name}"]`);
  await button.scrollIntoView({ block: "center" });
  await button.click();
  await waitUntil(
    async () => (await switchState(browser, name)) !== before,
    `the "${name}" switch to change`,
  );
}

const field = (browser, form, label, tag = "input") =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//${tag}`);

async function submit(browser, form) {
  const button = await browser.$(`${form} button[type="submit"]`);
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
  await waitUntil(async () => !(await exists(browser, form)), `${form} to close`);
}

async function select(browser, title) {
  await clickButton(browser, "Fit to screen");
  await browser.pause(800);
  const node = await browser.$(`//button[@data-node-id and starts-with(@aria-label, "${title},")]`);
  await node.waitForExist({ timeout: 10_000 });
  try {
    await node.click();
  } catch {
    await browser.execute((el) => el.click(), node);
  }
  await waitForText(browser, `${DETAILS} h2`, title);
}

describe("v1.4 Switches and learning (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    app = await launch(home, env);
    await app.browser.setWindowSize(1600, 1000);
  });
  after(async () => {
    await app?.close();
  });

  it("Settings → Switches: starting states, and a switch stays as set", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    await nav(browser, "Settings");
    await waitUntil(() => exists(browser, "#switches-features"), "the Switches section");
    assert.equal(await switchState(browser, "Plenipo's browser"), "true");
    assert.equal(await switchState(browser, "Screen, mouse, and keyboard"), "false");
    assert.equal(await switchState(browser, "Sending forms and messages"), "false");
    assert.equal(
      await switchState(browser, "Hand me checks that a person is using a website"),
      "true",
    );
    await waitUntil(
      async () => (await switchState(browser, "Worker learning")) === "true",
      "the learning switch",
    );
    await flip(browser, "Screenshots in the Activity trail");
    await nav(browser, "Organization");
    await nav(browser, "Settings");
    await waitUntil(
      async () => (await switchState(browser, "Screenshots in the Activity trail")) === "false",
      "the switch to stay off",
    );
    await flip(browser, "Screenshots in the Activity trail");
    await browser.execute(() =>
      document.querySelector("#switches-features")?.scrollIntoView({ block: "start" }),
    );
    await screenshot(browser, "switches-settings");
  });

  it("a worker's lesson waits on the Approvals page, and the owner keeps it", async () => {
    const { browser } = app;
    await nav(browser, "Organization");
    await clickButton(browser, "Create a department");
    await (await field(browser, "New department", "Name")).setValue("Operations");
    await submit(browser, 'form[aria-label="New department"]');
    await clickButton(browser, "+ Project");
    await (await field(browser, "New project", "Name")).setValue("Shop");
    await submit(browser, 'form[aria-label="New project"]');
    answer("Planned.\n```plenipo-lesson\n- Check the supplier's price list before ordering.\n```");
    await select(browser, "Shop Supervisor");
    await (
      await browser.$('form[aria-label="Give an objective"] textarea')
    ).setValue("Plan this week's orders.");
    await clickButton(browser, "Give objective");
    // The sidebar counts it, and the Approvals page shows it.
    await waitUntil(
      () => exists(browser, 'nav[aria-label="Main"] [aria-label="1 waiting for you"]'),
      "the lesson in the sidebar count",
      60_000,
    );
    await nav(browser, "Approvals");
    const card = 'article[aria-label="Lesson from Shop Supervisor"]';
    await waitUntil(() => exists(browser, card), "the lesson card");
    await waitForText(browser, card, "Shop Supervisor learned something");
    const box = await browser.$(`${card} textarea`);
    assert.equal(await box.getValue(), "Check the supplier's price list before ordering.");
    await screenshot(browser, "new-lesson");
    await box.setValue("Check the supplier's current price list before ordering.");
    await clickButton(browser, "Keep");
    await waitUntil(async () => !(await exists(browser, card)), "the lesson to be kept");
  });

  it("the role's details show what it learned, and it can learn on its own", async () => {
    const { browser } = app;
    await nav(browser, "Organization");
    await select(browser, "Shop Supervisor");
    await waitForText(browser, DETAILS, "What it has learned");
    await waitForText(browser, DETAILS, "Check the supplier's current price list before ordering.");
    assert.equal(await switchState(browser, "Learn on its own"), "false");
    await flip(browser, "Learn on its own");
    await browser.execute(() =>
      document.querySelector(".role-lessons")?.scrollIntoView({ block: "center" }),
    );
    await screenshot(browser, "role-lessons");
    // The Activity trail records the owner's decisions.
    await nav(browser, "Activity");
    await (await browser.$('//button[@role="tab" and normalize-space()="All events"]')).click();
    const all = 'ol[aria-label="All events"]';
    await waitForText(browser, all, "You kept a lesson");
    await waitForText(browser, all, "now learns on its own");
  });
});
