// Phase 2 end-to-end: durable Ledger in the real app.

import assert from "node:assert/strict";
import { existsSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  appPids,
  clickButton,
  DETAIL,
  launch,
  LOG,
  makeHome,
  nav,
  screenshot,
  textOf,
  waitForText,
  waitPidGone,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();
const dbPath = join(home, ".local", "share", "com.eightwest.plenipo", "ledger", "plenipo.db");
const TRAIL = '[aria-label="Activity trail"]';

/**
 * Read the whole trail in one in-page snapshot. (Reading element by element over WebDriver
 * races with the live re-render that follows each ledger event: stale element references.)
 */
async function trailItems(browser) {
  return browser.execute((selector) => {
    return [...document.querySelectorAll(`${selector} li`)].map((li) => ({
      type: li.getAttribute("data-event-type"),
      seq: Number(li.getAttribute("data-seq")),
      text: li.innerText.replace(/\s+/g, " ").trim(),
    }));
  }, TRAIL);
}

/** Wait until the trail has exactly `count` entries (i.e. the latest refresh has landed). */
const waitForTrailLength = (browser, count) =>
  waitUntil(async () => (await trailItems(browser)).length === count, `${count} trail entries`);

/** Open a task by exact objective (sub-tasks share the parent's prefix, so match exactly). */
async function openActivityTask(browser, objective) {
  await nav(browser, "Activity");
  const task = await browser.$(
    `//button[starts-with(@aria-label, "${objective} — ") and not(contains(@aria-label, "— step"))]`,
  );
  await task.waitForExist({ timeout: 10_000 });
  await task.click();
  await (await browser.$(TRAIL)).waitForExist({ timeout: 10_000 });
}

describe("Phase 2 ledger (real app)", () => {
  let app;
  let trailBefore;
  const objective = "Synthetic diagnostic task #1";

  before(async () => {
    app = await launch(home);
  });
  after(async () => {
    await app?.close();
  });

  it("records a synthetic task with a complete ordered trail", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    await nav(browser, "Diagnostics");
    await clickButton(browser, "Create synthetic task");
    await waitForText(browser, '[role="status"]', "Created");

    await openActivityTask(browser, objective);
    await waitForTrailLength(browser, 1);
    await clickButton(browser, "Start");
    await waitForText(browser, TRAIL, "Queued → Running");
    await clickButton(browser, "Add step");
    // Adding a step keeps the parent selected; the step appears under Sub-tasks.
    await waitForText(browser, TRAIL, "Sub-task created");
    await waitForText(browser, ".children", "step 1");
    await waitForText(browser, DETAIL, objective);
    await waitForTrailLength(browser, 3);
    await clickButton(browser, "Await approval");
    await waitForText(browser, TRAIL, "Running → Awaiting approval");
    await clickButton(browser, "Resume");
    await waitForText(browser, TRAIL, "Awaiting approval → Running");
    await waitForTrailLength(browser, 5);

    const items = await trailItems(browser);
    assert.deepEqual(
      items.map((i) => i.type),
      [
        "task.created",
        "task.state_changed",
        "task.child_created",
        "task.state_changed",
        "task.state_changed",
      ],
    );
    const seqs = items.map((i) => i.seq);
    assert.ok(
      seqs.every((s, i) => i === 0 || s > seqs[i - 1]),
      `ordered: ${seqs}`,
    );
    trailBefore = items;
    await screenshot(browser, "activity-trail");
  });

  it("rejects an invalid transition and records the attempt", async () => {
    const { browser } = app;
    const result = await browser.execute(async () => {
      const invoke = window.__TAURI_INTERNALS__.invoke;
      const tasks = await invoke("list_tasks");
      const child = tasks.find((t) => t.parentTaskId !== null);
      try {
        await invoke("advance_synthetic_task", { taskId: child.id, action: "complete" });
        return "ALLOWED";
      } catch (e) {
        return JSON.stringify(e);
      }
    });
    assert.match(result, /invalidInput/);
    assert.match(result, /queued -> succeeded/);

    await (await browser.$(`//button[contains(., "step 1")]`)).click();
    await waitForText(browser, TRAIL, "Rejected: Queued → Succeeded is not allowed");
    await clickButton(browser, "↑ Parent task");
    await waitForText(browser, DETAIL, objective);
  });

  it("records runtime executions in the ledger", async () => {
    const { browser } = app;
    await nav(browser, "Runtimes");
    await clickButton(browser, "Start Echo test");
    await waitForText(browser, LOG, "echo complete");
    await nav(browser, "Activity");
    await clickButton(browser, "All events");
    await waitForText(browser, '[aria-label="All events"]', "Echo test: succeeded · exit 0");
  });

  it("keeps the full history after Plenipo is killed and relaunched", async () => {
    const [pid] = appPids();
    process.kill(pid, "SIGKILL"); // no graceful shutdown at all
    await waitPidGone(pid);
    await app.close();

    app = await launch(home);
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    assert.equal(await (await browser.$(".banner--severe")).isExisting(), false, "no corruption");
    await openActivityTask(browser, objective);
    await waitForText(browser, DETAIL, "Running");
    await waitForTrailLength(browser, trailBefore.length);
    const after = await trailItems(browser);
    assert.deepEqual(after, trailBefore, "identical ordered trail after a hard kill");
    // Phase 1 runtime history also survived, via the ledger.
    await nav(browser, "Runtimes");
    const item = await browser.$('//button[contains(@aria-label, "Echo test — Succeeded")]');
    await item.waitForExist({ timeout: 10_000 });
  });

  it("creates a verified backup", async () => {
    const { browser } = app;
    await nav(browser, "Diagnostics");
    await clickButton(browser, "Create backup");
    await waitForText(browser, '[role="status"]', "Backup saved and verified");
    const text = await textOf(browser, '[role="status"]');
    const path = /: (.+\.db)$/.exec(text)[1];
    assert.ok(existsSync(path), `backup exists at ${path}`);
    await screenshot(browser, "diagnostics-ledger");
  });

  it("does not silently ignore a corrupted database", async () => {
    await app.close(); // WebDriver session end terminates the app
    for (const pid of appPids()) await waitPidGone(pid);
    // Simulate disk damage: the database and its write-ahead log are both destroyed. (Garbage
    // in the main file alone is healed by SQLite from an intact WAL, which is correct.)
    rmSync(`${dbPath}-wal`, { force: true });
    rmSync(`${dbPath}-shm`, { force: true });
    writeFileSync(dbPath, Buffer.alloc(64 * 1024, 0x5a));

    app = await launch(home);
    const { browser } = app;
    await waitForText(browser, ".banner--severe", "failed its integrity check");
    const banner = await textOf(browser, ".banner--severe");
    assert.match(banner, /moved to .*plenipo\.db\.corrupt-\d+/);
    assert.match(banner, /Backups are in/);
    await screenshot(browser, "corruption-banner");
    // The app keeps working on a fresh ledger.
    await nav(browser, "Diagnostics");
    await clickButton(browser, "Create synthetic task");
    await waitForText(browser, '[role="status"]', "Created");
  });
});
