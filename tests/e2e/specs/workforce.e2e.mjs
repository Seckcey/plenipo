// Phase 5 end-to-end: the organization, in the real app, built and run from the Organization
// canvas against fake `claude` and `codex` CLIs (plenipo-fake-agent). The owner creates the
// Development department and a project, hires the project supervisor's team, assigns oversight
// (by dragging one position onto another, and from the details panel), gives the supervisor an
// objective, and watches its workers appear under it and leave when their tasks finish. A marker
// such as `[handoff:role:Senior Developer+delay:6000]` in the objective makes the fake
// supervisor hand that position a task whose worker takes six seconds. Finally the owner picks
// Army ranks in Settings, and they survive a restart. Real CLIs are verified by the owner (see
// the Phase 5 checklist).

import assert from "node:assert/strict";
import { chmodSync, copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { after, before, describe, it } from "node:test";

import { clickButton, launch, makeHome, nav, screenshot, waitUntil } from "../lib/app.mjs";

const root = resolve(import.meta.dirname, "../../..");
const exe = (name) => (process.platform === "win32" ? `${name}.exe` : name);
const FAKE = resolve(
  process.env.PLENIPO_FAKE_AGENT ?? join(root, "target", "release", exe("plenipo-fake-agent")),
);

const home = makeHome();
const bin = join(home, "bin");
mkdirSync(bin, { recursive: true });
for (const name of ["claude", "codex"]) {
  copyFileSync(FAKE, join(bin, exe(name)));
  chmodSync(join(bin, exe(name)), 0o755);
}
const env = { PATH: `${bin}${delimiter}${process.env.PATH ?? ""}` };
mkdirSync(join(home, ".plenipo-fake-agent"), { recursive: true });
writeFileSync(join(home, ".plenipo-fake-agent", "auth"), "subscription");

const MAP = '[aria-label="Organization topology"]';
const DETAILS = "aside.inspector";
const OBJECTIVE =
  "Ship the pricing page [handoff:role:Senior Developer+delay:6000]" +
  " [handoff:role:Code Reviewer+delay:6000]";

/**
 * The rendered text of the first element matching `selector` (in-page `innerText`: WebKit's
 * WebDriver reports no text for some clipped elements, such as an ellipsized heading).
 */
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

/** The canvas node whose label starts with `label` (a position's title, or "Worker for …"). */
const nodeXPath = (label) => `//button[@data-node-id and starts-with(@aria-label, "${label}")]`;

/** Labels of every node on the canvas, in one in-page read. */
function nodes(browser) {
  return browser.execute(() =>
    [...document.querySelectorAll("button[data-node-id]")].map((b) => b.getAttribute("aria-label")),
  );
}

/** Link chips on the canvas (department, project, a worker's AI tool). */
function chips(browser) {
  return browser.execute(() =>
    [...document.querySelectorAll(".topo-chip")].map((c) => c.textContent),
  );
}

/** The KPI strip, lower-cased (its labels are upper-cased by CSS). */
async function glance(browser) {
  return (await textOf(browser, '[aria-label="At a glance"]')).toLowerCase().replace(/\s+/g, " ");
}

const waitForNode = (browser, label, timeoutMs = 20_000) =>
  waitUntil(
    async () => (await nodes(browser)).find((l) => l.startsWith(label)) ?? null,
    `node "${label}"`,
    timeoutMs,
  );

/** Start the app at a size where the map reads well in screenshots (default: 1200×780). */
async function start() {
  const app = await launch(home, env);
  await app.browser.setWindowSize(1600, 1000);
  return app;
}

/** Close the details panel, so the map has the whole canvas. */
async function closeDetails(browser) {
  if (await exists(browser, "aside.inspector")) await clickButton(browser, "Close details");
}

const zoomLevel = (browser) =>
  browser.execute(() => document.querySelector('[aria-label="Zoom level"]')?.textContent ?? "");

/** Wait until the camera stops moving (frames are sparse in a virtual display). */
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

/** Fit the whole organization in view. */
async function fit(browser) {
  await clickButton(browser, "Fit to screen");
  await settle(browser);
  if (process.env.PLENIPO_E2E_DEBUG) console.log("fit →", await zoomLevel(browser));
}

/** Select a node on the canvas and wait for its details. */
async function select(browser, title) {
  await fit(browser);
  const node = await browser.$(nodeXPath(`${title},`));
  await node.waitForExist({ timeout: 10_000 });
  try {
    await node.click();
  } catch {
    // Covered by canvas chrome (the minimap, say): activate it as the keyboard would.
    await browser.execute((el) => el.click(), node);
  }
  await waitForText(browser, `${DETAILS} h2`, title);
  await settle(browser); // the selection is brought out from under the panel
}

/** The text field labelled `label` in the form labelled `form`. */
const field = (browser, form, label) =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//input`);

async function fill(browser, form, label, value) {
  const input = await field(browser, form, label);
  await input.waitForExist({ timeout: 10_000 });
  await input.setValue(value);
}

/** Whether `selector` matches anything (checked in the page: WebKit's WebDriver can report a
 * node that React is replacing as stale rather than missing). */
function exists(browser, selector) {
  return browser.execute((s) => document.querySelector(s) !== null, selector);
}

async function submit(browser, form) {
  const button = await browser.$(`${form} button[type="submit"]`);
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
  await waitUntil(async () => !(await exists(browser, form)), `${form} to close`);
}

/** The chosen option of the select labelled `label` in the form labelled `form`. */
function chosen(browser, form, label) {
  return browser.execute(
    (f, l) => {
      const labels = [...document.querySelectorAll(`form[aria-label="${f}"] label`)];
      const select = labels
        .find((x) => x.querySelector("span")?.textContent === l)
        ?.querySelector("select");
      return select?.selectedOptions[0]?.textContent ?? "";
    },
    form,
    label,
  );
}

/** Hire `role` into the team of the selected lead (the palette's keyboard path). */
async function hire(browser, lead, role) {
  await select(browser, lead);
  await clickButton(browser, `Hire ${role}`);
  await waitUntil(
    async () => (await chosen(browser, "Hire", "Reports to")).startsWith(lead),
    `the hire form to report to ${lead}`,
  );
  await submit(browser, 'form[aria-label="Hire"]');
  await waitForNode(browser, `${role},`);
}

/** Drag one canvas node onto another with the mouse (pointer events, as a person would). */
async function dragNode(browser, from, to) {
  await fit(browser);
  const source = await browser.$(nodeXPath(`${from},`));
  const target = await browser.$(nodeXPath(`${to},`));
  await browser
    .action("pointer", { parameters: { pointerType: "mouse" } })
    .move({ origin: source })
    .down({ button: 0 })
    .move({ origin: "pointer", x: 24, y: 24, duration: 150 })
    .move({ origin: target, duration: 400 })
    .pause(150)
    .up({ button: 0 })
    .perform();
}

describe("Phase 5 organization (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    app = await start();
  });
  after(async () => {
    await app?.close();
  });

  it("starts empty, then builds Development, its Website project, and the team", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    // The Organization view comes first, and nothing is hard-coded.
    await waitForText(browser, MAP, "Build your organization");
    assert.deepEqual(await nodes(browser), ["You, President", "Organization, organization"]);
    await screenshot(browser, "org-empty");

    await clickButton(browser, "Rename");
    const rename = 'form[aria-label="Rename organization"]';
    await (await browser.$(`${rename} input`)).setValue("8 West Ventures");
    await submit(browser, rename);
    await waitForText(browser, "#org-title", "8 West Ventures");

    // A department comes with its manager.
    await clickButton(browser, "Create a department");
    await fill(browser, "New department", "Name", "Development");
    await waitUntil(
      async () =>
        (await (await field(browser, "New department", "Title")).getValue()) ===
        "Development Manager",
      "the head's title to follow the name",
    );
    await submit(browser, 'form[aria-label="New department"]');
    await waitForNode(browser, "Development Manager, Idle");

    // A project in Development comes with its supervisor, who reports to the department's manager.
    await clickButton(browser, "+ Project");
    await fill(browser, "New project", "Name", "Website");
    await submit(browser, 'form[aria-label="New project"]');
    await waitForNode(browser, "Website Supervisor, Idle");
    // The links into the manager and the supervisor are labelled with the department and project.
    await waitUntil(async () => {
      const labels = await chips(browser);
      return labels.includes("Development") && labels.includes("Website");
    }, "department and project chips");

    // The supervisor's team, and two specialists who report to the department's manager.
    await hire(browser, "Website Supervisor", "Senior Developer");
    await hire(browser, "Website Supervisor", "Code Reviewer");
    await hire(browser, "Development Manager", "QA Engineer");
    await hire(browser, "Development Manager", "Security Auditor");
    assert.match(await glance(browser), /departments 1 projects 1 positions 6/);
  });

  it("assigns a security auditor by dragging and a QA evaluator from the details panel", async () => {
    const { browser } = app;
    await dragNode(browser, "Security Auditor", "Website Supervisor");
    const choice = await browser.$(
      '//button[@role="menuitem" and starts-with(normalize-space(), "Security auditor for Website Supervisor")]',
    );
    await choice.waitForClickable({ timeout: 10_000 });
    await choice.click();
    await waitForText(browser, ".toasts", "is now Website Supervisor's security auditor");

    await select(browser, "QA Engineer");
    const assign = 'form[aria-label="Assign oversight"]';
    await (await browser.$(`${assign} select`)).selectByVisibleText("QA evaluator");
    await (
      await browser.$("(//form[@aria-label='Assign oversight']//select)[2]")
    ).selectByVisibleText("Website Supervisor");
    await (await browser.$(`${assign} button[type="submit"]`)).click();
    await waitForText(browser, DETAILS, "QA evaluator for Website Supervisor's team");

    // Both show on the canvas as oversight links and badges.
    await fit(browser);
    const map = await textOf(browser, MAP);
    for (const part of ["Security → Website Supervisor", "QA → Website Supervisor"]) {
      assert.ok(map.includes(part), `${part} in the map`);
    }
    await closeDetails(browser);
    await fit(browser);
    await screenshot(browser, "org-team");
  });

  it("acceptance: a supervisor's objective puts workers under it, and they leave when done", async () => {
    const { browser } = app;
    await select(browser, "Website Supervisor");
    const form = 'form[aria-label="Give an objective"]';
    await (await browser.$(`${form} textarea`)).setValue(OBJECTIVE);
    await clickButton(browser, "Give objective");

    // One worker under each position it handed work to — queued for a moment, then working —
    // while the supervisor waits on them.
    const working = (label) =>
      waitUntil(
        async () =>
          (await nodes(browser)).some((l) => l.startsWith(label) && l.endsWith(", Working")),
        `"${label}" working`,
        30_000,
      );
    await working("Worker for Senior Developer: Review the answer above");
    await working("Worker for Code Reviewer: Review the answer above");
    await waitForNode(browser, "Website Supervisor, Waiting on team");
    assert.match(await glance(browser), /live workers 2/);
    assert.ok((await chips(browser)).includes("Claude Code"), "the workers' AI tool chips");
    await closeDetails(browser);
    await fit(browser);
    await screenshot(browser, "org-workers-live");

    // They finish and leave the active workforce; the supervisor continues and finishes.
    await waitUntil(
      async () => !(await nodes(browser)).some((l) => l.startsWith("Worker for")),
      "the workers to leave",
      45_000,
    );
    await waitForNode(browser, "Website Supervisor, Idle", 30_000);
    assert.match(await glance(browser), /live workers 0/);

    // History remains: the position remembers its former worker, and the work it did.
    await select(browser, "Senior Developer");
    await waitForText(browser, DETAILS, "1 retired");
    // The tab reads "Recent (1)" once the work has loaded.
    const recent = await browser.$(
      '//button[@role="tab" and starts-with(normalize-space(), "Recent")]',
    );
    await recent.waitForClickable({ timeout: 10_000 });
    await recent.click();
    await waitForText(browser, DETAILS, "Review the answer above");
    await screenshot(browser, "org-after-workers");
  });

  it("the Ledger keeps the organization's trail", async () => {
    const { browser } = app;
    await nav(browser, "Activity");
    const task = await browser.$('//button[starts-with(@aria-label, "Ship the pricing page")]');
    await task.waitForExist({ timeout: 10_000 });
    await task.click();
    const trail = '[aria-label="Activity trail"]';
    await waitForText(browser, trail, "Continued with 2 handoff replies");
    const tree = '[aria-label="Delegation tree"]';
    await waitForText(browser, tree, "Review the answer above");

    // A worker's own trail: brought in for its position, then retired with its task.
    await (
      await browser.$(
        '(//ol[@aria-label="Delegation tree"]//button[starts-with(normalize-space(), "Review the answer above")])[1]',
      )
    ).click();
    await waitForText(browser, trail, "Worker finished and left the organization");
    const child = await textOf(browser, trail);
    assert.match(child, /Worker brought in for (Senior Developer|Code Reviewer)/);
    await screenshot(browser, "org-worker-trail");
  });

  it("the organization, its supervisor, and the chosen titles survive a restart", async () => {
    // The owner calls the ranks by the Army's names instead.
    await nav(app.browser, "Settings");
    const titles = await app.browser.$('//label[.//span[normalize-space()="Titles"]]//select');
    await titles.waitForExist({ timeout: 10_000 });
    await titles.selectByVisibleText("U.S. Army");
    await waitForText(app.browser, '.titles [role="status"]', "U.S. Army titles");
    await nav(app.browser, "Organization");

    await app.close();
    app = await start();
    const { browser } = app;
    await waitForText(browser, "#org-title", "8 West Ventures");
    // Ranks follow the choice; job titles stay as written.
    await waitForNode(browser, "You, General");
    await closeDetails(browser);
    await fit(browser);
    const map = await textOf(browser, MAP);
    // The manager, the supervisor, and the team members.
    for (const rank of ["Captain", "Sergeant", "Private"]) {
      assert.ok(map.includes(rank), `${rank} in the map`);
    }
    await screenshot(browser, "org-army-titles");
    for (const label of [
      "Development Manager, Idle",
      "Website Supervisor, Idle",
      "Senior Developer, Idle",
      "Code Reviewer, Idle",
      "QA Engineer, Idle",
      "Security Auditor, Idle",
    ]) {
      await waitForNode(browser, label);
    }
    // The supervisor keeps its agent and conversation.
    await select(browser, "Website Supervisor");
    await waitForText(browser, DETAILS, "Continues its conversation.");
    await clickButton(browser, "List");
    await waitForText(browser, '[aria-label="Positions"]', "Website Supervisor");
    await screenshot(browser, "org-list-after-restart");
    await clickButton(browser, "Topology");
  });
});
