// Phase 3 end-to-end: agent runtimes in the real app, driven through the UI against fake
// `claude` and `codex` CLIs (plenipo-fake-agent) that speak each provider's stream format.
// Real CLIs with real sign-ins are verified by the owner (see the Phase 3 checklist).

import assert from "node:assert/strict";
import { chmodSync, copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  appPids,
  clickButton,
  launch,
  makeHome,
  nav,
  screenshot,
  textOf,
  waitForText,
  waitPidGone,
  waitUntil,
} from "../lib/app.mjs";

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
const stateDir = join(home, ".plenipo-fake-agent");
const setAuth = (mode) => {
  mkdirSync(stateDir, { recursive: true });
  writeFileSync(join(stateDir, "auth"), mode);
};

const TURNS = '[aria-label="Turns"]';
const NEW_TASK = 'form[aria-label="New task"]';

/** Snapshot the turns of the selected session in one in-page read. */
function turns(browser) {
  return browser.execute((selector) => {
    return [...document.querySelectorAll(`${selector} > li`)].map((li) => ({
      running: li.getAttribute("data-running") === "true",
      outcome: li.getAttribute("data-outcome"),
      text: li.innerText.replace(/\s+/g, " ").trim(),
    }));
  }, TURNS);
}

const waitForTurn = (browser, n, predicate, what, timeoutMs = 30_000) =>
  waitUntil(
    async () => {
      const t = (await turns(browser))[n - 1];
      return t && predicate(t) ? t : null;
    },
    what,
    timeoutMs,
  );

async function startTask(browser, runtimeLabel, objective) {
  await nav(browser, "Workers");
  const radio = await browser.$(`//label[.//span[normalize-space()="${runtimeLabel}"]]//input`);
  await radio.waitForExist({ timeout: 10_000 });
  await radio.click();
  await waitUntil(
    async () => (await textOf(browser, NEW_TASK)).includes("Ready"),
    `${runtimeLabel} ready`,
  );
  const box = await browser.$(`${NEW_TASK} textarea`);
  await box.setValue(objective);
  await clickButton(browser, "Start task");
  // Wait until the new session is the one shown (the previous selection stays visible until
  // the command returns).
  await waitForText(browser, ".detail__header h2", objective);
}

async function openSession(browser, title) {
  await nav(browser, "Workers");
  const item = await browser.$(`//ul[@aria-label="Sessions"]//button[contains(., "${title}")]`);
  await item.waitForExist({ timeout: 10_000 });
  await item.click();
  await waitForText(browser, ".detail__header", title);
}

async function followUp(browser, objective) {
  const box = await browser.$('form[aria-label="Continue session"] textarea');
  await box.setValue(objective);
  await clickButton(browser, "Send");
}

describe("Phase 3 agent runtimes (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    setAuth("subscription");
    app = await launch(home, env);
  });
  after(async () => {
    await app?.close();
  });

  it("detects both runtimes, their versions, and subscription sign-in", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    await nav(browser, "Runtimes");
    const cards = '[aria-label="Agent runtimes"]';
    await waitUntil(
      async () => (await textOf(browser, cards)).match(/Ready/g)?.length === 2,
      "both runtimes ready",
    );
    const text = await textOf(browser, cards);
    assert.match(text, /Claude Code[\s\S]*v2\.1\.999[\s\S]*Signed in \(subscription\)/);
    assert.match(text, /Codex[\s\S]*v0\.99\.0[\s\S]*ChatGPT sign-in/);
    assert.doesNotMatch(text, /owner@example\.com/, "no account identifiers shown");
    await screenshot(browser, "agent-runtimes");
  });

  it("A1+A3+A4: launches a Codex task with live activity and a normalized result", async () => {
    const { browser } = app;
    await startTask(browser, "Codex", "List the workspace");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Codex result");
    assert.match(t.text, /Turn 1: you said "List the workspace"\. Previous: None\./);
    assert.match(t.text, /20 in \(8 cached\) · 9 out/);
    // Live activity included Codex's command and message.
    await (await browser.$('//summary[contains(., "Live activity")]')).click();
    await waitForText(browser, TURNS, "bash -lc ls");
  });

  it("A2+A3+A4: launches a Claude Code task with streamed text and a normalized result", async () => {
    const { browser } = app;
    await startTask(browser, "Claude Code", "Say hello");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Claude result");
    assert.match(t.text, /Turn 1: you said "Say hello"\. Previous: None\./);
    await waitForText(browser, ".detail__header", "Provider session");
    // Finished sessions are not shown as running (the start response can arrive after the
    // live "finished" update for a fast turn).
    await waitUntil(
      async () => !(await textOf(browser, '[aria-label="Sessions"]')).includes("Running"),
      "no session shown as running",
    );
    await screenshot(browser, "worker-result");
  });

  it("A5: resumes both sessions in the same provider session", async () => {
    const { browser } = app;
    for (const [title, first] of [
      ["Say hello", "Say hello"],
      ["List the workspace", "List the workspace"],
    ]) {
      await openSession(browser, title);
      await followUp(browser, "And again?");
      const t = await waitForTurn(browser, 2, (t) => t.outcome === "completed", `${title} turn 2`);
      assert.match(
        t.text,
        new RegExp(`Turn 2: you said "And again\\?"\\. Previous: Some\\("${first}"\\)`),
      );
    }
  });

  it("A3+A6: shows live activity and cancels an active task", async () => {
    const { browser } = app;
    await startTask(browser, "Claude Code", "Count slowly [slow]");
    // Live: streamed ticks appear while the turn is still running.
    await waitForTurn(browser, 1, (t) => t.running && /tick 3/.test(t.text), "live ticks", 20_000);
    await screenshot(browser, "worker-live");
    await clickButton(browser, "Cancel turn");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "cancelled", "cancelled");
    assert.match(t.text, /Cancelled/);
    // The session stays usable: resume after cancel.
    await followUp(browser, "Done counting?");
    await waitForTurn(browser, 2, (t) => t.outcome === "completed", "resume after cancel");
  });

  it("A7: preserves the executions and turns in the Ledger", async () => {
    const { browser } = app;
    await nav(browser, "Activity");
    const task = await browser.$('//button[starts-with(@aria-label, "Say hello — Succeeded")]');
    await task.waitForExist({ timeout: 10_000 });
    await task.click();
    const trail = '[aria-label="Activity trail"]';
    await waitForText(browser, trail, "Result: Completed");
    const text = await textOf(browser, trail);
    assert.match(text, /Task created: Say hello/);
    assert.match(text, /Provider session /);
    assert.match(text, /Claude Code · turn 1: running/);
    assert.match(text, /Claude Code · turn 1: succeeded · exit 0/);
    assert.match(text, /Agent: Turn 1: you said/);
    await screenshot(browser, "worker-ledger-trail");
    // Raw provider output stays available for diagnostics.
    await nav(browser, "Runtimes");
    await (
      await browser.$('//button[contains(@aria-label, "Codex · turn 1 — Succeeded")]')
    ).waitForExist({ timeout: 10_000 });
  });

  it("marks a turn interrupted when Plenipo is killed mid-turn, keeping all history", async () => {
    let { browser } = app;
    await startTask(browser, "Codex", "Think slowly [slow]");
    await waitForTurn(browser, 1, (t) => t.running && /tick 2/.test(t.text), "running turn");
    const [pid] = appPids();
    process.kill(pid, "SIGKILL"); // no graceful shutdown at all
    await waitPidGone(pid);
    await app.close();

    app = await launch(home, env);
    ({ browser } = app);
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    await openSession(browser, "Think slowly");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "interrupted", "interrupted");
    assert.match(t.text, /Interrupted/);
    await waitForText(browser, '[aria-label="Worker notices"]', "marked interrupted");
    // Earlier sessions and results are intact.
    await openSession(browser, "Say hello");
    await waitForTurn(browser, 2, (t) => t.outcome === "completed", "history after restart");
  });

  it("refuses work when a runtime is signed out, with login guidance", async () => {
    const { browser } = app;
    setAuth("signed-out");
    await nav(browser, "Runtimes");
    await clickButton(browser, "Re-check");
    await waitForText(browser, '[aria-label="Agent runtimes"]', "Not signed in");
    await nav(browser, "Workers");
    const radio = await browser.$('//label[.//span[normalize-space()="Codex"]]//input');
    await radio.click();
    await waitForText(browser, `${NEW_TASK} [role="note"]`, "codex login");
    const start = await browser.$('//button[normalize-space()="Start task"]');
    assert.equal(await start.isEnabled(), false);
    // Even a direct call from the webview is refused, and nothing runs.
    const result = await browser.execute(async () => {
      try {
        await window.__TAURI_INTERNALS__.invoke("start_agent_session", {
          runtimeId: "codex",
          objective: "sneak in",
          model: null,
        });
        return "ALLOWED";
      } catch (e) {
        return JSON.stringify(e);
      }
    });
    assert.match(result, /not signed in/);
    setAuth("subscription");
  });
});
