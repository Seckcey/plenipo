// Phase 3 end-to-end: agent runtimes in the real app, driven through the UI against fake
// `claude`, `codex`, `grok`, and `kimi` CLIs (plenipo-fake-agent) that speak each provider's
// format (Grok and Kimi over ACP, ADR-015; Kimi's files through Plenipo, ADR-027).
// Real CLIs with real sign-ins are verified by the owner (see the Phase 3 checklist).

import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  appPids,
  clickButton,
  installFakeTools,
  launch,
  makeHome,
  nav,
  screenshot,
  textOf,
  waitForText,
  waitPidGone,
  waitUntil,
  waitForShell,
} from "../lib/app.mjs";
import {
  CHAT_LOG,
  START_FORM,
  openConversation,
  openFolds,
  sendMessage,
  sessionTitled,
  startConversation,
  stopChat,
  waitForTurn,
} from "../lib/chat.mjs";

const home = makeHome();
const env = installFakeTools(home);
const stateDir = join(home, ".plenipo-fake-agent");
const setAuth = (mode) => {
  mkdirSync(stateDir, { recursive: true });
  writeFileSync(join(stateDir, "auth"), mode);
};

/** The AI tool's own conversation was confirmed for it (its ID kept, ADR-014). */
async function confirmed(browser, title) {
  const session = await sessionTitled(browser, title);
  assert.ok(session.providerSessionConfirmed && session.providerSessionId, title);
}

/**
 * AI tool cards start closed (Phase 25, item 2.1): open every card shown. An opened card is
 * remembered, so it stays open while the test works in it.
 */
async function openCards(browser) {
  await browser.execute(() => {
    for (const toggle of document.querySelectorAll(
      'li[aria-label$=" AI tool"] .ui-disclosure__toggle[aria-expanded="false"]',
    )) {
      toggle.click();
    }
  });
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

  it("detects every AI tool, its version, and its subscription sign-in", async () => {
    const { browser } = app;
    await waitForShell(browser);
    await nav(browser, "AI tools");
    const cards = '[aria-label="AI tools"]';
    // Claude Code, Codex, Grok, and Kimi are connected; Ollama is found too, and connected only
    // when an Ollama service is signed in on this machine (the fake plays only its program). A
    // card says which works: its subscription, its key, or both (Phase 25, item 1.3).
    await waitUntil(
      async () =>
        (await textOf(browser, cards)).match(/(?:Subscription|API key) connected/g)?.length >= 4,
      "every AI tool ready",
    );
    await openCards(browser);
    const text = await textOf(browser, cards);
    // Each card shows the sign-in first, then the version (Phase 19).
    assert.match(text, /Claude Code[\s\S]*Signed in \(subscription\)[\s\S]*Installed 2\.1\.999/);
    assert.match(text, /Codex[\s\S]*ChatGPT sign-in[\s\S]*Installed 0\.99\.0/);
    assert.match(text, /Grok[\s\S]*xAI[\s\S]*grok\.com sign-in[\s\S]*Installed 1\.0\.99/);
    assert.match(text, /Kimi[\s\S]*Moonshot AI[\s\S]*Kimi sign-in[\s\S]*Installed 0\.34\.99/);
    assert.match(text, /Ollama[\s\S]*Installed 0\.34\.4/);
    assert.match(text, /Antigravity[\s\S]*Google[\s\S]*Google sign-in[\s\S]*Installed 1\.2\.99/);
    assert.doesNotMatch(text, /owner@example\.com/, "no account identifiers shown");
    await screenshot(browser, "agent-runtimes");
  });

  it("A1+A3+A4: launches a Codex task with live activity and a normalized result", async () => {
    const { browser } = app;
    await startConversation(browser, "Codex", "List the workspace");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Codex result");
    assert.match(t.text, /Turn 1: you said "List the workspace"\. Previous: None\./);
    // Its tokens, normalized: 20 read, 8 of them reused, and 9 written.
    assert.match(t.text, /20 read \(8 reused\) · 9 written/);
    // Its steps included Codex's command (a step line you can open).
    await waitForTurn(browser, 1, (t) => t.text.includes("bash -lc ls"), "Codex's command");
  });

  it("A2+A3+A4: launches a Claude Code task with streamed text and a normalized result", async () => {
    const { browser } = app;
    await startConversation(browser, "Claude Code", "Say hello");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Claude result");
    assert.match(t.text, /Turn 1: you said "Say hello"\. Previous: None\./);
    await confirmed(browser, "Say hello");
    // Finished conversations are not shown as working (the start response can arrive after the
    // live "finished" update for a fast turn).
    await waitUntil(
      async () =>
        !(await textOf(browser, '[aria-label="Other conversations"]')).includes("Working"),
      "no conversation shown as working",
    );
    await screenshot(browser, "worker-result");
  });

  it("shows Claude's thinking as one paragraph, not a few letters on each line", async () => {
    const { browser } = app;
    // The fake Claude Code thinks first, its thinking in eight small pieces (ADR-200).
    await startConversation(browser, "Claude Code", "Plan first [think]");
    await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Claude result with thinking");
    await openFolds(browser);
    const thinking = await waitUntil(async () => {
      const blocks = await browser.execute(
        (log) =>
          [...document.querySelectorAll(`${log} .chat-thinking__text`)].map((p) =>
            p.innerText.replace(/\s+/g, " ").trim(),
          ),
        CHAT_LOG,
      );
      return blocks.length > 0 ? blocks : null;
    }, "its thinking");
    // One paragraph from its eight pieces, and no second block for its sign that it began.
    assert.deepEqual(thinking, ["I should check the file first."]);
    await screenshot(browser, "worker-thinking");
  });

  it("launches a Grok task over ACP with a normalized result", async () => {
    const { browser } = app;
    await startConversation(browser, "Grok", "Hello Grok");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Grok result");
    assert.match(t.text, /Turn 1: you said "Hello Grok"\. Previous: None\./);
    assert.match(t.text, /30 read \(12 reused\) · 9 written/);
    await confirmed(browser, "Hello Grok");
    await screenshot(browser, "worker-result-grok");
  });

  it("launches an Antigravity task, text only, with Plenipo's own settings for it (ADR-082)", async () => {
    const { browser } = app;
    await startConversation(browser, "Antigravity", "Hello Antigravity [refused-tool] [settings]");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Antigravity result");
    assert.match(
      t.text,
      /Turn 1: you said "Hello Antigravity \[refused-tool\] \[settings\]"\. Previous: None\./,
    );
    // It ran with Plenipo's settings for it: strict permissions, paid AI credits off.
    assert.match(t.text, /"toolPermission":"strict"/);
    assert.doesNotMatch(t.text, /"useG1Credits":true/);
    assert.match(t.text, /20 read \(8 reused\) · 9 written/);
    await confirmed(browser, "Hello Antigravity");
    // The tool it asked for was refused, and that is in its chat, in plain words.
    await waitForTurn(
      browser,
      1,
      (t) => t.text.includes("they are off for Plenipo's tasks"),
      "the refusal",
    );
    await screenshot(browser, "worker-result-antigravity");
  });

  it("launches a GitHub Copilot task, text only, in Plenipo's own settings folder for it (ADR-083)", async () => {
    const { browser } = app;
    await startConversation(browser, "GitHub Copilot", "Hello Copilot [refused-tool] [settings]");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Copilot result");
    assert.match(
      t.text,
      /Turn 1: you said "Hello Copilot \[refused-tool\] \[settings\]"\. Previous: None\./,
    );
    // It ran with Plenipo's settings folder for it, never the owner's.
    assert.match(t.text, /Settings folder: .*ai-tool-homes.copilot/);
    await confirmed(browser, "Hello Copilot");
    // The tool it asked for was refused, and that is in its chat.
    await waitForTurn(browser, 1, (t) => t.text.includes("does not exist"), "the refusal");
    await screenshot(browser, "worker-result-copilot");
  });

  it("launches a Kimi task over ACP, and Plenipo refuses Kimi's own shell", async () => {
    const { browser } = app;
    await startConversation(browser, "Kimi", "Hello Kimi [own-shell]");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "completed", "Kimi result");
    assert.match(t.text, /Turn 1: you said "Hello Kimi \[own-shell\]"\. Previous: None\./);
    assert.match(t.text, /Shell answer: reject\./);
    await confirmed(browser, "Hello Kimi");
    // The refusal is in its chat, in plain words.
    await waitForTurn(
      browser,
      1,
      (t) => t.text.includes("Workers run programs with Plenipo's run_command tool"),
      "the refusal",
    );
    await screenshot(browser, "worker-result-kimi");
  });

  it("A5: resumes both sessions in the same provider session", async () => {
    const { browser } = app;
    for (const [title, first] of [
      ["Say hello", "Say hello"],
      ["List the workspace", "List the workspace"],
    ]) {
      await openConversation(browser, title);
      await sendMessage(browser, "And again?");
      const t = await waitForTurn(browser, 2, (t) => t.outcome === "completed", `${title} turn 2`);
      assert.match(
        t.text,
        new RegExp(`Turn 2: you said "And again\\?"\\. Previous: Some\\("${first}"\\)`),
      );
    }
  });

  it("A3+A6: shows live activity and cancels an active task", async () => {
    const { browser } = app;
    await startConversation(browser, "Claude Code", "Count slowly [slow]");
    // Live: streamed ticks appear while the turn is still running.
    await waitForTurn(browser, 1, (t) => t.running && /tick 3/.test(t.text), "live ticks", 20_000);
    await screenshot(browser, "worker-live");
    await stopChat(browser);
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "cancelled", "cancelled");
    assert.match(t.text, /Stopped/);
    // The conversation stays usable: it goes on after the stop.
    await sendMessage(browser, "Done counting?");
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
    assert.match(text, /Conversation /);
    assert.match(text, /Claude Code · task 1: running/);
    assert.match(text, /Claude Code · task 1: succeeded · exit 0/);
    assert.match(text, /Agent: Turn 1: you said/);
    await screenshot(browser, "worker-ledger-trail");
    // Raw provider output stays available for diagnostics.
    await nav(browser, "AI tools");
    await (
      await browser.$('//button[contains(@aria-label, "Codex · task 1 — Succeeded")]')
    ).waitForExist({ timeout: 10_000 });
  });

  it("marks a turn interrupted when Plenipo is killed mid-turn, keeping all history", async () => {
    let { browser } = app;
    await startConversation(browser, "Codex", "Think slowly [slow]");
    await waitForTurn(browser, 1, (t) => t.running && /tick 2/.test(t.text), "running turn");
    const [pid] = appPids();
    process.kill(pid, "SIGKILL"); // no graceful shutdown at all
    await waitPidGone(pid);
    await app.close();

    app = await launch(home, env);
    ({ browser } = app);
    await waitForShell(browser);
    await openConversation(browser, "Think slowly");
    const t = await waitForTurn(browser, 1, (t) => t.outcome === "interrupted", "interrupted");
    assert.match(t.text, /Stopped/);
    await waitForText(browser, '[aria-label="Worker notices"]', "marked interrupted");
    // Earlier sessions and results are intact.
    await openConversation(browser, "Say hello");
    await waitForTurn(browser, 2, (t) => t.outcome === "completed", "history after restart");
  });

  it("refuses work when a runtime is signed out, with login guidance", async () => {
    const { browser } = app;
    setAuth("signed-out");
    await nav(browser, "AI tools");
    await clickButton(browser, "Check again");
    await waitForText(browser, '[aria-label="AI tools"]', "Not signed in");
    await nav(browser, "Workers");
    await clickButton(browser, "Start a conversation outside your organization");
    const radio = await browser.$(
      '//form[@aria-label="Start a conversation"]//label[.//span[normalize-space()="Codex"]]//input',
    );
    await radio.click();
    await waitForText(browser, `${START_FORM} [role="note"]`, "codex login");
    const start = await browser.$(
      '//form[@aria-label="Start a conversation"]//button[normalize-space()="Start"]',
    );
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
