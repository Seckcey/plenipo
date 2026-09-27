// Phase 12A end-to-end: the design system in the real app (ADR-029). The frame (names under the
// icons, the light/dark switch remembered after a restart, the bell); the Gallery's look in both
// themes against a checked-in snapshot of computed styles; keyboard use with visible focus; a
// 5,000-row table; the smallest window; and activity strips drawn from real Ledger events.
//
// Look snapshots: `PLENIPO_E2E_UPDATE_SNAPSHOTS=1 pnpm e2e` rewrites tests/e2e/snapshots/.

import assert from "node:assert/strict";
import { mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  installFakeTools,
  launch,
  makeHome,
  nav,
  screenshot as save,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();
const env = installFakeTools(home);
const agentDir = join(home, ".plenipo-fake-agent");
mkdirSync(agentDir, { recursive: true });
writeFileSync(join(agentDir, "auth"), "subscription");

const SNAPSHOTS = resolve(import.meta.dirname, "..", "snapshots");
const UPDATE = process.env.PLENIPO_E2E_UPDATE_SNAPSHOTS === "1";

/** The Supervisor's next answer (the stand-in AI tool follows this script). */
function answer(text) {
  rmSync(join(agentDir, "script-used"), { recursive: true, force: true });
  writeFileSync(
    join(agentDir, "script.json"),
    JSON.stringify({ "Shop Supervisor": [{ say: text }] }),
  );
}

const textOf = (browser, selector) =>
  browser.execute((s) => document.querySelector(s)?.innerText.replace(/\s+/g, " ") ?? "", selector);
const exists = (browser, selector) =>
  browser.execute((s) => document.querySelector(s) !== null, selector);
const waitForText = (browser, selector, needle, timeoutMs) =>
  waitUntil(
    async () => (await textOf(browser, selector)).includes(needle),
    `"${needle}" in ${selector}`,
    timeoutMs,
  );

/** Evidence, once the window has drawn what the test just saw. */
const screenshot = async (browser, name) => {
  await browser.pause(600);
  await save(browser, name);
};

const field = (browser, form, label, tag = "input") =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//${tag}`);

async function submit(browser, form) {
  const button = await browser.$(`${form} button[type="submit"]`);
  await button.waitForClickable({ timeout: 10_000 });
  await button.click();
  await waitUntil(async () => !(await exists(browser, form)), `${form} to close`);
}

async function selectNode(browser, title) {
  await clickButton(browser, "Fit to screen");
  await browser.pause(800);
  const node = await browser.$(`//button[@data-node-id and starts-with(@aria-label, "${title},")]`);
  await node.waitForExist({ timeout: 10_000 });
  try {
    await node.click();
  } catch {
    await browser.execute((el) => el.click(), node);
  }
  await waitForText(browser, "aside.inspector h2", title);
}

async function openGallery(browser) {
  await nav(browser, "Diagnostics");
  await clickButton(browser, "Open the gallery");
  await waitUntil(() => exists(browser, '[data-gallery="status-dot"]'), "the Gallery");
}

const theme = (browser) => browser.execute(() => document.documentElement.dataset.theme ?? "");

async function setTheme(browser, wanted) {
  if ((await theme(browser)) === wanted) return;
  await clickButton(
    browser,
    wanted === "light" ? "Switch to the light theme" : "Switch to the dark theme",
  );
  await waitUntil(async () => (await theme(browser)) === wanted, `the ${wanted} theme`);
}

/** Scroll a Gallery section to the top of the page, for a screenshot. */
async function showSection(browser, id) {
  await browser.execute((s) => {
    document.querySelector(`[data-gallery-section="${s}"]`)?.scrollIntoView({ block: "start" });
  }, id);
  await browser.pause(300);
}

// ---- Look snapshots ------------------------------------------------------------------------

/** Elements whose look is recorded, in every Gallery sample that has them. */
const PROBES = [
  ".ui-status",
  ".ui-status__mark",
  ".ui-pill",
  ".ui-badge",
  ".ui-health__track",
  ".ui-health__fill",
  ".ui-spark",
  ".ui-strip__bar",
  ".ui-strip__seg--high",
  ".ui-strip__seg--problem",
  ".ui-strip__axis",
  ".ui-strip__message",
  ".ui-card",
  ".ui-card__title",
  ".ui-card__sub",
  ".ui-card__foot",
  ".ui-button",
  ".ui-button--primary",
  ".ui-button--danger",
  ".ui-button--quiet",
  ".ui-icon-button",
  ".ui-switch__track",
  ".ui-switch__thumb",
  ".ui-check",
  ".ui-search",
  ".ui-field input",
  ".ui-field select",
  ".ui-segmented",
  '.ui-segmented button[aria-pressed="true"]',
  '.ui-tabs__tab[aria-selected="true"]',
  ".ui-tabs__tab",
  ".ui-empty",
  ".ui-error",
  ".ui-skeleton",
  ".ui-map__tile",
  ".ui-map__caption",
  ".ui-map__link",
  ".ui-table th",
  ".ui-table td",
  ".ui-table__footer",
  ".gallery__chip",
];

const STYLE = [
  "color",
  "backgroundColor",
  "borderTopColor",
  "borderLeftColor",
  "borderTopWidth",
  "borderLeftWidth",
  "borderTopLeftRadius",
  "fontSize",
  "fontWeight",
  "paddingTop",
  "paddingLeft",
  "opacity",
  "stroke",
];

/** Each Gallery sample's computed styles; colors normalized to [r, g, b, a] through a canvas.
 * Returned as JSON text: the driver cannot pass back an object this large. */
async function lookOf(browser) {
  const json = await browser.execute(
    (probes, props) => {
      const ctx = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
      const color = (value) => {
        if (!value || value === "none") return value;
        ctx.clearRect(0, 0, 1, 1);
        ctx.fillStyle = "#00000000";
        ctx.fillStyle = value;
        ctx.fillRect(0, 0, 1, 1);
        const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
        return [r, g, b, Math.round((a / 255) * 100) / 100];
      };
      const colorProps = new Set([
        "color",
        "backgroundColor",
        "borderTopColor",
        "borderLeftColor",
        "stroke",
      ]);
      const out = {};
      const record = (name, root) => {
        const entry = {};
        for (const probe of probes) {
          const el = root.matches(probe) ? root : root.querySelector(probe);
          if (!el) continue;
          const cs = getComputedStyle(el);
          entry[probe] = Object.fromEntries(
            props.map((p) => [p, colorProps.has(p) ? color(cs[p]) : cs[p]]),
          );
        }
        if (Object.keys(entry).length > 0) out[name] = entry;
      };
      for (const el of document.querySelectorAll("[data-gallery]")) {
        const name = el.getAttribute("data-gallery");
        if (name === "live-cards") continue; // real data, not a fixed sample
        record(name, el);
      }
      // The frame around the page.
      const frame = {};
      for (const [name, sel] of [
        ["page", "html"],
        ["rail", ".ui-rail"],
        ["rail-item", ".ui-rail__item"],
        ["rail-current", '.ui-rail__item[aria-current="page"]'],
        ["rail-label", ".ui-rail__label"],
        ["topbar", ".ui-topbar"],
        ["topbar-title", ".ui-topbar__title"],
        ["footer", ".shell__footer"],
      ]) {
        const el = document.querySelector(sel);
        if (!el) continue;
        const cs = getComputedStyle(el);
        frame[name] = Object.fromEntries(
          props.map((p) => [p, colorProps.has(p) ? color(cs[p]) : cs[p]]),
        );
      }
      out.frame = frame;
      return JSON.stringify(out);
    },
    PROBES,
    STYLE,
  );
  return JSON.parse(json);
}

/** Differences between two looks; colors may differ by 3 per channel (canvas rounding). */
function compareLooks(expected, actual) {
  const diffs = [];
  const near = (a, b) =>
    Array.isArray(a) && Array.isArray(b)
      ? a.length === b.length && a.every((x, i) => Math.abs(x - b[i]) <= (i === 3 ? 0.02 : 3))
      : a === b;
  for (const [name, probes] of Object.entries(expected)) {
    if (!actual[name]) {
      diffs.push(`${name}: missing`);
      continue;
    }
    for (const [probe, props] of Object.entries(probes)) {
      const got = actual[name][probe];
      if (!got) {
        diffs.push(`${name} ${probe}: missing`);
        continue;
      }
      for (const [prop, value] of Object.entries(props)) {
        if (!near(value, got[prop])) {
          diffs.push(
            `${name} ${probe} ${prop}: expected ${JSON.stringify(value)}, got ${JSON.stringify(got[prop])}`,
          );
        }
      }
    }
  }
  for (const name of Object.keys(actual)) if (!expected[name]) diffs.push(`${name}: new sample`);
  return diffs;
}

function checkLook(look, name) {
  const file = join(SNAPSHOTS, `${name}.json`);
  if (UPDATE || !existsSync(file)) {
    mkdirSync(SNAPSHOTS, { recursive: true });
    writeFileSync(file, `${JSON.stringify(look, null, 2)}\n`);
    if (!UPDATE) assert.fail(`Wrote a new look snapshot ${file}; check it in and run again.`);
    return;
  }
  const diffs = compareLooks(JSON.parse(readFileSync(file, "utf8")), look);
  assert.deepEqual(diffs, [], `The ${name} look changed:\n${diffs.slice(0, 40).join("\n")}`);
}

// ---- Tests ---------------------------------------------------------------------------------

describe("Phase 12A: design system (real app)", () => {
  let app;

  before(async () => {
    app = await launch(home, env);
    await app.browser.setWindowSize(1440, 960);
  });
  after(async () => {
    await app?.close();
  });

  it("frames every page: names under the icons, the page title, the bell, dark by default", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    assert.equal(await theme(browser), "dark");
    const labels = await browser.execute(() =>
      [...document.querySelectorAll('nav[aria-label="Main"] .ui-rail__label')].map((l) => ({
        text: l.textContent,
        shown: l.getBoundingClientRect().width > 0,
      })),
    );
    assert.deepEqual(
      labels.map((l) => l.text),
      [
        "Organization",
        "Projects",
        "Workers",
        "Approvals",
        "AI tools",
        "Activity",
        "Settings",
        "Diagnostics",
      ],
    );
    assert.ok(
      labels.every((l) => l.shown),
      "every name is shown under its icon",
    );
    // Names fit the strip (none is cut off).
    const clipped = await browser.execute(() =>
      [...document.querySelectorAll(".ui-rail__label")]
        .filter((l) => l.scrollWidth > l.clientWidth)
        .map((l) => l.textContent),
    );
    assert.deepEqual(clipped, []);
    assert.equal(await textOf(browser, ".ui-topbar__title"), "Organization");
    assert.ok(
      await exists(
        browser,
        'button[aria-label="Notifications: Nothing waiting for your approval"]',
      ),
    );
    // The base text size is 13 px (the owner's choice).
    const base = await browser.execute(() => getComputedStyle(document.documentElement).fontSize);
    assert.equal(base, "13px");
    await screenshot(browser, "frame-dark");
  });

  it("switches to light, and remembers it after a restart", async () => {
    let { browser } = app;
    await setTheme(browser, "light");
    await nav(browser, "Settings");
    await screenshot(browser, "frame-light");
    await app.close();
    app = await launch(home, env);
    browser = app.browser;
    await browser.setWindowSize(1440, 960);
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    assert.equal(await theme(browser), "light", "the theme is remembered");
    await setTheme(browser, "dark");
  });

  it("the Gallery matches its look snapshot in both themes", async () => {
    const { browser } = app;
    await openGallery(browser);
    for (const t of ["dark", "light"]) {
      await setTheme(browser, t);
      await browser.pause(400);
      checkLook(await lookOf(browser), `gallery-${t}`);
      await browser.execute(() => document.querySelector("main")?.scrollTo(0, 0));
      await screenshot(browser, `gallery-${t}-top`);
      for (const section of ["g-cards", "g-table", "g-split", "g-map", "g-notices", "g-tokens"]) {
        await showSection(browser, section);
        await screenshot(browser, `gallery-${t}-${section.slice(2)}`);
      }
    }
    await setTheme(browser, "dark");
    await clickButton(browser, "Both side by side");
    await waitUntil(() => exists(browser, '[data-theme="light"].gallery__pane'), "both themes");
    await showSection(browser, "gd-status");
    await screenshot(browser, "gallery-both");
    await clickButton(browser, "Dark");
  });

  it("status reads without color: every mark has its word", async () => {
    const { browser } = app;
    const marks = await browser.execute(() =>
      [...document.querySelectorAll(".ui-status, .ui-pill, .ui-map__tile")].map((el) => ({
        hasMark: el.querySelector(".ui-status__mark") !== null,
        word: el.textContent.trim(),
      })),
    );
    assert.ok(marks.length > 20, `found ${marks.length} status marks`);
    assert.ok(
      marks.every((m) => m.hasMark && m.word.length > 0),
      "each mark has a word",
    );
  });

  it("works from the keyboard, with a visible focus outline on the strip, filters, table, and cards", async () => {
    const { browser } = app;
    const focus = () =>
      browser.execute(() => {
        const el = document.activeElement;
        const cs = getComputedStyle(el);
        const after = getComputedStyle(el, "::after");
        return {
          label:
            el.getAttribute("aria-label") ||
            el.labels?.[0]?.textContent.trim() ||
            el.textContent.trim().slice(0, 40),
          visible: el.matches(":focus-visible"),
          outline: cs.outlineStyle !== "none" && cs.outlineWidth !== "0px",
          outlineAfter: after.outlineStyle !== "none" && after.outlineWidth !== "0px",
        };
      });
    // The strip: Tab from the top of the page reaches the first section. A marker just before
    // the app is the starting point, as when the window first opens.
    await browser.execute(() => {
      const start = document.createElement("button");
      start.id = "e2e-start";
      document.body.prepend(start);
      start.focus();
    });
    await browser.keys("Tab");
    await browser.execute(() => document.getElementById("e2e-start")?.remove());
    let f = await focus();
    assert.equal(f.label, "Organization");
    assert.ok(f.visible && f.outline, "the strip shows where focus is");
    await browser.keys("Tab");
    assert.equal((await focus()).label, "Projects");

    // The filters: type in the search box and the 5,000 rows narrow down.
    await browser.execute(() =>
      document
        .querySelector('[data-gallery-section="g-table"] .ui-facets input[type="search"]')
        ?.focus(),
    );
    await browser.keys([..."Staff"]);
    await waitForText(browser, '[data-gallery-section="g-table"] .ui-facets__count', "Showing");
    const count = await textOf(browser, '[data-gallery-section="g-table"] .ui-facets__count');
    assert.match(count, /^Showing [\d,]+ of 5,000$/);
    assert.notEqual(count, "Showing 5,000 of 5,000");
    f = await focus();
    assert.ok(f.outline || f.visible, "the search box shows focus");
    await browser.keys("Tab");
    f = await focus();
    assert.equal(f.label, "Hide filters");
    assert.ok(f.visible && f.outline);

    // The table: the Columns button, select all, then the first sortable header.
    await browser.execute(() =>
      document.querySelector('[data-gallery-section="g-table"] .ui-table__columns button')?.focus(),
    );
    await browser.keys("Tab");
    f = await focus();
    assert.match(f.label, /^Select all/);
    await browser.keys("Tab");
    f = await focus();
    assert.equal(f.label, "Name");
    assert.ok(f.visible && f.outline);
    await browser.keys("Enter");
    await waitUntil(
      () =>
        browser.execute(
          () =>
            document
              .querySelector('[data-gallery-section="g-table"] th[aria-sort]')
              ?.getAttribute("aria-sort") === "ascending",
        ),
      "the table sorted by name",
    );

    // The cards: each card's title is a stop, and the card shows the outline.
    await browser.execute(() =>
      document.querySelector('[data-gallery-section="g-grid"] .ui-card__open')?.focus(),
    );
    await browser.keys("Tab");
    f = await focus();
    assert.ok(f.outlineAfter, "the card shows where focus is");
    await browser.keys("Escape");
    await browser.execute(() =>
      document.querySelector('[data-gallery-section="g-table"] input[type="search"]')?.focus(),
    );
    await browser.execute(() => {
      const box = document.querySelector('[data-gallery-section="g-table"] input[type="search"]');
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      setter.call(box, "");
      box.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await waitForText(
      browser,
      '[data-gallery-section="g-table"] .ui-facets__count',
      "Showing 5,000 of 5,000",
    );
  });

  it("keeps a 5,000-row table responsive: scrolling and sorting draw only what is on screen", async () => {
    const { browser } = app;
    const result = await browser.executeAsync((done) => {
      const section = document.querySelector('[data-gallery-section="g-table"]');
      const scroller = section.querySelector(".ui-table__scroll");
      const frames = () =>
        new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      const started = performance.now();
      scroller.scrollTop = scroller.scrollHeight;
      scroller.dispatchEvent(new Event("scroll"));
      frames().then(async () => {
        await frames();
        const scrolled = performance.now() - started;
        const rows = section.querySelectorAll("tbody tr:not(.ui-table__spacer)").length;
        const last = section
          .querySelector("tbody tr:not(.ui-table__spacer):last-child")
          ?.getAttribute("aria-rowindex");
        const sortStart = performance.now();
        [...section.querySelectorAll("th button")]
          .find((b) => b.textContent.startsWith("Tasks"))
          ?.click();
        await frames();
        const sorted = performance.now() - sortStart;
        done({
          scrolled,
          rows,
          last,
          sorted,
          records: section.querySelector(".ui-table__records")?.textContent,
        });
      });
    });
    assert.equal(result.records, "1–5,000 of 5,000 records");
    assert.equal(result.last, "5001", "the last of 5,000 rows is drawn after scrolling to the end");
    assert.ok(result.rows < 100, `only the rows on screen are drawn (${result.rows})`);
    assert.ok(result.scrolled < 1000, `scrolling took ${Math.round(result.scrolled)} ms`);
    assert.ok(result.sorted < 1500, `sorting took ${Math.round(result.sorted)} ms`);
    await showSection(browser, "g-table");
    await screenshot(browser, "table-5000");
  });

  it("fits the smallest window (800 × 560)", async () => {
    const { browser } = app;
    await browser.setWindowSize(800, 560);
    await browser.pause(800);
    const fit = await browser.execute(() => {
      const w = window.innerWidth;
      const rect = (s) => document.querySelector(s)?.getBoundingClientRect();
      const columns = new Set(
        [...document.querySelectorAll('[data-gallery-section="g-grid"] .ui-grid__cell')].map((c) =>
          Math.round(c.getBoundingClientRect().left),
        ),
      ).size;
      return {
        width: w,
        overflow: document.documentElement.scrollWidth - w,
        shell: Math.round(rect(".ui-shell").width),
        bell: rect(".ui-bell").right <= w,
        theme: rect(".ui-theme-toggle").right <= w,
        rail: rect(".ui-rail").width,
        labels: [...document.querySelectorAll(".ui-rail__label")].every(
          (l) => l.getBoundingClientRect().width > 0,
        ),
        columns,
      };
    });
    assert.ok(fit.width <= 820, `the window is small (${fit.width} px)`);
    assert.ok(fit.overflow <= 0, `nothing spills sideways (${fit.overflow} px)`);
    assert.equal(fit.shell, fit.width);
    assert.ok(fit.bell && fit.theme, "the top bar's buttons stay on screen");
    assert.ok(fit.labels, "the strip keeps its names");
    assert.ok(fit.columns >= 1 && fit.columns <= 2, `the card grid uses ${fit.columns} column(s)`);
    await showSection(browser, "g-grid");
    await screenshot(browser, "min-window");
    await browser.setWindowSize(1440, 960);
  });

  it("draws activity strips from real Ledger events", async () => {
    const { browser } = app;
    await nav(browser, "Organization");
    await clickButton(browser, "Create a department");
    await (await field(browser, "New department", "Name")).setValue("Operations");
    await submit(browser, 'form[aria-label="New department"]');
    await clickButton(browser, "+ Project");
    await (await field(browser, "New project", "Name")).setValue("Shop");
    await submit(browser, 'form[aria-label="New project"]');
    answer("Ordered the week's stock.");
    await selectNode(browser, "Shop Supervisor");
    await (
      await browser.$('form[aria-label="Give an objective"] textarea')
    ).setValue("Order this week's stock.");
    await clickButton(browser, "Give objective");

    await openGallery(browser);
    const strip = '[data-gallery="live-cards"] article[aria-label^="Shop,"] .ui-strip__bar';
    const events = await waitUntil(
      async () => {
        const n = await browser.execute(
          (s) => Number(document.querySelector(s)?.dataset.events ?? 0),
          strip,
        );
        return n > 0 ? n : null;
      },
      "the Shop card's strip to count Ledger events",
      60_000,
    );
    assert.ok(events > 0);
    const spoken = await browser.execute(
      (s) => document.querySelector(s)?.getAttribute("aria-label"),
      strip,
    );
    assert.match(spoken, /^Shop activity\. Last 24 hours: \d+ events?/);
    assert.ok(
      await exists(browser, '[data-gallery="live-cards"] article[aria-label^="Operations,"]'),
      "the department has a card too",
    );
    // The top bar's picker lists them, and opens the project.
    const picker = await browser.$('select[aria-label="Showing"], .ui-scope select');
    await picker.selectByVisibleText("Shop");
    await waitForText(browser, ".ui-topbar__title", "Projects");
    await openGallery(browser);
    await showSection(browser, "g-live");
    await screenshot(browser, "live-cards");
  });
});
