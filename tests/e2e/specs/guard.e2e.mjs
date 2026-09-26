// Phase 7 end-to-end: Plenipo Guard, the capability broker, and human approval, in the real app,
// against fake `claude` and `codex` CLIs (plenipo-fake-agent) that call Plenipo's tools over MCP
// through the real relay (`plenipo-desktop --plenipo-tools=<ticket>`). The owner gives a project
// its folder; the Senior Developer's worker reads and writes there and runs an approved command;
// a request outside the folder is blocked and shown; a push stops for an approval card and runs
// only after the owner approves it. Real CLIs are verified by the owner (Phase 7 checklist).

import assert from "node:assert/strict";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
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

// The Website project's folder.
const folder = join(home, "website");
mkdirSync(join(folder, "src"), { recursive: true });
writeFileSync(join(folder, "README.md"), "# Website\nThe company website.\n");
writeFileSync(join(home, "outside.txt"), "not yours\n");

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

const nodeXPath = (label) => `//button[@data-node-id and starts-with(@aria-label, "${label}")]`;

function nodes(browser) {
  return browser.execute(() =>
    [...document.querySelectorAll("button[data-node-id]")].map((b) => b.getAttribute("aria-label")),
  );
}

const waitForNode = (browser, label, timeoutMs = 20_000) =>
  waitUntil(
    async () => (await nodes(browser)).find((l) => l.startsWith(label)) ?? null,
    `node "${label}"`,
    timeoutMs,
  );

function exists(browser, selector) {
  return browser.execute((s) => document.querySelector(s) !== null, selector);
}

function scrollTo(browser, selector) {
  return browser.execute(
    (s) => document.querySelector(s)?.scrollIntoView({ block: "start" }),
    selector,
  );
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

/** A Plenipo tool call in the fake agent's objective. */
const tool = (name, args) => `<<tool:${name} ${JSON.stringify(args)}>>`;

describe("Phase 7 Guard, capability broker, and approvals (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    app = await launch(home, env);
    await app.browser.setWindowSize(1600, 1000);
  });
  after(async () => {
    await app?.close();
  });

  it("Settings → Permissions shows each role's permission set, the rules, and the Vault", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    await nav(browser, "Settings");
    await waitForText(browser, '[aria-labelledby="who-title"]', "Senior Developer");
    const dev = await browser.$('select[aria-label="Senior Developer\'s permission set"]');
    assert.equal(await dev.getValue(), "developer");
    const reviewer = await browser.$('select[aria-label="Code Reviewer\'s permission set"]');
    assert.equal(await reviewer.getValue(), "reviewer");
    await waitForText(browser, ".permissions", "Tools ready");
    await waitForText(browser, '[aria-labelledby="sets-title"]', "Developer");
    await waitForText(
      browser,
      '[aria-labelledby="sensitive-title"]',
      "Deploying or changing live systems",
    );
    // The approved list gets `git --version` for this test (Enter starts a new line).
    const approved = await field(
      browser,
      "Command lists",
      "Approved: run without asking",
      "textarea",
    );
    await approved.addValue("\uE007git --version *");
    assert.match(await approved.getValue(), /\nmake check \*\ngit --version \*$/);
    await clickButton(browser, "Save command lists");
    // Saved: it is still there after leaving the page and coming back.
    await browser.pause(500);
    assert.ok(!(await exists(browser, 'form[aria-label="Command lists"] [role="alert"]')));
    await nav(browser, "Organization");
    await nav(browser, "Settings");
    await waitUntil(
      async () =>
        (
          await (
            await field(browser, "Command lists", "Approved: run without asking", "textarea")
          ).getValue()
        ).endsWith("\ngit --version *"),
      "the saved approved list",
    );
    await scrollTo(browser, "#who-title");
    await screenshot(browser, "permissions-settings");
    await scrollTo(browser, "#commands-title");
    await screenshot(browser, "permissions-commands");
  });

  it("gives the Website project a folder and builds a team", async () => {
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
    await select(browser, "Website Supervisor");
    await clickButton(browser, "Hire Senior Developer");
    await submit(browser, 'form[aria-label="Hire"]');
    await waitForNode(browser, "Senior Developer,");
  });

  it("acceptance: the worker works only in its folder, a blocked request is visible, and a push waits for approval", async () => {
    const { browser } = app;
    const work = [
      tool("read_file", { path: "README.md" }),
      tool("write_file", { path: "src/hello.txt", content: "written by the worker" }),
      tool("run_command", { program: "git", args: ["--version"] }),
      tool("read_file", { path: "../outside.txt" }),
      tool("git_push", {}),
    ].join(" ");
    await nav(browser, "Organization");
    await select(browser, "Website Supervisor");
    const form = 'form[aria-label="Give an objective"]';
    await (
      await browser.$(`${form} textarea`)
    ).setValue(`Ship it {{handoff:role:Senior Developer|${work}}}`);
    await clickButton(browser, "Give objective");

    // The push pauses: a banner on every page, and a count in the sidebar.
    await waitForText(browser, ".banner--approval", "is waiting for your approval", 45_000);
    await waitForText(browser, ".banner--approval", "git push origin");
    await screenshot(browser, "approval-banner");
    // Before the approval, reading and writing inside the folder already happened.
    assert.equal(readFileSync(join(folder, "src", "hello.txt"), "utf8"), "written by the worker");

    await clickButton(browser, "Review");
    const card = 'article[aria-label^="Senior Developer wants to git push origin"]';
    await waitUntil(() => exists(browser, card), "the approval card");
    const text = await textOf(browser, card);
    assert.match(text, /Senior Developer · Website project/);
    assert.match(
      text,
      /Why it needs you: Git push origin needs your approval: it sends commits to a server/,
    );
    assert.match(text, /Sending or publishing outside this computer/);
    assert.match(text, /min left/);
    // The request outside the folder was blocked, and it shows.
    await waitForText(browser, '[aria-labelledby="blocked-title"]', "tried to read ../outside.txt");
    await waitForText(browser, '[aria-labelledby="grants-title"]', "Senior Developer");
    await screenshot(browser, "approval-card");

    await clickButton(browser, "Approve");
    await waitForText(
      browser,
      '[aria-labelledby="waiting-title"]',
      "Nothing is waiting for your approval.",
    );
    await waitForText(browser, '[aria-labelledby="answered-title"]', "Approved");
    await screenshot(browser, "approvals-answered");
    assert.ok(!existsSync(join(home, "src")), "nothing was written outside the folder");
  });

  it("the Activity trail records the grant, each use, the refusal, and the approval", async () => {
    const { browser } = app;
    await nav(browser, "Activity");
    const task = await browser.$('//button[starts-with(@aria-label, "Ship it")]');
    await task.waitForExist({ timeout: 10_000 });
    await task.click();
    const tree = '[aria-label="Delegation tree"]';
    await waitForText(browser, tree, "read_file");
    await (await browser.$('(//ol[@aria-label="Delegation tree"]//button)[2]')).click();
    const trail = '[aria-label="Activity trail"]';
    await waitForText(browser, trail, "Permissions given to Senior Developer", 30_000);
    await waitForText(browser, trail, "Senior Developer: read README.md");
    await waitForText(browser, trail, "Senior Developer: write src/hello.txt");
    await waitForText(browser, trail, "Senior Developer: run git --version");
    await waitForText(browser, trail, "Blocked: Senior Developer tried to read ../outside.txt");
    await waitForText(browser, trail, "Waiting for your approval: git push origin");
    await waitForText(browser, trail, "Approved: git push origin");
    await waitForText(browser, trail, "Permissions ended for Senior Developer", 30_000);
    await browser.execute((s) => {
      const row = [...document.querySelectorAll(`${s} li`)].find((li) =>
        li.innerText.includes("Permissions given"),
      );
      row?.scrollIntoView({ block: "start" });
    }, trail);
    await screenshot(browser, "guard-trail");
  });
});
