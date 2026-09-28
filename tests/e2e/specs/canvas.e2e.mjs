// v1.11 end-to-end (Phase 18): the organization canvas in the real app, with the stand-in AI
// tools. Tiles placed by hand and kept after a restart, then Tidy up with Undo (ADR-053); a
// Security Auditor lent to another department for one objective, which comes home by itself
// (ADR-054); a "reports to" line rewired by its end; the trash can with Undo, and the Archived
// drawer; filters and the legend; the first-time tour; your picture, status, mood, and message
// (ADR-056); and Watch: a worker's file change being written, then saved with its new lines, and
// a change Guard refuses, while the live view says where the work is (ADR-055).

import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
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

// The Website project's folder.
const folder = join(home, "website");
mkdirSync(join(folder, "src", "pages"), { recursive: true });
writeFileSync(join(folder, "README.md"), "# Website\nThe company website.\n");

const PICTURE = resolve(
  import.meta.dirname,
  "..",
  "..",
  "..",
  "apps",
  "desktop",
  "src-tauri",
  "icons",
  "128x128.png",
);

const DETAILS = "aside.inspector";
const MAP = '[aria-label="Organization topology"]';
const TOASTS = ".toasts";

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
  await save(browser, `canvas-${name}`);
};

const nodeXPath = (label) => `//button[@data-node-id and starts-with(@aria-label, "${label}")]`;

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

/** Where a tile is on the canvas (its saved spot, in the canvas's own coordinates). */
const spotOf = (browser, label) =>
  browser.execute((l) => {
    const b = [...document.querySelectorAll("button[data-node-id]")].find((x) =>
      (x.getAttribute("aria-label") ?? "").startsWith(l),
    );
    return b ? { left: b.style.left, top: b.style.top } : null;
  }, label);

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

async function fit(browser) {
  await clickButton(browser, "Fit to screen");
  await settle(browser);
}

async function closeDetails(browser) {
  if (await exists(browser, DETAILS)) await clickButton(browser, "Close details");
}

async function select(browser, title) {
  await fit(browser);
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

async function hire(browser, lead, role) {
  await select(browser, lead);
  await detailsTab(browser, "Manage");
  await clickButton(browser, "Hire into team");
  await (await field(browser, "Hire", "Role", "select")).selectByVisibleText(role);
  await submit(browser, 'form[aria-label="Hire"]');
  await waitForNode(browser, `${role},`);
}

/** Drag with the mouse (pointer events, as a person would) from one element to another. */
async function dragTo(browser, source, target, { alt = false } = {}) {
  const action = browser.action("pointer", { parameters: { pointerType: "mouse" } });
  action
    .move({ origin: source })
    .down({ button: 0 })
    .move({ origin: "pointer", x: 24, y: 24, duration: 150 });
  if (typeof target === "object" && "x" in target && !("elementId" in target)) {
    action.move({ x: Math.round(target.x), y: Math.round(target.y), duration: 400 });
  } else {
    action.move({ origin: target, duration: 400 });
  }
  await action.pause(150).up({ button: 0 }).perform();
  if (alt) await browser.keys("Alt");
}

async function dragNode(browser, from, to) {
  await fit(browser);
  const source = await browser.$(nodeXPath(`${from},`));
  const target = await browser.$(nodeXPath(`${to},`));
  await dragTo(browser, source, target);
}

async function menuItem(browser, start) {
  const item = await browser.$(
    `//button[@role="menuitem" and starts-with(normalize-space(), "${start}")]`,
  );
  await item.waitForClickable({ timeout: 10_000 });
  await item.click();
}

/** A Plenipo tool call in the fake agent's objective. */
const tool = (name, args) => `<<tool:${name} ${JSON.stringify(args)}>>`;

async function start() {
  const app = await launch(home, env);
  await app.browser.setWindowSize(1600, 1000);
  await waitForShell(app.browser);
  return app;
}

describe("v1.11 The organization canvas (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    app = await start();
  });
  after(async () => {
    await app?.close();
  });

  it("builds two departments and shows the first-time tour", async () => {
    const { browser } = app;
    await nav(browser, "Organization");
    await clickButton(browser, "Create a department");
    await (await field(browser, "New department", "Name")).setValue("Development");
    await submit(browser, 'form[aria-label="New department"]');
    await waitForNode(browser, "Development Manager, Idle");
    await clickButton(browser, "+ Project");
    await (await field(browser, "New project", "Name")).setValue("Website");
    await (await field(browser, "New project", "Project folder (optional)")).setValue(folder);
    await submit(browser, 'form[aria-label="New project"]');
    await waitForNode(browser, "Website Supervisor, Idle");
    await hire(browser, "Website Supervisor", "Senior Developer");
    await hire(browser, "Website Supervisor", "Code Reviewer");
    await hire(browser, "Development Manager", "Security Auditor");

    // A second department, from the toolbar's Add menu.
    await closeDetails(browser);
    await clickButton(browser, "Add");
    await menuItem(browser, "A department");
    await (await field(browser, "New department", "Name")).setValue("Marketing");
    await submit(browser, 'form[aria-label="New department"]');
    await waitForNode(browser, "Marketing Manager, Idle");
    await clickButton(browser, "Add");
    await menuItem(browser, "A project");
    await (await field(browser, "New project", "Name")).setValue("Campaign");
    await (
      await field(browser, "New project", "Department", "select")
    ).selectByVisibleText("Marketing");
    await submit(browser, 'form[aria-label="New project"]');
    await waitForNode(browser, "Campaign Supervisor, Idle");

    // The first-time tour: six steps, which can be skipped.
    await browser.execute(() => localStorage.removeItem("plenipo.canvasTour"));
    await nav(browser, "Home");
    await nav(browser, "Organization");
    await waitForText(browser, ".canvas-tour", "Step 1 of 6");
    await fit(browser);
    await screenshot(browser, "tour");
    for (let i = 2; i <= 3; i++) {
      await clickButton(browser, "Next");
      await waitForText(browser, ".canvas-tour", `Step ${i} of 6`);
    }
    await waitForText(browser, ".canvas-tour", "Move or lend an agent");
    await clickButton(browser, "Skip the tour");
    await waitUntil(async () => !(await exists(browser, ".canvas-tour")), "the tour to close");
    assert.equal(await browser.execute(() => localStorage.getItem("plenipo.canvasTour")), "seen");
  });

  it("places a tile by hand, keeps it after a restart, and Tidy up puts it back (with Undo)", async () => {
    let { browser } = app;
    await closeDetails(browser);
    await fit(browser);
    await clickButton(browser, "Zoom out");
    await settle(browser);
    const before = await spotOf(browser, "Security Auditor");
    // An empty spot below every tile.
    const spot = await browser.execute(() => {
      const map = document
        .querySelector('[aria-label="Organization topology"]')
        .getBoundingClientRect();
      const tiles = [...document.querySelectorAll("button[data-node-id]")].map((b) =>
        b.getBoundingClientRect(),
      );
      const bottom = Math.max(...tiles.map((r) => r.bottom));
      const auditor = [...document.querySelectorAll("button[data-node-id]")]
        .find((b) => (b.getAttribute("aria-label") ?? "").startsWith("Security Auditor"))
        .getBoundingClientRect();
      return {
        x: auditor.left + auditor.width / 2,
        y: Math.min(map.bottom - 60, bottom + 90),
      };
    });
    const source = await browser.$(nodeXPath("Security Auditor,"));
    await dragTo(browser, source, spot);
    await waitUntil(
      async () => (await spotOf(browser, "Security Auditor"))?.top !== before?.top,
      "the tile to stay where it was dropped",
    );
    const placed = await spotOf(browser, "Security Auditor");
    await fit(browser);
    await screenshot(browser, "placed");

    // After a restart, it is still there.
    await app.close();
    app = await start();
    browser = app.browser;
    await nav(browser, "Organization");
    await waitForNode(browser, "Security Auditor,");
    assert.deepEqual(await spotOf(browser, "Security Auditor"), placed);

    // Tidy up puts every tile back in rows, and Undo brings the spot back.
    await clickButton(browser, "Tidy up");
    await waitUntil(
      async () => (await spotOf(browser, "Security Auditor"))?.top === before?.top,
      "the automatic layout",
    );
    await waitForText(browser, TOASTS, "Tidied up");
    await screenshot(browser, "tidy-up");
    await clickButton(browser, "Undo");
    await waitUntil(
      async () => (await spotOf(browser, "Security Auditor"))?.top === placed?.top,
      "Undo to bring the spot back",
    );
  });

  it("lends the Security Auditor to Marketing for one objective, and it comes home by itself", async () => {
    const { browser } = app;
    await closeDetails(browser);
    await dragNode(browser, "Security Auditor", "Campaign Supervisor");
    await menuItem(browser, "Lend for one objective");
    await waitForText(
      browser,
      TOASTS,
      "Security Auditor is lent to Campaign Supervisor's team for one objective.",
    );
    const badge = '[data-symbol="badge-lent"]';
    await waitForText(browser, badge, "Lent → Campaign Supervisor");
    assert.ok(await exists(browser, ".topo-chip--lent"), "the lent line's label");
    await fit(browser);
    await screenshot(browser, "lent");

    // The Campaign Supervisor hands it work; when that objective is done, it goes home.
    await select(browser, "Campaign Supervisor");
    await (
      await objectiveBox(browser)
    ).setValue("Check the campaign pages [handoff:role:Security Auditor+delay:3000]");
    await clickButton(browser, "Give objective");
    await select(browser, "Security Auditor");
    await detailsTab(browser, "Team");
    await waitForText(browser, DETAILS, "Lent to another team");
    await waitForText(browser, DETAILS, "Campaign Supervisor");
    await screenshot(browser, "lent-team-tab");
    await waitUntil(
      async () => !(await exists(browser, badge)),
      "the auditor to come home when the objective is done",
      90_000,
    );
    await waitForText(browser, `${DETAILS}`, "Development Manager");
  });

  it("rewires a line by its end, and archives with the trash can (Undo, and the drawer)", async () => {
    const { browser } = app;
    await select(browser, "Code Reviewer");
    const handle = await browser.$(
      '//button[starts-with(@aria-label, "Line end: Code Reviewer reports to Website Supervisor")]',
    );
    await handle.waitForExist({ timeout: 10_000 });
    await screenshot(browser, "line-ends");
    const target = await browser.$(nodeXPath("Campaign Supervisor,"));
    await dragTo(browser, handle, target);
    await waitForText(browser, TOASTS, "Code Reviewer now reports to Campaign Supervisor.");

    // The trash can: archived at once, with Undo.
    await closeDetails(browser);
    await fit(browser);
    const reviewer = await browser.$(nodeXPath("Code Reviewer,"));
    const trash = await browser.$('button[data-drop="trash"]');
    await dragTo(browser, reviewer, trash);
    await waitForText(browser, TOASTS, "Archived Code Reviewer.");
    await screenshot(browser, "trash-undo");
    await clickButton(browser, "Undo");
    await waitForText(browser, TOASTS, "Brought back Code Reviewer.");
    await waitForNode(browser, "Code Reviewer,");

    // Again, then the Archived drawer brings it back.
    await fit(browser);
    await dragTo(browser, await browser.$(nodeXPath("Code Reviewer,")), trash);
    await waitForText(browser, TOASTS, "Archived Code Reviewer.");
    await trash.click();
    await waitForText(browser, ".canvas-panel--archived", "Code Reviewer");
    await screenshot(browser, "archived-drawer");
    await clickButton(browser, "Bring back Code Reviewer");
    await waitForNode(browser, "Code Reviewer,");
    await clickButton(browser, "Close archived");
  });

  it("filters narrow the canvas, and the legend explains every mark", async () => {
    const { browser } = app;
    await closeDetails(browser);
    await clickButton(browser, "Filters");
    const department = await browser.$(
      '//section[contains(@class, "canvas-panel--filters")]//label[normalize-space()="Department"]/following::select[1]',
    );
    await department.selectByVisibleText("Marketing");
    await waitForText(browser, ".canvas-panel--filters", "Showing 3 of");
    assert.ok(!(await nodes(browser)).some((l) => l.startsWith("Senior Developer,")));
    await fit(browser);
    await screenshot(browser, "filters");
    await clickButton(browser, "Clear filters");
    await waitForNode(browser, "Senior Developer,");
    await clickButton(browser, "Close filters");

    await clickButton(browser, "Legend");
    await waitForText(browser, ".canvas-panel--legend", "Reports to");
    await waitForText(browser, ".canvas-panel--legend", "Thinks in an AI company's cloud");
    await screenshot(browser, "legend");
    await clickButton(browser, "Legend");
  });

  it("your tile: your picture, status, mood, and message", async () => {
    const { browser } = app;
    await (await browser.$("#owner-button")).click();
    const panel = '[role="dialog"]';
    await waitForText(browser, panel, "Your picture");
    const file = await browser.$(`${panel} input[type="file"]`);
    await browser.execute((el) => el.removeAttribute("hidden"), file);
    await file.setValue(PICTURE);
    await waitUntil(() => exists(browser, `${panel} img[alt="Your picture"]`), "the picture");
    await (await browser.$(`//label[.//span[normalize-space()="Busy"]]//input`)).click();
    await (await browser.$(`//label[.//span[normalize-space()="Great"]]//input`)).click();
    await (await browser.$(`${panel} input[type="text"]`)).setValue("Feeling great!");
    await screenshot(browser, "owner-panel");
    await clickButton(browser, "Save");
    await waitUntil(async () => !(await exists(browser, panel)), "the panel to close");
    await waitForNode(browser, "You, President: Busy, feeling Great, “Feeling great!”");
    await fit(browser);
    await screenshot(browser, "owner-tile");
  });

  it("watch a worker write code: being written, saved with its new lines, and refused", async () => {
    const { browser } = app;
    const page = Array.from({ length: 24 }, (_, i) => `<p>Line ${i + 1} of the home page</p>`);
    const work = [
      "[stream-writes:1200]",
      tool("write_file", {
        path: "src/pages/home.html",
        content: `<main>\n${page.join("\n")}\n</main>\n`,
      }),
      tool("write_file", { path: ".env", content: "API_KEY=never-shown\n" }),
      "[delay:4000]",
    ].join(" ");
    await select(browser, "Website Supervisor");
    await (
      await objectiveBox(browser)
    ).setValue(`Build the home page {{handoff:role:Senior Developer|${work}}}`);
    await clickButton(browser, "Give objective");
    await closeDetails(browser);

    // The live view: where the work is, and the hand-off moving along the line.
    await waitForNode(browser, "Senior Developer, Working", 60_000);
    await clickButton(browser, "Where");
    await waitForText(browser, MAP, "Thinks in Anthropic's cloud", 30_000);
    await fit(browser);
    await screenshot(browser, "live-where");

    // Watch, from the Watch button beside the working tile.
    await clickButton(browser, "Watch Senior Developer write code");
    const tab = ".code-watch";
    await waitForText(browser, tab, "being written — not saved yet", 30_000);
    await screenshot(browser, "watch-writing");
    await waitForText(browser, tab, "Line 24 of the home page", 60_000);
    await waitUntil(
      async () =>
        /saved/i.test(await textOf(browser, tab)) &&
        (await exists(browser, `${tab} [data-mark="new"], ${tab} .code-watch__line--new`)),
      "the saved change with its new lines",
      60_000,
    );
    await screenshot(browser, "watch-saved");
    await waitForText(browser, tab, "refused", 60_000);
    assert.ok(
      !(await textOf(browser, tab)).includes("never-shown"),
      "a refused change never shows its text",
    );
    await screenshot(browser, "watch-refused");
    assert.match(readFileSync(join(folder, "src", "pages", "home.html"), "utf8"), /Line 24/);
    assert.ok(!existsSync(join(folder, ".env")), "Guard's refusal wrote nothing");
    // Read-only: nothing in the tab can be typed into.
    assert.equal(
      await browser.execute(
        (s) =>
          document.querySelector(s)?.querySelectorAll("input, textarea, [contenteditable]")
            .length ?? -1,
        tab,
      ),
      0,
    );
  });
});
