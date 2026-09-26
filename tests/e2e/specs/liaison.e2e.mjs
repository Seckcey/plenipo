// Phase 4 end-to-end: handoffs between workers through Plenipo Liaison, in the real app, driven
// through the UI against fake `claude` and `codex` CLIs (plenipo-fake-agent). A marker such as
// `[handoff:claude-code]` in an objective makes the fake worker end its answer with a
// plenipo-handoff block asking that runtime to review it. Real CLIs are verified by the owner
// (see the Phase 4 checklist).

import assert from "node:assert/strict";
import { chmodSync, copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  launch,
  makeHome,
  nav,
  screenshot,
  textOf,
  waitForText,
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
mkdirSync(join(home, ".plenipo-fake-agent"), { recursive: true });
writeFileSync(join(home, ".plenipo-fake-agent", "auth"), "subscription");

const TURNS = '[aria-label="Turns"]';
const NEW_TASK = 'form[aria-label="New task"]';
const HEADER = ".detail__header";

/** Snapshot the turns of the selected session in one in-page read. */
function turns(browser) {
  return browser.execute((selector) => {
    return [...document.querySelectorAll(`${selector} > li`)].map((li) => ({
      running: li.getAttribute("data-running") === "true",
      waiting: li.getAttribute("data-waiting") === "true",
      outcome: li.getAttribute("data-outcome"),
      text: li.innerText.replace(/\s+/g, " ").trim(),
    }));
  }, TURNS);
}

/** Screenshot with the selected session's turns in view. (A DOM scroll: WebKit's WebDriver
 * rejects wheel actions inside the app's scroll area.) */
async function screenshotTurns(browser, name) {
  await browser.execute((selector) => {
    document.querySelector(selector)?.scrollIntoView({ block: "start" });
  }, TURNS);
  await screenshot(browser, name);
}

const waitForTurn = (browser, n, predicate, what, timeoutMs = 45_000) =>
  waitUntil(
    async () => {
      const t = (await turns(browser))[n - 1];
      return t && predicate(t) ? t : null;
    },
    what,
    timeoutMs,
  );

/** The text of the handoff card for `destinationLabel` in the selected session. */
async function handoffCard(browser, destinationLabel) {
  const card = await browser.$(`li[aria-label^="Handoff to ${destinationLabel}"]`);
  return (await card.isExisting()) ? (await card.getText()).replace(/\s+/g, " ") : "";
}

async function startTask(browser, runtimeLabel, objective, { handoffs }) {
  await nav(browser, "Workers");
  const radio = await browser.$(`//label[.//span[normalize-space()="${runtimeLabel}"]]//input`);
  await radio.waitForExist({ timeout: 10_000 });
  await radio.click();
  await waitUntil(
    async () => (await textOf(browser, NEW_TASK)).includes("Ready"),
    `${runtimeLabel} ready`,
  );
  const allow = await browser.$('//label[contains(., "Allow handoffs")]//input');
  if ((await allow.isSelected()) !== handoffs) await allow.click();
  const box = await browser.$(`${NEW_TASK} textarea`);
  await box.setValue(objective);
  await clickButton(browser, "Start task");
  await waitForText(browser, `${HEADER} h2`, objective);
}

describe("Phase 4 Liaison handoffs (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    app = await launch(home, env);
  });
  after(async () => {
    await app?.close();
  });

  it("A1: a Codex worker gets a Claude Code review through Liaison and continues with it", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    await startTask(browser, "Codex", "Write a parser [handoff:claude-code]", { handoffs: true });
    await waitForText(browser, HEADER, "Handoffs allowed");

    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Codex result");
    // The review came back into the originating Codex workflow as a second step.
    assert.match(
      t.text,
      /Turn 2: received 1 reply: Claude Code: completed: Turn 1: you asked "Review the answer above"/,
    );
    assert.match(t.text, /Step 1/);
    assert.match(t.text, /Step 2 · continued with handoff replies/);
    const card = await waitUntil(async () => {
      const text = await handoffCard(browser, "Claude Code");
      return text.includes("Answered") ? text : null;
    }, "the answered handoff");
    // (WebKit's element text runs the card's cells together.)
    for (const part of [
      "→ Claude Code",
      "Review the answer above",
      "Context: The requester's answer",
    ]) {
      assert.ok(card.includes(part), `${part} in ${card}`);
    }
    await screenshotTurns(browser, "handoff-codex-to-claude");

    // The Claude Code worker's own session: the request it was started for, and its answer.
    await clickButton(browser, "Open worker session");
    await waitForText(browser, HEADER, "Handoff worker");
    await waitForText(browser, '[aria-label="Handoff request"]', "Asked by Codex");
    const review = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "review");
    assert.match(
      review.text,
      /Turn 1: you asked "Review the answer above"; context: "Turn 1: you said \\"Write a parser/,
    );
    assert.equal(
      await (await browser.$('form[aria-label="Continue session"]')).isExisting(),
      false,
    );
    await screenshotTurns(browser, "handoff-worker-session");
    await clickButton(browser, "Open requester session");
    await waitForText(browser, `${HEADER} h2`, "Write a parser");
  });

  it("A1: the Ledger holds the complete trail and the delegation tree", async () => {
    const { browser } = app;
    await nav(browser, "Activity");
    const task = await browser.$(
      '//button[starts-with(@aria-label, "Write a parser [handoff:claude-code] — Succeeded")]',
    );
    await task.waitForExist({ timeout: 10_000 });
    await task.click();
    const tree = '[aria-label="Delegation tree"]';
    await waitForText(browser, tree, "Review the answer above");
    assert.match(await textOf(browser, tree), /Claude Code · reply: Completed/);
    const trail = '[aria-label="Activity trail"]';
    await waitForText(browser, trail, "Continued with 1 handoff reply");
    const text = await textOf(browser, trail);
    for (const line of [
      /Task created: Write a parser/,
      /Handoff requested → claude-code: Review the answer above/,
      /Sub-task created: Review the answer above/,
      /Running → Blocked \(waiting for 1 handoff reply\)/,
      /Reply received: Completed/,
      /Continued with 1 handoff reply/,
      /Blocked → Running \(delivering 1 handoff reply\)/,
      /Result: Completed — Turn 2: received 1 reply/,
    ]) {
      assert.match(text, line);
    }
    await screenshot(browser, "handoff-ledger-trail");

    // The child's own trail: received, dispatched, answered, replied.
    await (
      await browser.$(
        '//ol[@aria-label="Delegation tree"]//button[normalize-space()="Review the answer above"]',
      )
    ).click();
    await waitForText(browser, trail, "Reply sent: Completed");
    const child = await textOf(browser, trail);
    assert.match(child, /Received as a handoff \(depth 1\): Review the answer above/);
    assert.match(child, /Handoff worker started/);
  });

  it("A2: the reverse path — a Claude Code worker gets a Codex review", async () => {
    const { browser } = app;
    await startTask(browser, "Claude Code", "Plan the release [handoff:codex]", { handoffs: true });
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Claude result");
    assert.match(
      t.text,
      /Turn 2: received 1 reply: Codex: completed: Turn 1: you asked "Review the answer above"/,
    );
    await waitUntil(
      async () => (await handoffCard(browser, "Codex")).includes("Answered"),
      "the answered handoff",
    );
    await screenshotTurns(browser, "handoff-claude-to-codex");
  });

  it("cancelling a waiting task stops the handoff it waits for", async () => {
    const { browser } = app;
    await startTask(browser, "Codex", "Build it [handoff:claude-code+slow]", { handoffs: true });
    await waitForTurn(browser, 1, (t) => t.waiting, "the turn waiting for its handoff");
    await waitUntil(
      async () => (await handoffCard(browser, "Claude Code")).includes("Worker running"),
      "the handoff worker running",
    );
    await waitForText(browser, '[role="status"]', "waiting for replies to its handoffs");
    await screenshotTurns(browser, "handoff-waiting");
    await clickButton(browser, "Cancel turn");
    await waitForTurn(browser, 1, (t) => t.outcome === "cancelled", "cancelled");
    await waitUntil(
      async () => (await handoffCard(browser, "Claude Code")).includes("Cancelled"),
      "the handoff cancelled",
    );
    await clickButton(browser, "Open worker session");
    await waitForText(browser, HEADER, "Handoff worker");
    await waitForTurn(browser, 1, (t) => t.outcome === "cancelled", "the worker stopped");
  });

  it("a missing destination is refused and the worker is told why", async () => {
    const { browser } = app;
    await startTask(browser, "Claude Code", "Ask around [handoff:gemini]", { handoffs: true });
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "result");
    assert.match(t.text, /received 1 reply: Plenipo: rejected: Reason: missing destination/);
    const card = await handoffCard(browser, "gemini");
    assert.match(card, /Refused/);
    assert.match(card, /no AI tool named "gemini"/);
    // Nothing was sent to another provider instead.
    assert.equal(
      await (await browser.$('//button[normalize-space()="Open worker session"]')).isExisting(),
      false,
    );
  });
});
