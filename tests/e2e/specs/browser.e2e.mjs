// Phase 10 end-to-end: Plenipo's browser, in the real app, against fake `claude` and `codex` CLIs
// (plenipo-fake-agent) that call Plenipo's tools over MCP through the real relay, and a
// synthetic website on 127.0.0.1 (no internet). The owner allows 127.0.0.1 and blocks
// localhost in Settings → Permissions → Websites; a Web Assistant fills in the site's contact
// form, and sending it waits for the owner's approval, with a screenshot of the page; the
// blocked website never opens; a sign on every page shows who is using the browser; Take over
// and the emergency Stop halt the worker; and the Activity trail keeps each step with its
// screenshot. The browser is a real Chrome or Chromium: the owner chooses Google Chrome in
// Settings (ADR-028) where it is installed, as on the CI runner, or PLENIPO_BROWSER names one.
// Real AI tools and Windows are checked by the owner (Phase 10 checklist).

import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import http from "node:http";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  installFakeTools,
  launch,
  makeHome,
  nav,
  screenshot as save,
  waitUntil,
  openSettings,
  waitForShell,
} from "../lib/app.mjs";

const home = makeHome();
const env = installFakeTools(home);
mkdirSync(join(home, ".plenipo-fake-agent"), { recursive: true });
writeFileSync(join(home, ".plenipo-fake-agent", "auth"), "subscription");

// ---- The synthetic website ------------------------------------------------------------------

const STYLE =
  "<style>body{font:16px system-ui;margin:40px;background:#fff}label{display:block;margin:8px 0}" +
  "input,textarea{font:inherit;width:320px}button{font:inherit;padding:6px 14px}</style>";
const page = (title, body) =>
  `<!doctype html><html><head><meta charset=utf-8><title>${title}</title>${STYLE}</head>` +
  `<body><h1>${title}</h1>${body}</body></html>`;

/** Every form the site received (the owner's approval must come first). */
const received = [];

const site = http.createServer((req, res) => {
  let body = "";
  req.on("data", (c) => (body += c));
  req.on("end", () => {
    const path = (req.url ?? "/").split("?")[0];
    let html;
    if (req.method === "POST" && path === "/send") {
      received.push(body);
      const name = new URLSearchParams(body).get("name") ?? "";
      html = page("Thank you", `<p>Thanks, ${name}. We got your message.</p>`);
    } else if (path === "/form") {
      html = page(
        "Contact us",
        '<form method=post action="/send"><label>Your name <input name=name></label>' +
          "<label>Email <input name=email type=email></label>" +
          "<label>Message <textarea name=message></textarea></label>" +
          "<button type=submit>Send message</button></form>",
      );
    } else {
      html = page(
        "Synthetic Shop",
        '<p>Welcome to the test shop.</p><a href="/form">Contact us</a>',
      );
    }
    res.writeHead(200, { "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store" });
    res.end(html);
  });
});

let port = 0;
const url = (host, path) => `http://${host}:${port}${path}`;

// ---- Helpers ---------------------------------------------------------------------------------

const DETAILS = "aside.inspector";
const CONTROL = '[aria-label="Browser, desktop, and server work"]';

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

const nodeXPath = (label) => `//button[@data-node-id and starts-with(@aria-label, "${label}")]`;

const waitForNode = (browser, label, timeoutMs = 20_000) =>
  waitUntil(
    async () =>
      (
        await browser.execute(() =>
          [...document.querySelectorAll("button[data-node-id]")].map((b) =>
            b.getAttribute("aria-label"),
          ),
        )
      ).find((l) => l.startsWith(label)) ?? null,
    `node "${label}"`,
    timeoutMs,
  );

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

/** Evidence: let the window draw what the test just saw, then save it (the WebDriver
 * screenshot can lag the page by a frame or two on a slow virtual display). */
const screenshot = async (browser, name) => {
  await browser.pause(800);
  await save(browser, name);
};

/** A screenshot shown in the app has loaded and been drawn. */
const shotShown = (browser, scope) =>
  waitUntil(
    () =>
      browser.execute((s) => {
        const img = document.querySelector(`${s} img.shot`);
        return img !== null && img.complete && img.naturalWidth > 0;
      }, scope),
    `a screenshot in ${scope}`,
  ).then(() => browser.pause(500));

/**
 * When a wait fails, print the objective's Activity trail (the worker's tool results say why a
 * browser step failed, for example that the browser could not start), then fail as before.
 */
async function explainOnFailure(browser, objective, work) {
  try {
    return await work();
  } catch (error) {
    try {
      await nav(browser, "Activity");
      const task = await browser.$(`//button[starts-with(@aria-label, "${objective}")]`);
      await task.click();
      await browser.pause(1000);
      const trees = await browser.$$('//ol[@aria-label="Delegation tree"]//button');
      if (trees.length > 1) await trees[trees.length - 1].click();
      await browser.pause(1500);
      console.error(`--- Activity trail of "${objective}" ---`);
      console.error(await textOf(browser, '[aria-label="Activity trail"]'));
    } catch (e) {
      console.error(`(could not read the Activity trail: ${e})`);
    }
    throw error;
  }
}

/** A Plenipo tool call in the fake agent's objective. */
const tool = (name, args) => `<<tool:${name} ${JSON.stringify(args)}>>`;

/** Give the Supervisor an objective that it hands to the Web Assistant. */
async function delegate(browser, objective, work) {
  await nav(browser, "Organization");
  await select(browser, "Shop Supervisor");
  const form = 'form[aria-label="Give an objective"]';
  await (
    await browser.$(`${form} textarea`)
  ).setValue(`${objective} {{handoff:role:Web Assistant|${work.join(" ")}}}`);
  await clickButton(browser, "Give objective");
}

/** Fill in the contact form and press Send (which waits for the owner). */
const contact = (name) => [
  tool("browser_open", { url: url("127.0.0.1", "/form") }),
  tool("browser_type", { ref: "e1", text: name }),
  tool("browser_type", { ref: "e2", text: "ada@example.com" }),
  tool("browser_type", { ref: "e3", text: "Please call me back." }),
  tool("browser_click", { ref: "e4" }),
];

const SEND_CARD =
  'article[aria-label^="Web Assistant wants to click the button \\"Send message\\""]';

describe("Phase 10 Plenipo's browser, control sign, Stop, and Take over (real app, fake CLIs)", () => {
  let app;

  before(async () => {
    await new Promise((done) => site.listen(0, "127.0.0.1", done));
    port = site.address().port;
    app = await launch(home, env);
    await app.browser.setWindowSize(1600, 1000);
  });
  after(async () => {
    await app?.close();
    site.close();
  });

  it("Settings → Permissions → Websites: allow the test site, block localhost", async () => {
    const { browser } = app;
    await waitForShell(browser);
    await openSettings(browser, "Permissions");
    await waitUntil(() => exists(browser, "#websites-title"), "the Websites section");
    await waitForText(browser, ".websites", "Check a website's terms before you allow it.");
    const allowed = await field(
      browser,
      "Website lists",
      "Allowed (open without asking)",
      "textarea",
    );
    await allowed.setValue("127.0.0.1");
    const blocked = await field(browser, "Website lists", "Blocked (never open)", "textarea");
    assert.match(await blocked.getValue(), /linkedin\.com/);
    await blocked.addValue("localhost");
    await clickButton(browser, "Save websites");
    await browser.pause(500);
    assert.ok(!(await exists(browser, 'form[aria-label="Website lists"] [role="alert"]')));
    // Plenipo's browser is found, with its own profile.
    const box = 'div.plenipo-browser[aria-label="Plenipo\'s browser"]';
    await waitForText(browser, box, "with its own profile");
    // The owner chooses Google Chrome (ADR-028) where it is installed; the CI runner has Edge
    // too. Each browser keeps its own profile folder.
    const chrome = await browser.execute(
      (s) => document.querySelector(`${s} select option[value="chrome"]`)?.disabled === false,
      box,
    );
    if (chrome) {
      await (await browser.$(`${box} select`)).selectByAttribute("value", "chrome");
      await waitUntil(
        async () =>
          /(Google Chrome|Chromium), with its own profile/.test(await textOf(browser, box)),
        "Google Chrome chosen",
      );
    } else {
      assert.ok(!process.env.CI, "the CI runner has Google Chrome");
    }
    await browser.execute(() =>
      document.querySelector("#websites-title")?.scrollIntoView({ block: "start" }),
    );
    await screenshot(browser, "websites-settings");
  });

  it("builds a shop team with a Web Assistant", async () => {
    const { browser } = app;
    await nav(browser, "Organization");
    await clickButton(browser, "Create a department");
    await (await field(browser, "New department", "Name")).setValue("Operations");
    await submit(browser, 'form[aria-label="New department"]');
    await waitForNode(browser, "Operations Manager, Idle");
    await clickButton(browser, "+ Project");
    await (await field(browser, "New project", "Name")).setValue("Shop");
    await submit(browser, 'form[aria-label="New project"]');
    await waitForNode(browser, "Shop Supervisor, Idle");
    await select(browser, "Shop Supervisor");
    await clickButton(browser, "Hire Web Assistant");
    await submit(browser, 'form[aria-label="Hire"]');
    await waitForNode(browser, "Web Assistant,");
    await select(browser, "Web Assistant");
    // Its role's working instructions are in its details (ADR-019).
    await waitUntil(() => exists(browser, `${DETAILS} .inspector__job`), "its instructions");
    await waitForText(browser, DETAILS, "Does tasks on websites you allow");
  });

  it("acceptance: sending a form waits for approval with a screenshot, a sign shows the worker, and a blocked website never opens", async () => {
    const { browser } = app;
    await delegate(browser, "Contact the shop", [
      ...contact("Ada Lovelace"),
      tool("browser_open", { url: url("localhost", "/") }),
    ]);
    // While the worker waits, the sign on every page says who is using the browser. This is the
    // suite's first browser start, which can be slow on a cold machine: Plenipo gives a start 30
    // seconds and, if it is not ready, a second start of 30 more (browser/mod.rs, `launch`).
    await explainOnFailure(browser, "Contact the shop", () =>
      waitForText(browser, ".banner--approval", "is waiting for your approval", 90_000),
    );
    await waitForText(browser, CONTROL, "Web Assistant is using Plenipo's browser");
    await waitForText(browser, ".shell__footer", "Web Assistant is using Plenipo's browser");
    await screenshot(browser, "control-banner");
    assert.equal(received.length, 0, "nothing is sent before the owner approves");

    await clickButton(browser, "Review");
    await waitUntil(() => exists(browser, SEND_CARD), "the approval card");
    const text = await textOf(browser, SEND_CARD);
    assert.match(text, /Use websites/);
    assert.match(text, /Sending or publishing outside this computer/);
    assert.match(text, new RegExp(`On the page http://127\\.0\\.0\\.1:${port}/form`));
    await shotShown(browser, SEND_CARD);
    await screenshot(browser, "browser-approval-card");

    await clickButton(browser, "Approve");
    await waitUntil(() => received.length === 1, "the form to reach the site", 30_000);
    assert.match(received[0], /name=Ada\+Lovelace/);
    // The blocked website was refused, and it shows.
    await waitForText(browser, '[aria-labelledby="blocked-title"]', "tried to open", 30_000);
    await waitForText(browser, '[aria-labelledby="blocked-title"]', "localhost");
    // The worker's step ends and the sign goes away.
    await waitUntil(async () => !(await exists(browser, CONTROL)), "the sign to go", 60_000);
  });

  it("the Activity trail keeps each browser step with its screenshot", async () => {
    const { browser } = app;
    await nav(browser, "Activity");
    const task = await browser.$('//button[starts-with(@aria-label, "Contact the shop")]');
    await task.waitForExist({ timeout: 10_000 });
    await task.click();
    const tree = '[aria-label="Delegation tree"]';
    await waitForText(browser, tree, "browser_open");
    await (await browser.$('(//ol[@aria-label="Delegation tree"]//button)[2]')).click();
    const trail = '[aria-label="Activity trail"]';
    await waitForText(browser, trail, "Web Assistant started using Plenipo's browser", 30_000);
    await waitForText(browser, trail, `Web Assistant: open http://127.0.0.1:${port}/form`);
    await waitForText(browser, trail, 'Web Assistant: type "Ada Lovelace" into');
    await waitForText(browser, trail, 'Waiting for your approval: click the button "Send message"');
    await waitForText(browser, trail, 'Approved: click the button "Send message"');
    await waitForText(browser, trail, "Blocked: Web Assistant tried to open");
    await waitForText(browser, trail, "Web Assistant stopped using Plenipo's browser", 30_000);
    // Each step keeps its screenshot: show the first one.
    const show = await browser.$(`${trail} button.shot__show`);
    await show.waitForExist({ timeout: 10_000 });
    await show.scrollIntoView({ block: "center" });
    await show.click();
    await shotShown(browser, trail);
    await screenshot(browser, "browser-trail");
  });

  it("Take over: the owner takes the browser and the worker stops, sending nothing", async () => {
    const { browser } = app;
    await delegate(browser, "Contact the shop again", contact("Grace Hopper"));
    await waitForText(browser, CONTROL, "Web Assistant is using Plenipo's browser", 60_000);
    await waitForText(browser, ".banner--approval", "is waiting for your approval", 60_000);
    await clickButton(browser, "Take over");
    // The sign says the owner has control, and stays until dismissed.
    await waitForText(browser, CONTROL, "You have control of the browser. Web Assistant stopped.");
    await screenshot(browser, "take-over");
    // Its waiting request was refused: nothing reached the site.
    await waitUntil(
      async () => !(await exists(browser, ".banner--approval")),
      "the request to be refused",
      30_000,
    );
    assert.equal(received.length, 1, "the second form was never sent");
    await waitForText(browser, CONTROL, "You have control of the browser.");
    await clickButton(browser, "Dismiss");
    await waitUntil(async () => !(await exists(browser, CONTROL)), "the sign to go");
    await nav(browser, "Activity");
    const trail = '[aria-label="Activity trail"]';
    await (await browser.$('//button[starts-with(@aria-label, "Contact the shop again")]')).click();
    await (await browser.$('(//ol[@aria-label="Delegation tree"]//button)[2]')).click();
    await waitForText(browser, trail, "You took over Plenipo's browser from Web Assistant", 30_000);
  });

  it("the emergency Stop halts all control until the owner allows it again", async () => {
    const { browser } = app;
    await delegate(browser, "Contact the shop a third time", contact("Alan Turing"));
    await waitForText(browser, CONTROL, "Web Assistant is using Plenipo's browser", 60_000);
    await clickButton(browser, "Stop all");
    await waitForText(browser, CONTROL, "Browser, desktop, and server work is stopped.");
    await screenshot(browser, "emergency-stop");
    await waitUntil(
      async () => !(await exists(browser, ".banner--approval")),
      "no request left waiting",
      30_000,
    );
    assert.equal(received.length, 1, "nothing more was sent");
    await clickButton(browser, "Allow again");
    await waitUntil(async () => !(await exists(browser, CONTROL)), "control allowed again", 30_000);
  });
});
