// v1.10 end-to-end (Phase 17): the owner's control over workers, in the real app with the
// stand-in AI tools. Model and effort rules in layers (ADR-041): the organization's effort, then
// one agent's own, which never hires a new agent; a Database developer (a specialty, ADR-042);
// learning off for one agent; the details panel's six tabs, each option with its line, widened;
// archive, bring back, and delete for good, saving an agent to the Workforce and hiring it again
// (ADR-043, ADR-045); and a whole department archived and brought back.

import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  detailsTab,
  installFakeTools,
  launch,
  makeHome,
  nav,
  openSettings,
  pointedAt,
  screenshot as save,
  settle,
  steady,
  waitForShell,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();
const env = installFakeTools(home);
const agentDir = join(home, ".plenipo-fake-agent");
mkdirSync(agentDir, { recursive: true });
writeFileSync(join(agentDir, "auth"), "subscription");

const DETAILS = "aside.inspector";
const MAP = '[aria-label="Organization topology"]';

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
  await save(browser, `control-${name}`);
};

const nodes = (browser) =>
  browser.execute(() =>
    [...document.querySelectorAll("button[data-node-id]")].map(
      (b) => b.getAttribute("aria-label") ?? "",
    ),
  );

const waitForNode = (browser, label, timeoutMs = 20_000) =>
  waitUntil(
    async () => (await nodes(browser)).some((l) => l.startsWith(label)),
    `node "${label}"`,
    timeoutMs,
  );

const field = (browser, form, label, tag = "input") =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//${tag}`);

/** A control in the details panel, by the label beside it. */
const panelControl = (browser, label, tag = "select") =>
  browser.$(
    `//aside[contains(@class, "inspector")]//${tag}[@id=//label[normalize-space()="${label}"]/@for]`,
  );

async function submit(browser, form) {
  const button = await browser.$(`${form} button[type="submit"]`);
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
  await waitUntil(async () => !(await exists(browser, form)), `${form} to close`);
}

async function closeDetails(browser) {
  if (await exists(browser, DETAILS)) await clickButton(browser, "Close details");
}

/**
 * Select a tile on the canvas (or a row of the list). On the canvas, the camera is still and the
 * tile uncovered first: a click on a moving canvas can land beside the tile, which only clears
 * the selection.
 */
async function select(browser, title) {
  const onCanvas = await exists(browser, MAP);
  if (onCanvas) {
    await clickButton(browser, "Fit to screen");
    await settle(browser);
  }
  const node = await browser.$(`//button[@data-node-id and starts-with(@aria-label, "${title},")]`);
  await node.waitForExist({ timeout: 10_000 });
  if (onCanvas) await steady(browser, node, `${title}'s tile`);
  const clicked = await pointedAt(browser, node);
  try {
    await node.click();
  } catch {
    await browser.execute((el) => el.click(), node);
  }
  await waitForText(browser, `${DETAILS} h2`, title).catch(async (error) => {
    const shown = await textOf(browser, `${DETAILS} h2`);
    console.error(
      `--- the details show "${shown}"; the click at ${clicked.at} was on ${clicked.hit}`,
    );
    await save(browser, `control-failed-select-${title.replace(/\W+/g, "-")}`);
    throw error;
  });
}

/** Answer a "Delete …?" or "Archive …?" box with its button. */
async function confirm(browser, label) {
  const button = await browser.$(`//div[@role="dialog"]//button[normalize-space()="${label}"]`);
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
  await waitUntil(async () => !(await exists(browser, '[role="dialog"]')), "the box to close");
}

/** A tab of the List view ("Archived (1)", "Workforce (1)"), by the start of its name. */
async function listTab(browser, name) {
  const tab = await browser.$(
    `//div[@role="tablist" and @aria-label="Directory"]//button[@role="tab" and starts-with(normalize-space(), "${name}")]`,
  );
  await tab.waitForClickable({ timeout: 10_000 });
  await tab.click();
}

async function archiveSelected(browser) {
  await detailsTab(browser, "Manage");
  await clickButton(browser, "Archive");
  await confirm(browser, "Archive position");
}

describe("v1.10 The owner's control over workers (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    app = await launch(home, env);
    await app.browser.setWindowSize(1600, 1000);
  });
  after(async () => {
    await app?.close();
  });

  it("hires a Senior Developer with the Database specialty", async () => {
    const { browser } = app;
    await waitForShell(browser);
    await nav(browser, "Organization");
    await clickButton(browser, "Create a department");
    await (await field(browser, "New department", "Name")).setValue("Development");
    await submit(browser, 'form[aria-label="New department"]');
    await clickButton(browser, "+ Project");
    await (await field(browser, "New project", "Name")).setValue("Website");
    await submit(browser, 'form[aria-label="New project"]');
    await waitForNode(browser, "Website Supervisor, Idle");
    for (const [role, specialty, title] of [
      ["Senior Developer", "Database", "Database Developer"],
      ["Code Reviewer", null, "Code Reviewer"],
    ]) {
      await select(browser, "Website Supervisor");
      await detailsTab(browser, "Manage");
      await clickButton(browser, "Hire into team");
      await (await field(browser, "Hire", "Role", "select")).selectByVisibleText(role);
      if (specialty) {
        await (await field(browser, "Hire", "Specialty", "select")).selectByVisibleText(specialty);
        // The specialty suggests its title.
        assert.equal(await (await field(browser, "Hire", "Title")).getValue(), title);
      }
      await submit(browser, 'form[aria-label="Hire"]');
      await waitForNode(browser, `${title},`);
    }
    await select(browser, "Database Developer");
    await detailsTab(browser, "Overview");
    await waitForText(browser, DETAILS, "Senior Developer (Database)");
  });

  it("rules in layers: the organization's effort, then one agent's own (no new agent)", async () => {
    const { browser } = app;
    await openSettings(browser, "AI models");
    await waitUntil(() => exists(browser, "#who-uses-what-title"), "Who uses what");
    await clickButton(browser, "Change the rule for The whole organization");
    const form = "Rule for The whole organization";
    await (
      await browser.$(
        `//form[@aria-label="${form}"]//label[.//span[normalize-space()="Effort for any other model"]]//select`,
      )
    ).selectByVisibleText("High effort");
    await (
      await browser.$(`//form[@aria-label="${form}"]//button[normalize-space()="Save rule"]`)
    ).click();
    await waitForText(browser, ".models__rules", "High effort");
    await browser.execute(() =>
      document.querySelector("#who-uses-what-title")?.scrollIntoView({ block: "start" }),
    );
    await screenshot(browser, "rules-settings");

    await nav(browser, "Organization");
    await select(browser, "Database Developer");
    await detailsTab(browser, "AI model");
    await waitForText(browser, DETAILS, "It runs at high effort, from the organization's rule.");
    await (await panelControl(browser, "Effort")).selectByVisibleText("Max effort");
    await waitForText(
      browser,
      DETAILS,
      "It runs at max effort, from this agent's own setting.",
      30_000,
    );
    await screenshot(browser, "panel-ai-model");
  });

  it("learning off for one agent, and the panel's other tabs, widened", async () => {
    const { browser } = app;
    await detailsTab(browser, "Job");
    await waitForText(browser, DETAILS, "What Database adds to its instructions");
    await (await panelControl(browser, "This agent learns")).selectByVisibleText("Never learns");
    await waitForText(browser, DETAILS, "It does not learn: its own setting.", 30_000);
    await screenshot(browser, "panel-job");
    await detailsTab(browser, "Work");
    await waitForText(browser, DETAILS, "Its permissions");
    await screenshot(browser, "panel-work");
    await detailsTab(browser, "Team");
    await waitForText(browser, DETAILS, "Move to report to");
    await screenshot(browser, "panel-team");
    await detailsTab(browser, "Manage");
    // Widen the panel from its edge (the arrow keys move it too), and it stays wider.
    const edge = await browser.$('[role="separator"][aria-label="Widen or narrow the details"]');
    await browser.execute((el) => el.focus(), edge);
    for (let i = 0; i < 12; i++) await browser.keys("ArrowLeft");
    await waitUntil(
      async () => Number(await edge.getAttribute("aria-valuenow")) >= 540,
      "the panel to widen",
    );
    await screenshot(browser, "panel-manage-wide");
    await detailsTab(browser, "Overview");
    await screenshot(browser, "panel-overview");
  });

  it("archive, bring back, and delete for good: saved to the Workforce and hired again", async () => {
    const { browser } = app;
    await archiveSelected(browser);
    await waitUntil(
      async () => !(await nodes(browser)).some((l) => l.startsWith("Database Developer,")),
      "the agent to leave the chart",
    );
    await clickButton(browser, "List");
    await listTab(browser, "Archived (1)");
    await waitForText(browser, '[aria-label="Archived agents"]', "Database Developer");
    await screenshot(browser, "archived-list");
    await clickButton(browser, "Bring back Database Developer");
    await waitForText(browser, ".toasts", "Brought back Database Developer.");
    await clickButton(browser, "Topology");
    await waitForNode(browser, "Database Developer,");

    // Archive it again, then delete it for good, saving it to the Workforce.
    await select(browser, "Database Developer");
    await archiveSelected(browser);
    await clickButton(browser, "List");
    await listTab(browser, "Archived (1)");
    await clickButton(browser, "Delete Database Developer for good");
    const box = '[role="dialog"]';
    await waitForText(browser, `${box} h2`, "Delete Database Developer for good?");
    await waitUntil(() => exists(browser, `${box} input[type="checkbox"]`), "the agents that go");
    await waitForText(browser, box, "This cannot be undone");
    const keep = await browser.$(`${box} input[type="checkbox"]`);
    if (!(await keep.isSelected())) await keep.click();
    await waitForText(
      browser,
      box,
      "Will save 1 agent to your Workforce and delete 0 agents for good.",
    );
    await screenshot(browser, "delete-for-good");
    await confirm(browser, "Delete for good");
    await waitForText(browser, ".toasts", "saved to your Workforce");

    await listTab(browser, "Workforce (1)");
    await waitForText(browser, '[aria-label="Workforce"]', "Database Developer");
    await waitForText(browser, '[aria-label="Workforce"]', "Senior Developer (Database)");
    await screenshot(browser, "workforce");
    await clickButton(browser, "Hire Database Developer into a team");
    const hire = 'form[aria-label="Hire from my Workforce"]';
    await (
      await browser.$(
        `//form[@aria-label="Hire from my Workforce"]//label[.//span[normalize-space()="Reports to"]]//select`,
      )
    ).selectByVisibleText("Website Supervisor");
    await submit(browser, hire);
    await waitForText(browser, ".toasts", "Hired Database Developer.");
    await clickButton(browser, "Topology");
    await waitForNode(browser, "Database Developer,");
    await select(browser, "Database Developer");
    await detailsTab(browser, "Overview");
    await waitForText(browser, DETAILS, "Senior Developer (Database)");

    // Deleted for good without saving: gone from every list, a short record in the Ledger.
    await closeDetails(browser);
    await select(browser, "Code Reviewer");
    await archiveSelected(browser);
    await clickButton(browser, "List");
    await listTab(browser, "Archived (1)");
    await clickButton(browser, "Delete Code Reviewer for good");
    await waitForText(browser, '[role="dialog"] h2', "Delete Code Reviewer for good?");
    await waitUntil(
      () => exists(browser, '[role="dialog"] input[type="checkbox"]'),
      "the agents that go",
    );
    await confirm(browser, "Delete for good");
    await listTab(browser, "Archived (0)");
    await clickButton(browser, "Topology");
    await nav(browser, "Activity");
    await (await browser.$('//button[@role="tab" and normalize-space()="All events"]')).click();
    const all = 'ol[aria-label="All events"]';
    await waitForText(browser, all, "Deleted for good: Code Reviewer");
    await waitForText(browser, all, "Hired from your Workforce: Database Developer");
    await waitForText(browser, all, "Saved to your Workforce: Database Developer");
  });

  it("archives a whole department and brings it back", async () => {
    const { browser } = app;
    await nav(browser, "Organization");
    await waitUntil(() => exists(browser, MAP), "the organization's map");
    await closeDetails(browser);
    await select(browser, "Development Manager");
    await detailsTab(browser, "Team");
    await clickButton(browser, "Archive department");
    await confirm(browser, "Archive department");
    await clickButton(browser, "List");
    await listTab(browser, "Archived");
    await waitForText(browser, '[aria-label="Archived departments"]', "Development");
    await waitForText(browser, '[aria-label="Archived projects"]', "Website");
    await screenshot(browser, "archived-department");
    await clickButton(browser, "Bring back Development");
    await waitForText(browser, ".toasts", "Brought back Development.");
    await clickButton(browser, "Topology");
    for (const label of ["Development Manager,", "Website Supervisor,", "Database Developer,"]) {
      await waitForNode(browser, label);
    }
    await closeDetails(browser);
  });
});
