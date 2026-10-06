// Phase 4 end-to-end: handoffs between workers through Plenipo Liaison, in the real app, driven
// through the UI against fake `claude` and `codex` CLIs (plenipo-fake-agent). A marker such as
// `[handoff:claude-code]` in an objective makes the fake worker end its answer with a
// plenipo-handoff block asking that runtime to review it. Real CLIs are verified by the owner
// (see the Phase 4 checklist).

import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  installFakeTools,
  launch,
  makeHome,
  nav,
  screenshot,
  textOf,
  waitForText,
  waitUntil,
  waitForShell,
} from "../lib/app.mjs";
import {
  CHAT,
  CHAT_LOG,
  openFolds,
  sessionTitled,
  startConversation,
  stopChat,
  waitForChat,
  waitForChatState,
  waitForTurn,
} from "../lib/chat.mjs";

const home = makeHome();
const env = installFakeTools(home);
mkdirSync(join(home, ".plenipo-fake-agent"), { recursive: true });
writeFileSync(join(home, ".plenipo-fake-agent", "auth"), "subscription");

/** Screenshot with the chat's turns in view. (A DOM scroll: WebKit's WebDriver rejects wheel
 * actions inside the app's scroll area.) */
async function screenshotTurns(browser, name) {
  await browser.execute((selector) => {
    document.querySelector(selector)?.scrollIntoView({ block: "start" });
  }, CHAT_LOG);
  await screenshot(browser, name);
}

/** The text of the request card for `destinationLabel` in the lead's chat (its reply opened). */
async function handoffCard(browser, destinationLabel) {
  await openFolds(browser);
  const card = await browser.$(`${CHAT} li[aria-label^="Handoff to ${destinationLabel}"]`);
  return (await card.isExisting()) ? (await card.getText()).replace(/\s+/g, " ") : "";
}

/** A task outside the organization (the Workers page), with handoffs allowed or not. */
const startTask = (browser, runtimeLabel, objective, { handoffs }) =>
  startConversation(browser, runtimeLabel, objective, { handoffs });

/** The lead's conversation may hand work to others (Liaison is on for it). */
async function handoffsAllowed(browser, title) {
  const session = await sessionTitled(browser, title);
  assert.equal(session.metadata?.liaison?.enabled, true, `handoffs allowed for ${title}`);
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
    await waitForShell(browser);
    await startTask(browser, "Codex", "Write a parser [handoff:claude-code]", { handoffs: true });
    await handoffsAllowed(browser, "Write a parser");

    // (Its step marks name the replies once Liaison's record of them is in.)
    const t = await waitForTurn(
      browser,
      1,
      (t) =>
        t.outcome === "completed" && t.text.includes("Step 2 · continued with handoff replies"),
      "Codex result",
    );
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

    // The Claude Code worker's own chat: who asked it, the request, and its answer.
    await clickButton(browser, "Open worker conversation");
    await waitForChat(browser, "Review the answer above");
    await waitForText(browser, `${CHAT} [aria-label="Request from Codex"]`, "Codex asked");
    const review = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "review");
    // Its answer carries Codex's answer as its context. The chat shows answers as Markdown, where
    // the fake's escaped quote (\") is a quote.
    assert.match(
      review.text,
      /Turn 1: you asked "Review the answer above"; context: "Turn 1: you said "Write a parser/,
    );
    // Its work comes from who asked it: watched, not messaged.
    assert.equal(await (await browser.$(`${CHAT} textarea`)).isEnabled(), false);
    await screenshotTurns(browser, "handoff-worker-session");
    await clickButton(browser, "Open Codex's conversation");
    await waitForChat(browser, "Write a parser");
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
    await waitForChatState(browser, "Waiting for its team");
    await screenshotTurns(browser, "handoff-waiting");
    await stopChat(browser);
    await waitForTurn(browser, 1, (t) => t.outcome === "cancelled", "cancelled");
    await waitUntil(
      async () => (await handoffCard(browser, "Claude Code")).includes("Cancelled"),
      "the handoff cancelled",
    );
    await clickButton(browser, "Open worker conversation");
    await waitForChat(browser, "Review the answer above");
    await waitForTurn(browser, 1, (t) => t.outcome === "cancelled", "the worker stopped");
  });

  it("a missing destination is refused and the worker is told why", async () => {
    const { browser } = app;
    await startTask(browser, "Claude Code", "Ask around [handoff:gemini]", { handoffs: true });
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "result");
    assert.match(t.text, /received 1 reply: Plenipo: rejected: Reason: missing destination/);
    const card = await waitUntil(async () => {
      const text = await handoffCard(browser, "gemini");
      return text.includes("Refused") ? text : null;
    }, "the refused handoff");
    assert.match(card, /no AI tool named "gemini"/);
    // Nothing was sent to another provider instead.
    assert.equal(
      await (
        await browser.$('//button[normalize-space()="Open worker conversation"]')
      ).isExisting(),
      false,
    );
  });
});
