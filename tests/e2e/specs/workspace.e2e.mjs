// Phase 21 end-to-end: the workspace in the real app (ADR-092, ADR-093). Panels move, resize,
// pop out into their own window, and come back the same after a restart; Reset layout puts them
// back. The Files panel lists a project's folder and working copies; the owner opens README.md
// in the editor, edits it, and saves it (recorded as the owner's). A file outside the folders
// Plenipo knows cannot be opened. A working copy a worker is writing opens read-only, names the
// worker, shows the worker's change as it lands, and becomes writable after Stop the worker.
// The workers are fake AI tools (plenipo-fake-agent) following markers in their objective.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  installFakeTools,
  launch,
  makeHome,
  nav,
  screenshot,
  waitForShell,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();
const env = installFakeTools(home);
const fake = join(home, ".plenipo-fake-agent");
mkdirSync(fake, { recursive: true });
writeFileSync(join(fake, "auth"), "subscription");

const git = (dir, ...args) =>
  execFileSync("git", ["-C", dir, ...args], { encoding: "utf8" }).trim();

// The Website project's folder: the owner's own git checkout.
const folder = join(home, "website");
mkdirSync(join(folder, "src"), { recursive: true });
writeFileSync(join(folder, "README.md"), "# Website\n\nThe company website.\n");
writeFileSync(
  join(folder, "src", "app.txt"),
  Array.from({ length: 6 }, (_, i) => `line ${i + 1}`).join("\n") + "\n",
);
git(folder, "init", "-q", "-b", "main");
git(folder, "config", "user.name", "Plenipo E2E");
git(folder, "config", "user.email", "e2e@example.com");
git(folder, "add", "-A");
git(folder, "commit", "-q", "-m", "Start");

const tool = (name, args) => `<<tool:${name} ${JSON.stringify(args)}>>`;

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
const field = (browser, form, label, tag = "input") =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//${tag}`);
/** The layout kept on this computer. */
const layout = (browser) =>
  browser.execute(() => JSON.parse(localStorage.getItem("plenipo.layout") ?? "null"));
/** Pick a panel menu's choice ("Terminal panel" → "Pop out"). */
async function panelMenu(browser, panel, choice) {
  await clickButton(browser, `${panel} panel`);
  const item = await browser.$(
    `//button[@role="menuitem"][.//span[normalize-space()="${choice}"]]`,
  );
  await item.waitForClickable({ timeout: 10_000 });
  await item.click();
}
/** Call one of Plenipo's commands from the page, as a page could: its answer or its refusal. */
const invoke = (browser, cmd, args) =>
  browser.execute(
    async (c, a) => {
      try {
        return { ok: await window.__TAURI_INTERNALS__.invoke(c, a) };
      } catch (e) {
        return { error: typeof e === "string" ? e : JSON.stringify(e) };
      }
    },
    cmd,
    args,
  );
/** Give a resize edge the keyboard and press keys on it. */
async function resizeWith(browser, label, keys) {
  await browser.execute(
    (l) => document.querySelector(`[role="separator"][aria-label="${l}"]`)?.focus(),
    label,
  );
  await browser.keys(keys);
}
/** A row of the Files panel, by its name. */
const row = (browser, name) =>
  browser.$(
    `//div[@role="tree"]//div[@role="treeitem"][.//span[contains(@class,"files-tree__name") and normalize-space()="${name}"]]`,
  );

describe("Phase 21 the workspace: panels, windows, files, and the editor (real app)", () => {
  let app;

  before(async () => {
    app = await launch(home, env);
    await app.browser.setWindowSize(1600, 1000);
  });
  after(async () => {
    await app?.close();
  });

  it("sets up a project with a folder", async () => {
    const { browser } = app;
    await waitForShell(browser);
    await nav(browser, "Projects");
    await clickButton(browser, "Set up a Development project");
    const form = "Set up a Development project";
    await (await field(browser, form, "Name")).setValue("Website");
    await (await field(browser, form, "Description", "textarea")).setValue("The company website");
    await (await field(browser, form, "Project folder (optional)")).setValue(folder);
    const submit = await browser.$(`form[aria-label="${form}"] button[type="submit"]`);
    await submit.click();
    await waitUntil(
      async () => !(await exists(browser, `form[aria-label="${form}"]`)),
      "the dialog to close",
    );
    await waitForText(browser, '[aria-label="About the project"]', "Website Supervisor");
  });

  it("panels: the terminal moves to the right, resizes, and Files docks on the left", async () => {
    const { browser } = app;
    await clickButton(browser, "Terminal");
    await waitUntil(
      () => exists(browser, 'section[data-dock="bottom"]:not([hidden])'),
      "the bottom dock",
    );
    await panelMenu(browser, "Terminal", "Move to the right");
    await waitUntil(
      () =>
        exists(browser, 'section[data-dock="right"]:not([hidden]) section[aria-label="Terminal"]'),
      "the terminal on the right",
    );
    const edge = await browser.$('[role="separator"][aria-label="Resize the panels on the right"]');
    await resizeWith(browser, "Resize the panels on the right", ["ArrowLeft", "ArrowLeft"]);
    await waitUntil(
      async () => (await edge.getAttribute("aria-valuenow")) === "452",
      "a wider panel",
    );
    await clickButton(browser, "Files");
    await waitUntil(
      () => exists(browser, 'section[data-dock="left"]:not([hidden]) section[aria-label="Files"]'),
      "Files on the left",
    );
    const kept = await layout(browser);
    assert.equal(kept.panels.terminal.dock, "right");
    assert.equal(kept.docks.right.size, 452);
    assert.equal(kept.panels.files.dock, "left");
    await screenshot(browser, "workspace-docked");
  });

  it("pops the terminal out into its own window, the same terminal, and puts it back", async () => {
    const { browser } = app;
    const before = (await browser.getWindowHandles()).length;
    await panelMenu(browser, "Terminal", "Pop out");
    // The terminal's parts leave the organization's window for the pop-out.
    await waitUntil(
      () => browser.execute(() => document.querySelector(".panel-host--terminal") === null),
      "the terminal to move into its own window",
    );
    assert.equal((await layout(browser)).panels.terminal.popped, true);
    const handles = await browser.getWindowHandles();
    if (handles.length > before) {
      const main = await browser.getWindowHandle();
      await browser.switchToWindow(handles.find((h) => h !== main));
      await waitUntil(
        () => exists(browser, 'section[aria-label="Terminal"]'),
        "the terminal in the pop-out",
      );
      await screenshot(browser, "workspace-terminal-popped-out");
      await browser.switchToWindow(main);
    }
    await screenshot(browser, "workspace-main-while-popped-out");
    // Put back: the same panel, in its dock again.
    await panelMenu(browser, "Terminal", "Put back");
    await waitUntil(
      () => exists(browser, 'section[data-dock="right"] .panel-host--terminal'),
      "the terminal back on the right",
    );
    assert.equal((await layout(browser)).panels.terminal.popped, false);
  });

  it("a pop-out has no commands of its own", async () => {
    const { browser } = app;
    // Plenipo refuses a new window the page did not ask for.
    const opened = await browser.execute(() => window.open("about:blank", "_blank") !== null);
    await browser.pause(500);
    const handles = (await browser.getWindowHandles()).length;
    assert.ok(!opened || handles === 1, "a window Plenipo was not asked for does not open");
  });

  it("the layout comes back the same after a restart, popped-out panel included", async () => {
    await panelMenu(app.browser, "Terminal", "Pop out");
    await waitUntil(
      () => app.browser.execute(() => document.querySelector(".panel-host--terminal") === null),
      "popped out",
    );
    await app.close();
    app = await launch(home, env);
    await app.browser.setWindowSize(1600, 1000);
    const { browser } = app;
    await waitForShell(browser);
    await waitUntil(
      async () => (await layout(browser))?.panels.terminal.popped === true,
      "the kept layout",
    );
    // The pop-out opens again by itself.
    await waitUntil(
      () => browser.execute(() => document.querySelector(".panel-host--terminal") === null),
      "the terminal in its own window again",
      30_000,
    );
    await waitUntil(
      () => exists(browser, 'section[data-dock="left"]:not([hidden]) section[aria-label="Files"]'),
      "Files still on the left",
    );
    const size = await (
      await browser.$('[role="separator"][aria-label="Resize the panels on the left"]')
    ).getAttribute("aria-valuenow");
    assert.ok(Number(size) >= 120);
    await screenshot(browser, "workspace-after-restart");
  });

  it("Reset layout puts every panel back where it started", async () => {
    const { browser } = app;
    await panelMenu(browser, "Files", "Reset layout");
    await waitUntil(async () => {
      const l = await layout(browser);
      return (
        l &&
        !l.panels.terminal.popped &&
        l.panels.terminal.dock === "bottom" &&
        l.panels.files.dock === "left" &&
        !l.docks.left.open &&
        !l.docks.bottom.open
      );
    }, "the first layout");
    await waitUntil(
      () =>
        exists(
          browser,
          'section[data-dock="bottom"] .panel-host--terminal, .panel-parking .panel-host--terminal',
        ),
      "the terminal back home",
    );
  });

  it("opens README.md from Files, edits it, and saves it as yours", async () => {
    const { browser } = app;
    await clickButton(browser, "Files");
    await (await row(browser, "Website")).click();
    await browser.keys(["ArrowRight"]);
    await (await row(browser, "Project folder")).waitForExist({ timeout: 20_000 });
    await (await row(browser, "Project folder")).doubleClick();
    await (await row(browser, "README.md")).waitForExist({ timeout: 20_000 });
    await (await row(browser, "README.md")).doubleClick();
    await waitForText(browser, ".file-editor", "README.md", 20_000);
    const editor = await browser.$(".cm-content");
    await editor.waitForExist({ timeout: 20_000 });
    await editor.click();
    await browser.keys(["Control", "End"]);
    await browser.keys(["Control"]);
    await browser.keys("Edited in Plenipo.");
    await waitForText(browser, ".file-editor__state", "Not saved yet");
    await screenshot(browser, "workspace-editor-unsaved");
    await clickButton(browser, "Save");
    await waitForText(browser, ".file-editor__state", "Saved");
    assert.match(readFileSync(join(folder, "README.md"), "utf8"), /Edited in Plenipo\./);
    await nav(browser, "Activity");
    await waitForText(browser, "main", "README.md", 20_000);
    await screenshot(browser, "workspace-saved-in-activity");
  });

  it("a file outside the folders Plenipo knows cannot be opened", async () => {
    const { browser } = app;
    const roots = await invoke(browser, "get_file_roots", {});
    const root = roots.ok.roots.find((r) => r.kind === "projectFolder").id;
    for (const path of ["../../etc/passwd", "/etc/passwd", ".git/config"]) {
      const answer = await invoke(browser, "read_file", { root, path });
      assert.ok(answer.error, `${path} is refused`);
    }
    const unknown = await invoke(browser, "read_file", { root: "project:nope", path: "README.md" });
    assert.ok(unknown.error);
  });

  it("a working copy a worker is writing opens read-only, shows its change as it lands, and Stop the worker makes it writable", async () => {
    const { browser } = app;
    const lines = Array.from({ length: 12 }, (_, i) => `line ${i + 1} (from the Senior Developer)`);
    const work = [
      "[stream-writes:1500]",
      tool("write_file", { path: "src/app.txt", content: `${lines.join("\n")}\n` }),
      "[delay:20000]",
    ].join(" ");
    await nav(browser, "Projects");
    const form = 'form[aria-label="Give an objective"]';
    await waitUntil(() => exists(browser, form), "the objective form");
    await (
      await browser.$(`${form} select`)
    ).selectByVisibleText("Website Supervisor (Supervisor)");
    await (
      await browser.$(`${form} textarea`)
    ).setValue(`Update the app {{handoff:role:Senior Developer|${work}}}`);
    await clickButton(browser, "Give objective");
    // Its working copy appears in Files, with who is writing there.
    await waitUntil(
      async () => {
        const r = await invoke(browser, "get_file_roots", {});
        return r.ok?.roots.some((x) => x.kind === "workingCopy" && x.writer);
      },
      "the working copy with its writer",
      90_000,
    );
    const roots = await invoke(browser, "get_file_roots", {});
    const copy = roots.ok.roots.find((x) => x.kind === "workingCopy");
    await browser.execute((id) => {
      window.history.replaceState(null, "");
      localStorage.setItem("plenipo.place", JSON.stringify({ view: "file", id }));
    }, `${copy.id}/src/app.txt`);
    await browser.refresh();
    await waitForShell(browser);
    await waitForText(browser, ".file-editor", "is writing in this working copy", 30_000);
    assert.equal(await (await browser.$(".cm-content")).getAttribute("aria-readonly"), "true");
    // The worker's change lands in the editor as it is written, then saved.
    await waitForText(browser, ".file-editor", "from the Senior Developer", 60_000);
    await screenshot(browser, "workspace-watch-in-editor");
    await clickButton(browser, "Stop the worker");
    await screenshot(browser, "workspace-stop-the-worker");
    await clickButton(browser, "Stop the worker");
    await waitUntil(
      async () =>
        !(await textOf(browser, ".file-editor")).includes("is writing in this working copy"),
      "the working copy to be free",
      60_000,
    );
    await waitUntil(
      async () => (await (await browser.$(".cm-content")).getAttribute("aria-readonly")) === null,
      "the editor to be writable",
      30_000,
    );
    await screenshot(browser, "workspace-writable-again");
  });
});
