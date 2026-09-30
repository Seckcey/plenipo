// Phase 21 end-to-end: more than one organization in the real app (ADR-094). The owner makes a
// second organization for a client from the Organizations menu; it opens in a window of its own.
// Work, secrets, and backups made in one window never show in the other; the first window can
// switch to it; archiving it closes its window. By 8 West Ventures, LLC.

import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  launch,
  makeHome,
  screenshot,
  waitForShell,
  waitForText,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();

/** Call one of Plenipo's commands from the page, as a page could: its answer or its refusal. */
const invoke = (browser, cmd, args = {}) =>
  browser.executeAsync(
    (c, a, done) => {
      window.__TAURI_INTERNALS__.invoke(c, a).then(
        (ok) => done({ ok }),
        // Not `error`: WebDriver takes an answer with an `error` field for its own error.
        (e) => done({ refused: typeof e === "string" ? e : JSON.stringify(e) }),
      );
    },
    cmd,
    args,
  );

/** Pick a choice in the Organizations menu at the top. */
async function organizationsMenu(browser, choice) {
  await clickButton(browser, "Organizations");
  const item = await browser.$(
    `//button[@role="menuitem"][.//span[normalize-space()="${choice}"]]`,
  );
  await item.waitForClickable({ timeout: 10_000 });
  await item.click();
}

describe("Phase 21 more than one organization (real app)", () => {
  let app;
  let main;
  let client;
  let clientId;

  before(async () => {
    app = await launch(home, {});
    await app.browser.setWindowSize(1400, 900);
  });
  after(async () => {
    await app?.close();
  });

  it("makes a client's organization, which opens in a window of its own", async () => {
    const { browser } = app;
    await waitForShell(browser);
    main = await browser.getWindowHandle();
    await organizationsMenu(browser, "New organization…");
    const form = await browser.$('//form[@aria-label="New organization"]');
    await form.waitForExist({ timeout: 10_000 });
    const name = await form.$('.//label[.//span[normalize-space()="Name"]]//input');
    await name.setValue("Client Co");
    await screenshot(browser, "organizations-new");
    await clickButton(browser, "Create and open");
    await waitUntil(
      async () => (await browser.getWindowHandles()).length === 2,
      "the client's window",
      30_000,
    );
    client = (await browser.getWindowHandles()).find((h) => h !== main);
    await browser.switchToWindow(client);
    await waitForShell(browser);
    const listing = (await invoke(browser, "get_organizations")).ok;
    const here = listing.organizations.find((o) => o.here);
    assert.equal(here.name, "Client Co");
    assert.equal(here.first, false);
    clientId = here.id;
    await waitForText(browser, "header", "All of Client Co");
    await screenshot(browser, "organizations-client-window");
    await browser.switchToWindow(main);
  });

  it("keeps work, secrets, and approvals apart between the two windows", async () => {
    const { browser } = app;
    // Work in the first organization.
    const task = (await invoke(browser, "create_synthetic_task")).ok;
    assert.ok(task.id);
    // A secret in the client's.
    await browser.switchToWindow(client);
    const saved = await invoke(browser, "save_secret", {
      input: {
        name: "Client token",
        envVar: "CLIENT_TOKEN",
        programs: ["gh"],
        value: "client_secret_value_123",
      },
    });
    assert.ok(saved.ok, saved.refused);
    const theirTasks = (await invoke(browser, "list_tasks")).ok;
    assert.ok(
      theirTasks.every((t) => t.id !== task.id),
      "the client sees none of the first's work",
    );
    const theirApprovals = (await invoke(browser, "get_approvals")).ok;
    await browser.switchToWindow(main);
    const mine = (await invoke(browser, "get_permissions")).ok;
    assert.ok(mine.settings.secrets.every((s) => s.name !== "Client token"));
    const myTasks = (await invoke(browser, "list_tasks")).ok;
    assert.ok(myTasks.some((t) => t.id === task.id));
    const myApprovals = (await invoke(browser, "get_approvals")).ok;
    assert.deepEqual(
      theirApprovals.pending
        .map((a) => a.id)
        .filter((id) => myApprovals.pending.some((m) => m.id === id)),
      [],
    );
    // A window cannot reach another organization's task.
    await browser.switchToWindow(client);
    const other = await invoke(browser, "get_task_timeline", { taskId: task.id });
    assert.ok(other.refused, "the first organization's task is not the client's");
    await browser.switchToWindow(main);
  });

  it("keeps each organization's backups apart", async () => {
    const { browser } = app;
    await browser.switchToWindow(client);
    const made = (await invoke(browser, "create_ledger_backup")).ok;
    // Beside the client's own Ledger, in its own folder.
    assert.ok(made.path.includes(clientId), made.path);
    const name = made.path.split(/[\\/]/).pop();
    const theirs = (await invoke(browser, "list_ledger_backups")).ok;
    assert.ok(theirs.backups.some((b) => b.name === name));
    assert.match(theirs.folder, /organizations/);
    await browser.switchToWindow(main);
    const mine = (await invoke(browser, "list_ledger_backups")).ok;
    assert.ok(mine.backups.every((b) => b.name !== name));
    assert.notEqual(mine.folder, theirs.folder);
  });

  it("lists both organizations in Settings, and archiving the client's closes its window", async () => {
    const { browser } = app;
    await browser.switchToWindow(main);
    await organizationsMenu(browser, "Your organizations…");
    const list = await browser.$('//ul[@aria-label="Your organizations"]');
    await list.waitForExist({ timeout: 10_000 });
    await waitForText(browser, 'ul[aria-label="Your organizations"]', "Client Co");
    await waitForText(browser, 'ul[aria-label="Your organizations"]', "Open in another window");
    await screenshot(browser, "organizations-settings");
    const row = await browser.$(
      `//ul[@aria-label="Your organizations"]/li[.//strong[normalize-space()="Client Co"]]`,
    );
    await (await row.$('.//button[normalize-space()="Archive organization"]')).click();
    await waitUntil(
      async () => (await browser.getWindowHandles()).length === 1,
      "the client's window to close",
      30_000,
    );
    await waitForText(browser, 'ul[aria-label="Your organizations"]', "Archived");
    const listing = (await invoke(browser, "get_organizations")).ok;
    assert.ok(listing.organizations.some((o) => o.id === clientId && o.archived));
  });
});
