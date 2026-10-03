// Phase 25, item 2.9 (ADR-198): one full run of the setup tour in the real app, with the stand-in
// AI tools. It starts from Home, passes the AI tools step (the stand-in Claude Code is signed in),
// opens Settings → Organization, waits while the New department and New project dialogs are open,
// moves on by itself once each step is done (a department, a project, a hired worker, an
// objective), lets you skip the models step, and ends with Finish.

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
  objectiveBox,
  screenshot as save,
  waitForShell,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();
const env = installFakeTools(home);
mkdirSync(join(home, ".plenipo-fake-agent"), { recursive: true });
writeFileSync(join(home, ".plenipo-fake-agent", "auth"), "subscription");

const folder = join(home, "website");
mkdirSync(folder, { recursive: true });
writeFileSync(join(folder, "README.md"), "# Website\n");

const POPOVER = ".driver-popover";

const screenshot = async (browser, name) => {
  await browser.pause(600);
  await save(browser, `setup-tour-${name}`);
};

/** The tour's title now, or `null` while it is hidden. */
const titleNow = (browser) =>
  browser.execute(() => document.querySelector(".driver-popover-title")?.textContent ?? null);

const waitForStep = (browser, title, timeoutMs = 20_000) =>
  waitUntil(async () => (await titleNow(browser)) === title, `the tour's "${title}"`, timeoutMs);

/** The `data-tour` mark of the part the tour points at (`null`: the middle of the screen). */
const spotlight = (browser) =>
  browser.execute(
    () => document.querySelector(".driver-active-element")?.getAttribute("data-tour") ?? null,
  );

async function tourButton(browser, which) {
  const button = await browser.$(`${POPOVER} .driver-popover-${which}-btn`);
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
}

const field = (browser, form, label, tag = "input") =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//${tag}`);

async function submit(browser, form) {
  const button = await browser.$(`${form} button[type="submit"]`);
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
  await waitUntil(
    async () => !(await browser.execute((s) => document.querySelector(s) !== null, form)),
    `${form} to close`,
  );
}

const hidden = async (browser) =>
  !(await browser.execute((s) => document.querySelector(s) !== null, POPOVER));

describe("v25 The setup tour (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    app = await launch(home, env);
    await app.browser.setWindowSize(1600, 1000);
    await waitForShell(app.browser);
  });
  after(async () => {
    await app?.close();
  });

  it("walks a new organization from nothing to a team at work", async () => {
    const { browser } = app;
    await nav(browser, "Home");
    await clickButton(browser, "Take the setup tour");
    await waitForStep(browser, "Welcome to Plenipo");
    assert.match(
      await browser.execute(
        () => document.querySelector(".driver-popover-progress-text")?.textContent ?? "",
      ),
      /Step 1 of 9/,
    );
    await screenshot(browser, "welcome");

    // Signed in already: on to naming the organization, in Settings → Organization.
    await tourButton(browser, "next");
    await waitForStep(browser, "Name your organization, or pick a template");
    await waitUntil(
      async () => (await spotlight(browser)) === "organization",
      "the spotlight on Settings → Organization",
    );
    await screenshot(browser, "organization");

    // Add a department: the tour hides while the dialog is open, then moves on by itself.
    await tourButton(browser, "next");
    await waitForStep(browser, "Add a department");
    await waitUntil(
      async () => (await spotlight(browser)) === "add-department",
      "the spotlight on + Department",
    );
    await screenshot(browser, "department");
    // The part in the spotlight is the one to press (the palette's or the empty map's).
    const spot = await browser.$(".driver-active-element");
    await spot.waitForClickable({ timeout: 10_000 });
    await spot.click();
    await (await field(browser, "New department", "Name")).setValue("Development");
    await waitUntil(() => hidden(browser), "the tour to wait while the dialog is open");
    await submit(browser, 'form[aria-label="New department"]');

    // The first project.
    await waitForStep(browser, "Set up your first project");
    await clickButton(browser, "+ Project");
    await (await field(browser, "New project", "Name")).setValue("Website");
    await (await field(browser, "New project", "Project folder (optional)")).setValue(folder);
    await waitUntil(() => hidden(browser), "the tour to wait while the dialog is open");
    await submit(browser, 'form[aria-label="New project"]');

    // Hire the team: the supervisor is brought into view and selected.
    await waitForStep(browser, "Hire the team");
    await waitUntil(
      () =>
        browser.execute(
          () =>
            document.querySelector("aside.inspector h2")?.textContent?.includes("Website") ?? false,
        ),
      "the Website Supervisor selected",
    );
    await screenshot(browser, "team");
    await detailsTab(browser, "Manage");
    await clickButton(browser, "Hire into team");
    await (await field(browser, "Hire", "Role", "select")).selectByVisibleText("Senior Developer");
    await submit(browser, 'form[aria-label="Hire"]');

    // Models: Settings → AI models, Who uses what. Skipped here.
    await waitForStep(browser, "Choose a model for each job");
    await waitUntil(
      async () => (await spotlight(browser)) === "who-uses-what",
      "the spotlight on Who uses what",
    );
    await screenshot(browser, "models");
    await tourButton(browser, "next");

    // The first objective, to the Website Supervisor.
    await waitForStep(browser, "Give the first objective");
    await (await objectiveBox(browser)).setValue("Write a short welcome page");
    await clickButton(browser, "Give objective");

    // Watch it work, then Finish.
    await waitForStep(browser, "Watch it work", 60_000);
    await screenshot(browser, "watch");
    await tourButton(browser, "next");
    await waitUntil(() => hidden(browser), "the tour to end");
    await nav(browser, "Home");
    await (
      await browser.$('//button[normalize-space()="Take the setup tour again"]')
    ).waitForExist({ timeout: 10_000 });
    const saved = await browser.execute(() =>
      Object.values(JSON.parse(localStorage.getItem("plenipo.setupTour") ?? "{}")).map(
        (p) => p.status,
      ),
    );
    assert.deepEqual(saved, ["finished"]);
  });
});
