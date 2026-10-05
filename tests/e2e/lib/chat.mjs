// The Workers page's chat (I4): a conversation shown beside the organization's tree, the same
// chat as the Chat panel's (ADR-200). These helpers drive it the way the owner does: start a
// conversation outside the organization, read each turn (its folds opened, so its thinking, its
// steps, and its tokens can be read), write to it, and stop it.

import { clickButton, nav, textOf, waitForText, waitUntil } from "./app.mjs";

/** The chat shown on the Workers page. */
export const CHAT = ".workers__chat";
/** Its conversation. */
export const CHAT_LOG = `${CHAT} [role="log"]`;
const START = '//form[@aria-label="Start a conversation"]';
export const START_FORM = 'form[aria-label="Start a conversation"]';

/** A conversation's title as the chat shows it (markers like "[slow]" left out). */
export const titleOf = (objective) => objective.replace(/\s*\[.*$/, "");

/** Wait until the chat shown is the one titled `title`. */
export const waitForChat = (browser, title) =>
  waitForText(browser, `${CHAT} .chat-head__title`, titleOf(title));

/**
 * Open every fold in the chat shown (thinking, a run of tool steps, a turn's tokens, a reply), so
 * all of its words can be read. Folds opened stay open while the test reads.
 */
export async function openFolds(browser) {
  await browser.execute((chat) => {
    for (const button of document.querySelectorAll(
      `${chat} button.chat-fold[aria-expanded="false"], ${chat} button.chat-turn__tokens[aria-expanded="false"]`,
    )) {
      button.click();
    }
    for (const details of document.querySelectorAll(`${chat} details:not([open])`)) {
      details.open = true;
    }
  }, CHAT);
}

/** The turns of the chat shown, in one read: how each stands, and all its words. */
export async function chatTurns(browser) {
  await openFolds(browser);
  return browser.execute(
    (log) =>
      [...document.querySelectorAll(`${log} article.chat-turn`)].map((turn) => ({
        state: turn.getAttribute("data-state"),
        outcome: turn.getAttribute("data-outcome"),
        running: turn.getAttribute("data-state") === "working",
        waiting: turn.getAttribute("data-state") === "waiting",
        text: turn.innerText.replace(/\s+/g, " ").trim(),
      })),
    CHAT_LOG,
  );
}

/** Wait until turn `n` (1 for the first) of the chat shown is as `predicate` wants. */
export const waitForTurn = (browser, n, predicate, what, timeoutMs = 30_000) =>
  waitUntil(
    async () => {
      const turn = (await chatTurns(browser))[n - 1];
      return turn && predicate(turn) ? turn : null;
    },
    what,
    timeoutMs,
  );

/**
 * Start a conversation outside the organization on `runtimeLabel` (the Workers page's "Other
 * conversations"), and wait for its chat.
 */
export async function startConversation(browser, runtimeLabel, objective, { handoffs } = {}) {
  await nav(browser, "Workers");
  await clickButton(browser, "Start a conversation outside your organization");
  const radio = await browser.$(
    `${START}//label[.//span[normalize-space()="${runtimeLabel}"]]//input`,
  );
  await radio.waitForExist({ timeout: 10_000 });
  await radio.click();
  await waitUntil(
    async () => (await textOf(browser, START_FORM)).includes("Ready"),
    `${runtimeLabel} ready`,
  );
  if (handoffs !== undefined) {
    const allow = await browser.$(`${START}//label[contains(., "Allow handoffs")]//input`);
    if ((await allow.isSelected()) !== handoffs) await allow.click();
  }
  const box = await browser.$(`${START_FORM} textarea`);
  await box.setValue(objective);
  await clickButton(browser, "Start");
  await waitForChat(browser, objective);
}

/** Show a conversation outside the organization (listed under "Other conversations"). */
export async function openConversation(browser, title) {
  await nav(browser, "Workers");
  const item = await browser.$(
    `//ul[@aria-label="Other conversations"]//button[contains(., "${title}")]`,
  );
  await item.waitForExist({ timeout: 10_000 });
  await item.click();
  await waitForChat(browser, title);
}

/** Write to the chat shown, and send it. */
export async function sendMessage(browser, text) {
  const box = await browser.$(`${CHAT} textarea`);
  await box.setValue(text);
  const send = await browser.$(`${CHAT} button.chat-composer__send`);
  await send.waitForClickable({ timeout: 10_000 });
  await send.click();
}

/** Stop what the chat shown's agent is doing. */
export async function stopChat(browser) {
  const stop = await browser.$(`${CHAT} button.chat-composer__stop`);
  await stop.waitForClickable({ timeout: 10_000 });
  await stop.click();
}

/** A conversation as Plenipo keeps it (the newest one titled `title`), read through its command. */
export async function sessionTitled(browser, title) {
  const sessions = await browser.execute(async () => {
    const overview = await window.__TAURI_INTERNALS__.invoke("get_agent_overview");
    return overview.sessions;
  });
  const found = sessions.find((s) => s.title.includes(title));
  if (!found) throw new Error(`no conversation titled "${title}"`);
  return found;
}
