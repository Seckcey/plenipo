// Phase 6 end-to-end: model policy and role routing, in the real app, against fake `claude` and
// `codex` CLIs (plenipo-fake-agent). The owner builds a small organization whose positions
// follow their roles' model choices, sets the Senior Developer's choices in Settings → AI
// models, and watches the supervisor's next worker go to that AI tool — then changes the
// choice, and the next worker follows it, without touching the supervisor. Every worker says
// why it got its model, on the map and in the Ledger's trail. A worker that reports a usage
// limit holds that AI tool back until the owner tries it again. Real CLIs are verified by the
// owner (see the Phase 6 checklist).

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
  openSettings,
  screenshot,
  waitForShell,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();
const env = installFakeTools(home);
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
  await openSettings(browser, "AI models");
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
  await (await objectiveBox(browser)).setValue(objective);
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
    await waitForShell(browser);
    await openSettings(browser, "AI models");
    await waitForText(browser, ROLES, "Senior Developer");
    const models = await textOf(browser, '[aria-labelledby="models-title"]');
    for (const m of [
      "Claude Code (default model)",
      "Codex (default model)",
      "Grok (default model)",
      "Kimi (default model)",
    ]) {
      assert.ok(models.includes(m), `${m} listed`);
    }
    // The AI tools' sign-in and usage limits are on the AI tools page (Phase 19, ADR-060).
    await nav(browser, "AI tools");
    await waitForText(browser, '[aria-label="AI tools"]', "Subscription connected");
    await openSettings(browser, "AI models");
    await waitForText(browser, ROLES, "Senior Developer");
    // Starting choices for built-in roles: the Designer gets a model out of the box, since what a
    // model can do no longer rules one out (Phase 25, item 2.4).
    await waitUntil(async () => {
      const row = await roleRow(browser, "Designer");
      return row.includes("Designer") && !row.includes("None right now");
    }, "the Designer to have a model");
    assert.doesNotMatch(await roleRow(browser, "Designer"), /None right now|not marked as able/);
    await scrollTo(browser, "#role-choices-title");
    await screenshot(browser, "models-settings");

    // Adding a model: the AI tool's own models are a menu (Fable first), not typing.
    await clickButton(browser, "Add a model");
    const form = 'form[aria-label="Add a model"]';
    await (await browser.$(form)).waitForExist({ timeout: 10_000 });
    const menu = await browser.$(
      `//form[@aria-label="Add a model"]//label[.//span[normalize-space()="Model"]]//select`,
    );
    const options = await browser.execute((el) => [...el.options].map((o) => o.textContent), menu);
    // Each short name says which exact version it is now, the exact versions follow, and each
    // says who made it (ADR-081 §6, §8).
    assert.deepEqual(options.slice(0, 10), [
      "The AI tool's default (already in your list)",
      "fable — now Fable 5.1 — made by Anthropic",
      "opus — now Opus 5.5 — made by Anthropic",
      "sonnet — now Sonnet 5.5 — made by Anthropic",
      "haiku — now Haiku 4.5 — made by Anthropic",
      "claude-fable-5-1 — made by Anthropic",
      "claude-opus-5-5 — made by Anthropic",
      "claude-sonnet-5-5 — made by Anthropic",
      "claude-haiku-4-5-20251001 — made by Anthropic",
      "Type another name…",
    ]);
    await menu.selectByAttribute("value", "sonnet");
    await waitUntil(
      async () =>
        (await (await field(browser, "Add a model", "Your name for it")).getValue()) === "Sonnet",
      "the new model to be named Sonnet",
    );
    await screenshot(browser, "models-add-menu");

    // Another AI tool's own models: Grok's, from its adapter (ADR-015). Changing the AI tool
    // rebuilds the menu, so it is looked up afresh each time.
    const MENU = `//form[@aria-label="Add a model"]//label[.//span[normalize-space()="Model"]]//select`;
    const menuOptions = () =>
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
    const chooseTool = async (label) =>
      (
        await browser.$(
          `//form[@aria-label="Add a model"]//label[.//span[normalize-space()="AI tool"]]//select`,
        )
      ).selectByVisibleText(label);
    await chooseTool("Grok");
    await waitUntil(
      async () => (await menuOptions()).includes("grok-4.7 — made by xAI"),
      "Grok's models in the menu",
    );
    assert.deepEqual((await menuOptions()).slice(0, 6), [
      "The AI tool's default (already in your list)",
      "grok-4.7 — made by xAI",
      "grok-4.7-build-fast — made by xAI",
      "grok-4.6 — made by xAI",
      "grok-4.5 — made by xAI",
      "Type another name…",
    ]);
    await screenshot(browser, "models-add-menu-grok");
    // Kimi's models carry their provider's name (ADR-027): only the Kimi subscription's.
    await chooseTool("Kimi");
    await waitUntil(
      async () => (await menuOptions()).includes("kimi-code/k3 — made by Moonshot AI"),
      "Kimi's models in the menu",
    );
    assert.deepEqual((await menuOptions()).slice(0, 6), [
      "The AI tool's default (already in your list)",
      "kimi-code/k3 — made by Moonshot AI",
      "kimi-code/k3-256k — made by Moonshot AI",
      "kimi-code/kimi-for-coding — made by Moonshot AI",
      "kimi-code/kimi-for-coding-highspeed — made by Moonshot AI",
      "Type another name…",
    ]);
    // Antigravity runs several companies' models: each says who made it (ADR-081, ADR-082).
    await chooseTool("Antigravity");
    await waitUntil(
      async () => (await menuOptions()).some((o) => o.startsWith("gemini-3.8-flash-high")),
      "Antigravity's models in the menu",
    );
    const antigravity = await menuOptions();
    assert.equal(antigravity[1], "gemini-3.8-flash-high — made by Google");
    assert.ok(antigravity.includes("claude-sonnet-4-6 — made by Anthropic"), `${antigravity}`);
    assert.ok(antigravity.includes("gpt-oss-120b-medium — made by OpenAI"), `${antigravity}`);
    await screenshot(browser, "models-add-menu-antigravity");
    await chooseTool("Claude Code");
    await waitUntil(
      async () => (await menuOptions()).includes("sonnet — now Sonnet 5.5 — made by Anthropic"),
      "Claude Code's models back in the menu",
    );
    await (await browser.$(MENU)).selectByAttribute("value", "sonnet");
    await waitUntil(
      async () =>
        (await (await field(browser, "Add a model", "Your name for it")).getValue()) === "Sonnet",
      "the new model to be named Sonnet again",
    );
    await submit(browser, form);
    await waitForText(browser, '[aria-labelledby="models-title"]', "Sonnet");
  });

  it("Your models can be grouped by who made them or by AI tool, and the choice is kept (ADR-081)", async () => {
    const { browser } = app;
    const MODELS = '[aria-labelledby="models-title"]';
    await waitForText(browser, MODELS, "Sonnet");
    /** Each group's heading and its models' names, in order. */
    const groups = () =>
      browser.execute(() =>
        [...document.querySelectorAll('table[aria-label="Your models"] tbody')].map((g) => [
          g.getAttribute("aria-label"),
          [...g.querySelectorAll('th[scope="row"]')].map((h) => h.firstChild?.textContent ?? ""),
        ]),
      );
    const groupBy = async (label) => {
      const button = await browser.$(
        `//div[@role="group"][@aria-label="Group your models by"]//button[normalize-space()="${label}"]`,
      );
      await button.waitForClickable({ timeout: 10_000 });
      await button.click();
      await waitUntil(async () => (await button.getAttribute("aria-pressed")) === "true", label);
    };
    // By AI tool to begin with, each AI tool by name.
    const byTool = await groups();
    assert.deepEqual(
      byTool.map(([label]) => label),
      ["Antigravity", "Claude Code", "Codex", "GitHub Copilot", "Grok", "Kimi", "Ollama"],
    );
    await scrollTo(browser, "#models-title");
    await screenshot(browser, "models-grouped-by-tool");
    // By who made them: Ollama's default (OpenAI's gpt-oss) sits with Codex's; Antigravity's and
    // GitHub Copilot's default models are not known (ADR-083: Copilot's Auto picks), so they come
    // last.
    await groupBy("Who made it");
    const byMaker = await groups();
    assert.deepEqual(
      byMaker.map(([label]) => label),
      ["Anthropic", "Moonshot AI", "OpenAI", "xAI", "Not known"],
    );
    const openai = byMaker.find(([label]) => label === "OpenAI")[1];
    assert.deepEqual(openai, ["Codex (default model)", "Ollama (default model)"]);
    assert.deepEqual(byMaker.at(-1)[1].toSorted(), [
      "Antigravity (default model)",
      "GitHub Copilot (default model)",
    ]);
    // The same models, both ways.
    const names = (g) => g.flatMap(([, models]) => models).sort();
    assert.deepEqual(names(byMaker), names(byTool));
    await scrollTo(browser, "#models-title");
    await screenshot(browser, "models-grouped-by-maker");
    // Kept on this PC: shown again after the window reloads.
    await browser.refresh();
    await waitForShell(browser);
    await openSettings(browser, "AI models");
    await waitForText(browser, MODELS, "Sonnet");
    assert.equal((await groups())[0][0], "Anthropic");
    await groupBy("AI tool");
    assert.equal((await groups())[0][0], "Antigravity");
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
    await detailsTab(browser, "AI model");
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
      "Worker brought in for Senior Developer — Claude Code (default model) is Senior Developer's first choice and is ready. It runs at high effort, from Senior Developer's rule.",
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
    await (
      await objectiveBox(browser)
    ).setValue("Once more [handoff:role:Senior Developer+usage-limit]");
    await clickButton(browser, "Give objective");
    await waitForNode(browser, "Website Supervisor, Idle", 30_000);

    // Waiting is the default: no move to Claude Code, and Settings says why.
    await openSettings(browser, "AI models");
    await waitUntil(
      async () => (await roleRow(browser, "Senior Developer")).includes("None right now"),
      "the Senior Developer to wait for Codex",
    );
    assert.match(
      await roleRow(browser, "Senior Developer"),
      /Codex reached its usage limit, and Senior Developer waits for it/,
    );
    await scrollTo(browser, "#role-choices-title");
    await screenshot(browser, "models-usage-limit");
    // The usage limit and Try again now are on Codex's card (Phase 19, ADR-060).
    await nav(browser, "AI tools");
    const again = await browser.$(
      '//li[@aria-label="Codex AI tool"]//button[normalize-space()="Try again now"]',
    );
    await again.waitForClickable({ timeout: 10_000 });
    await again.click();
    await openSettings(browser, "AI models");
    await waitUntil(
      async () =>
        (await roleRow(browser, "Senior Developer")).includes(
          "Codex (default model) is Senior Developer's first choice",
        ),
      "Codex to be back",
    );
  });
});
