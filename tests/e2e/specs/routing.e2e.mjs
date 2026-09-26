// Phase 6 end-to-end: model policy and role routing, in the real app, against fake `claude` and
// `codex` CLIs (plenipo-fake-agent). The owner builds a small organization whose positions
// follow their roles' model choices, sets the Senior Developer's choices in Settings → AI
// models, and watches the supervisor's next worker go to that AI tool — then changes the
// choice, and the next worker follows it, without touching the supervisor. Every worker says
// why it got its model, on the map and in the Ledger's trail. A worker that reports a usage
// limit holds that AI tool back until the owner tries it again. Real CLIs are verified by the
// owner (see the Phase 6 checklist).

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

const DETAILS = "aside.inspector";
const ROLES = "table.models__roles";
const DEV_CHOICES = 'form[aria-label="Model choices for Senior Developer"]';

/** In-page text of the first element matching `selector` (whitespace collapsed). */
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

const nodeXPath = (label) => `//button[@data-node-id and starts-with(@aria-label, "${label}")]`;

function nodes(browser) {
  return browser.execute(() =>
    [...document.querySelectorAll("button[data-node-id]")].map((b) => b.getAttribute("aria-label")),
  );
}

/** The text of a worker node's AI tool chip and meta line, by the worker's label. */
function workerText(browser, label) {
  return browser.execute(
    (l) =>
      [...document.querySelectorAll("button[data-node-id]")]
        .find((b) => b.getAttribute("aria-label")?.startsWith(l))
        ?.innerText.replace(/\s+/g, " ") ?? "",
    label,
  );
}

const waitForNode = (browser, label, timeoutMs = 20_000) =>
  waitUntil(
    async () => (await nodes(browser)).find((l) => l.startsWith(label)) ?? null,
    `node "${label}"`,
    timeoutMs,
  );

/** Bring `selector` to the top of its scrolling view (for screenshots). */
function scrollTo(browser, selector) {
  return browser.execute(
    (s) => document.querySelector(s)?.scrollIntoView({ block: "start" }),
    selector,
  );
}

function exists(browser, selector) {
  return browser.execute((s) => document.querySelector(s) !== null, selector);
}

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

async function closeDetails(browser) {
  if (await exists(browser, "aside.inspector")) await clickButton(browser, "Close details");
}

const field = (browser, form, label) =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//input`);

async function submit(browser, form) {
  const button = await browser.$(`${form} button[type="submit"]`);
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
  await waitUntil(async () => !(await exists(browser, form)), `${form} to close`);
}

/** The row of the role-choices table for `role`, as text. */
function roleRow(browser, role) {
  return browser.execute((r) => {
    const row = [...document.querySelectorAll("table.models__roles tbody tr")].find((tr) =>
      tr.querySelector("th")?.textContent?.startsWith(r),
    );
    return row?.innerText.replace(/\s+/g, " ") ?? "";
  }, role);
}

/** The models listed in the open Senior Developer editor, in order. */
function listed(browser) {
  return browser.execute(
    (f) =>
      [...document.querySelectorAll(`${f} .models__order .models__name`)].map((n) => n.textContent),
    DEV_CHOICES,
  );
}

/**
 * Set the Senior Developer's model list to `labels`, in order, in Settings → AI models, with the
 * role's effort for some of them (`efforts`: model label → option text).
 */
async function preferForSeniorDeveloper(browser, labels, efforts = {}) {
  await nav(browser, "Settings");
  await waitForText(browser, ROLES, "Senior Developer");
  await clickButton(browser, "Change Senior Developer's model choices");
  await (await browser.$(DEV_CHOICES)).waitForExist({ timeout: 10_000 });
  // Clear the current list, then add the models in order.
  while ((await listed(browser)).length > 0) {
    const before = (await listed(browser)).length;
    await (await browser.$(`${DEV_CHOICES} button[aria-label^="Take "]`)).click();
    await waitUntil(
      async () => (await listed(browser)).length < before,
      "a model to leave the list",
    );
  }
  for (const [i, label] of labels.entries()) {
    const add = await browser.$(
      `//form[@aria-label="Model choices for Senior Developer"]//label[.//span[normalize-space()="Add a model to the list"]]//select`,
    );
    await add.selectByVisibleText(label);
    await waitUntil(
      async () => (await listed(browser))[i] === label,
      `${label} to be choice ${i + 1}`,
    );
  }
  for (const [label, effort] of Object.entries(efforts)) {
    await (
      await browser.$(`${DEV_CHOICES} select[aria-label="Effort for ${label}"]`)
    ).selectByVisibleText(effort);
  }
  if (Object.keys(efforts).length > 0) {
    await scrollTo(browser, DEV_CHOICES);
    await screenshot(browser, "models-effort");
  }
  await submit(browser, DEV_CHOICES);
  await waitUntil(
    async () => (await roleRow(browser, "Senior Developer")).includes(`${labels[0]} is Senior`),
    "the Senior Developer row to show its new first choice",
  );
}

/** Give the Website Supervisor an objective and wait for its worker on `tool`. */
async function delegate(browser, objective, tool) {
  await nav(browser, "Organization");
  await select(browser, "Website Supervisor");
  const form = 'form[aria-label="Give an objective"]';
  await (await browser.$(`${form} textarea`)).setValue(objective);
  await clickButton(browser, "Give objective");
  const worker = "Worker for Senior Developer";
  await waitUntil(
    async () => (await workerText(browser, worker)).includes(tool),
    `a Senior Developer worker on ${tool}`,
    30_000,
  );
  return worker;
}

describe("Phase 6 model policy and role routing (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    app = await launch(home, env);
    await app.browser.setWindowSize(1600, 1000);
  });
  after(async () => {
    await app?.close();
  });

  it("Settings → AI models lists the AI tools' default models and every role's next worker", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    await nav(browser, "Settings");
    await waitForText(browser, ROLES, "Senior Developer");
    const models = await textOf(browser, '[aria-labelledby="models-title"]');
    for (const m of ["Claude Code (default model)", "Codex (default model)"]) {
      assert.ok(models.includes(m), `${m} listed`);
    }
    await waitForText(browser, '[aria-labelledby="tools-title"]', "Ready: signed in");
    // Starting choices for built-in roles: the Designer needs a model that makes images.
    await waitUntil(
      async () => (await roleRow(browser, "Designer")).includes("None right now"),
      "the Designer to have no model yet",
    );
    assert.match(await roleRow(browser, "Designer"), /not marked as able to see images/);
    await scrollTo(browser, "#role-choices-title");
    await screenshot(browser, "models-settings");
  });

  it("builds a team whose positions follow their roles' model choices", async () => {
    const { browser } = app;
    await nav(browser, "Organization");
    await clickButton(browser, "Create a department");
    await (await field(browser, "New department", "Name")).setValue("Development");
    await submit(browser, 'form[aria-label="New department"]');
    await waitForNode(browser, "Development Manager, Idle");
    await clickButton(browser, "+ Project");
    await (await field(browser, "New project", "Name")).setValue("Website");
    await submit(browser, 'form[aria-label="New project"]');
    await waitForNode(browser, "Website Supervisor, Idle");
    await select(browser, "Website Supervisor");
    await clickButton(browser, "Hire Senior Developer");
    await submit(browser, 'form[aria-label="Hire"]');
    await waitForNode(browser, "Senior Developer,");
    await select(browser, "Senior Developer");
    await waitForText(browser, DETAILS, "Automatic: Senior Developer model choices");
  });

  it("acceptance: a role's model choice in Settings decides its next worker, with the reason", async () => {
    const { browser } = app;
    await preferForSeniorDeveloper(browser, ["Codex (default model)"]);
    await scrollTo(browser, "#role-choices-title");
    await screenshot(browser, "models-role-choices");

    const first = await delegate(
      browser,
      "Build it [handoff:role:Senior Developer+delay:5000]",
      "Codex",
    );
    await select(browser, "Senior Developer");
    await waitForText(
      browser,
      DETAILS,
      "Codex (default model) is Senior Developer's first choice and is ready.",
    );
    await screenshot(browser, "routing-why");
    await closeDetails(browser);
    await fit(browser);
    await screenshot(browser, "routing-worker-codex");
    await waitUntil(
      async () => !(await nodes(browser)).some((l) => l.startsWith(first)),
      "the Codex worker to leave",
      45_000,
    );
    await waitForNode(browser, "Website Supervisor, Idle", 30_000);

    // The owner changes the preference, with the effort Claude Code runs at for this role; the
    // next worker follows it. The supervisor is untouched.
    await preferForSeniorDeveloper(
      browser,
      ["Claude Code (default model)", "Codex (default model)"],
      { "Claude Code (default model)": "High effort" },
    );
    assert.match(
      await roleRow(browser, "Senior Developer"),
      /Claude Code \(default model\) · high effort/,
    );
    const second = await delegate(
      browser,
      "Now the API [handoff:role:Senior Developer+delay:5000]",
      "Claude Code",
    );
    await closeDetails(browser);
    await fit(browser);
    await screenshot(browser, "routing-worker-claude");
    await waitUntil(
      async () => !(await nodes(browser)).some((l) => l.startsWith(second)),
      "the Claude Code worker to leave",
      45_000,
    );
    await waitForNode(browser, "Website Supervisor, Idle", 30_000);
  });

  it("the Ledger records why each worker got its model", async () => {
    const { browser } = app;
    await nav(browser, "Activity");
    const task = await browser.$('//button[starts-with(@aria-label, "Now the API")]');
    await task.waitForExist({ timeout: 10_000 });
    await task.click();
    const tree = '[aria-label="Delegation tree"]';
    await waitForText(browser, tree, "Review the answer above");
    await (
      await browser.$(
        '(//ol[@aria-label="Delegation tree"]//button[starts-with(normalize-space(), "Review the answer above")])[1]',
      )
    ).click();
    const trail = '[aria-label="Activity trail"]';
    await waitForText(
      browser,
      trail,
      "Worker brought in for Senior Developer — Claude Code (default model) is Senior Developer's first choice and is ready. It runs at high effort (Senior Developer's setting for it).",
    );
    await screenshot(browser, "routing-trail");
  });

  it("a usage limit holds that AI tool back until the owner tries it again", async () => {
    const { browser } = app;
    // The supervisor works on Claude Code; the Senior Developer's worker goes to Codex, which
    // reports a usage limit.
    await preferForSeniorDeveloper(browser, [
      "Codex (default model)",
      "Claude Code (default model)",
    ]);
    await nav(browser, "Organization");
    await select(browser, "Website Supervisor");
    const form = 'form[aria-label="Give an objective"]';
    await (
      await browser.$(`${form} textarea`)
    ).setValue("Once more [handoff:role:Senior Developer+usage-limit]");
    await clickButton(browser, "Give objective");
    await waitForNode(browser, "Website Supervisor, Idle", 30_000);

    // Waiting is the default: no move to Claude Code, and Settings says why.
    await nav(browser, "Settings");
    await waitUntil(
      async () => (await roleRow(browser, "Senior Developer")).includes("None right now"),
      "the Senior Developer to wait for Codex",
    );
    assert.match(
      await roleRow(browser, "Senior Developer"),
      /Codex reached its usage limit, and Senior Developer waits for it/,
    );
    await waitForText(browser, '[aria-labelledby="tools-title"]', "Try again now");
    await scrollTo(browser, "#role-choices-title");
    await screenshot(browser, "models-usage-limit");
    await clickButton(browser, "Try again now");
    await waitUntil(
      async () =>
        (await roleRow(browser, "Senior Developer")).includes(
          "Codex (default model) is Senior Developer's first choice",
        ),
      "Codex to be back",
    );
  });
});
