// Phase 8 end-to-end: the Development Department in the real app, against fake `claude` and
// `codex` CLIs (plenipo-fake-agent) that follow a script — one step per turn, by position title
// — and a fake `gh`. The owner sets up a Development project on the Projects page (its folder is
// a git repository with a remote server), gives the Development VP an objective, and follows
// it: the VP hands it to the Website Supervisor, whose team implements, reviews, tests, and asks
// to open a draft pull request, which waits for the owner's approval. The result shows who did
// what on which AI tool, the files, the tests, the review, the branch, and the pull request. The
// owner's own checkout is never changed; the working copy is removed and its branch stays.
// Real CLIs are verified by the owner (Phase 8 checklist). It runs on Free, with no license key
// (Phase 11A): a worker past the third waits its turn, and the flow still finishes.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";

import {
  clickButton,
  installFakeTools,
  launch,
  makeHome,
  nav,
  screenshot,
  waitUntil,
  openSettings,
  waitForShell,
} from "../lib/app.mjs";

const home = makeHome();
const env = installFakeTools(home);
const fake = join(home, ".plenipo-fake-agent");
mkdirSync(fake, { recursive: true });
writeFileSync(join(fake, "auth"), "subscription");

const git = (dir, ...args) =>
  execFileSync("git", ["-C", dir, ...args], { encoding: "utf8" }).trim();

// The Website project's folder: the owner's own git checkout on `main`, with a remote server.
const folder = join(home, "website");
mkdirSync(join(folder, "src"), { recursive: true });
writeFileSync(join(folder, "README.md"), "# Website\n");
writeFileSync(join(folder, "src", "app.txt"), "version = 1\n");
git(folder, "init", "-q", "-b", "main");
git(folder, "config", "user.name", "Plenipo E2E");
git(folder, "config", "user.email", "e2e@example.com");
git(folder, "add", "-A");
git(folder, "commit", "-q", "-m", "Start");
const origin = join(home, "origin.git");
git(home, "init", "-q", "--bare", "origin.git");
git(folder, "remote", "add", "origin", origin);
git(folder, "push", "-q", "origin", "main");

const tool = (name, args) => [name, args];
const to = (title, objective) => ({ to: `role:${title}`, objective });

// What each member of the team does, turn by turn.
writeFileSync(
  join(fake, "script.json"),
  JSON.stringify({
    "Development VP": [
      {
        say: "Handing it to the Website supervisor.",
        handoffs: [
          to(
            "Website Supervisor",
            "Implement the login page and get it ready for review: a draft pull request.",
          ),
        ],
      },
      {
        say: "Development implemented the login page: reviewed, tested, and a draft pull request is open for your review.",
      },
    ],
    "Website Supervisor": [
      { handoffs: [to("Senior Developer", "Implement src/login.txt and commit it.")] },
      {
        handoffs: [
          to("Code Reviewer", "Review the login page."),
          to("QA Engineer", "Run verify src/login.txt login."),
        ],
      },
      { handoffs: [to("Senior Developer", "Open a draft pull request for the branch.")] },
      {
        say: "The login page is implemented, reviewed, and tested; the draft pull request is open.",
      },
    ],
    "Senior Developer": [
      {
        say: "Implemented.",
        tools: [
          tool("write_file", { path: "src/login.txt", content: "login form\n" }),
          tool("git_add", { paths: ["src/login.txt"] }),
          tool("git_commit", { message: "Add the login page" }),
        ],
      },
      {
        say: "Pull request opened.",
        tools: [
          tool("github_pr_create", {
            title: "Add the login page",
            body: "Adds the login page. Reviewed and tested by the team.",
          }),
        ],
      },
    ],
    "Code Reviewer": [
      {
        say: "Good.",
        tools: [tool("read_file", { path: "src/login.txt" })],
        review: {
          verdict: "approve",
          findings: [
            { severity: "minor", file: "src/login.txt", summary: "Label the email field" },
          ],
        },
      },
    ],
    "QA Engineer": [
      {
        say: "Passes.",
        tools: [tool("run_command", { program: "verify", args: ["src/login.txt", "login"] })],
        review: { verdict: "approve", findings: [] },
      },
    ],
  }),
);

const RESULT = 'article[aria-label="Result"]';

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

function exists(browser, selector) {
  return browser.execute((s) => document.querySelector(s) !== null, selector);
}

/** Scroll the page's content so `selector` is at the top (the window itself stays put). */
function scrollTo(browser, selector) {
  return browser.execute((s) => {
    document.querySelector(s)?.scrollIntoView({ block: "start" });
    document.scrollingElement.scrollTop = 0;
  }, selector);
}

const field = (browser, form, label, tag = "input") =>
  browser.$(`//form[@aria-label="${form}"]//label[.//span[normalize-space()="${label}"]]//${tag}`);

/**
 * A stand-in for 8 West's license check (Phase 11A, ADR-115): this copy sends its check here
 * when it has a key. On Free it must receive nothing.
 */
const licenseCheck = { seen: 0, server: null };

describe("Phase 8 Development Department (real app, fake CLIs and gh)", () => {
  let app;

  before(async () => {
    licenseCheck.server = createServer((req, res) => {
      licenseCheck.seen += 1;
      res.writeHead(503).end();
    });
    await new Promise((done) => licenseCheck.server.listen(8768, "127.0.0.1", done));
    app = await launch(home, env);
    await app.browser.setWindowSize(1600, 1000);
  });
  after(async () => {
    await app?.close();
    licenseCheck.server?.close();
  });

  it("the project's test is an approved command", async () => {
    const { browser } = app;
    // On Free (Phase 11A's acceptance): the whole Development flow runs on one department, one
    // project, and three workers at a time, and this copy never contacts 8 West.
    await waitForShell(browser, undefined, { edition: "free" });
    await openSettings(browser, "Permissions");
    const approved = await field(
      browser,
      "Command lists",
      "Approved: run without asking",
      "textarea",
    );
    await approved.waitForExist({ timeout: 20_000 });
    await approved.addValue("verify *");
    await clickButton(browser, "Save command lists");
    await browser.pause(500);
    assert.ok(!(await exists(browser, 'form[aria-label="Command lists"] [role="alert"]')));
  });

  it("sets up a Development project on the Projects page", async () => {
    const { browser } = app;
    await nav(browser, "Projects");
    await waitForText(browser, ".view", "No projects yet");
    await clickButton(browser, "Set up a Development project");
    const form = "Set up a Development project";
    await (await field(browser, form, "Name")).setValue("Website");
    await (await field(browser, form, "Description", "textarea")).setValue("The company website");
    await (
      await field(browser, form, "Repository URL (optional)")
    ).setValue("https://github.com/example/website");
    await (await field(browser, form, "Project folder (optional)")).setValue(folder);
    await browser.pause(300);
    await screenshot(browser, "development-setup");
    const submit = await browser.$(`form[aria-label="${form}"] button[type="submit"]`);
    await submit.click();
    await waitUntil(
      async () => !(await exists(browser, `form[aria-label="${form}"]`)),
      "the dialog to close",
    );
    await waitForText(browser, '[aria-label="Projects"]', "Website");
    await waitForText(browser, '[aria-label="About the project"]', "Website Supervisor");
    await waitForText(
      browser,
      '[aria-label="About the project"]',
      "A new branch and working copy for each objective",
    );
    // The template's department, leads, and team are in the organization.
    await nav(browser, "Organization");
    for (const title of [
      "Development VP",
      "Website Supervisor",
      "Senior Developer",
      "Code Reviewer",
      "QA Engineer",
      "Documentation Writer",
    ]) {
      await waitUntil(
        () =>
          browser.execute(
            (t) =>
              [...document.querySelectorAll("button[data-node-id]")].some((b) =>
                b.getAttribute("aria-label")?.startsWith(`${t},`),
              ),
            title,
          ),
        `node "${title}"`,
      );
    }
    await screenshot(browser, "development-organization");
  });

  it("acceptance: Development implements a feature and gets it ready for review", async () => {
    const { browser } = app;
    await nav(browser, "Projects");
    const form = 'form[aria-label="Give an objective"]';
    await waitUntil(() => exists(browser, form), "the objective form");
    const to = await browser.$(`${form} select`);
    assert.equal(
      await browser.execute(
        (s) => document.querySelector(s)?.selectedOptions[0]?.textContent ?? "",
        `${form} select`,
      ),
      "Development VP (VP)",
    );
    assert.ok(await to.isExisting());
    await (
      await browser.$(`${form} textarea`)
    ).setValue("Implement the login page and get it ready for review.");
    await clickButton(browser, "Give objective");
    await waitForText(browser, `${form} [role="status"]`, "Objective given to Development VP");

    // Opening the pull request waits for the owner; the result says so while it waits.
    await waitForText(browser, ".banner--approval", "is waiting for your approval", 90_000);
    await waitForText(browser, ".banner--approval", "pull request");
    await waitForText(browser, RESULT, "Waiting for you", 20_000);
    await waitForText(browser, RESULT, "src/login.txt");
    await screenshot(browser, "development-waiting");

    await clickButton(browser, "Review");
    const card = 'article[aria-label^="Senior Developer wants to"]';
    await waitUntil(() => exists(browser, card), "the approval card");
    await clickButton(browser, "Approve");
    await waitForText(
      browser,
      '[aria-labelledby="waiting-title"]',
      "Nothing is waiting for your approval.",
    );

    await nav(browser, "Projects");
    await waitForText(browser, `${RESULT} .detail__header`, "Done", 90_000);
    const result = await textOf(browser, RESULT);
    // Who got it, the answer, and every task of the team.
    assert.match(result, /Given to Development VP · Website/);
    assert.match(result, /Development implemented the login page/);
    for (const who of ["Website Supervisor", "Senior Developer", "Code Reviewer", "QA Engineer"]) {
      assert.ok(result.includes(who), `${who} in the result`);
    }
    // Files, tests, review, branch, and pull request.
    assert.match(result, /src\/login\.txt \+1 −0 Committed/);
    assert.match(result, /verify src\/login\.txt login Test QA Engineer Passed/);
    assert.match(result, /Code Reviewer: Approved/);
    assert.match(result, /Label the email field/);
    assert.match(result, /plenipo\/[a-z0-9-]+ from main Pushed/);
    assert.match(
      result,
      /Pull request #1, opened by Senior Developer: https:\/\/github\.com\/example\/website\/pull\/1/,
    );
    await scrollTo(browser, RESULT);
    await screenshot(browser, "development-result");
    await scrollTo(browser, '[aria-label="Tests and programs"]');
    await screenshot(browser, "development-result-checks");

    // The owner's own checkout is unchanged; the branch is in the repository and on the server.
    assert.ok(!existsSync(join(folder, "src", "login.txt")), "the owner's checkout is unchanged");
    assert.equal(git(folder, "branch", "--show-current"), "main");
    const branch = git(folder, "branch", "--list", "plenipo/*", "--format=%(refname:short)");
    assert.match(branch, /^plenipo\/[a-z0-9-]+$/);
    assert.equal(git(folder, "show", `${branch}:src/login.txt`), "login form");
    assert.match(git(origin, "branch", "--list", branch), new RegExp(branch));
  });

  it("removes the working copy, keeping its branch", async () => {
    const { browser } = app;
    const branch = git(folder, "branch", "--list", "plenipo/*", "--format=%(refname:short)");
    const copies = git(folder, "worktree", "list", "--porcelain")
      .split("\n")
      .filter((l) => l.startsWith("worktree "))
      .map((l) => l.slice("worktree ".length));
    const copy = copies.find((p) => p !== folder);
    assert.ok(copy && existsSync(copy), `a working copy exists: ${copies.join(", ")}`);
    await scrollTo(browser, 'table[aria-label="Working copies"]');
    await screenshot(browser, "development-working-copies");
    await clickButton(browser, `Remove the working copy of ${branch}`);
    const dialog = '[role="dialog"]';
    await waitForText(browser, dialog, "stays in the repository");
    await browser.execute((s) => {
      [...document.querySelectorAll(`${s} button`)]
        .find((b) => b.textContent.trim() === "Remove")
        ?.click();
    }, dialog);
    await waitForText(
      browser,
      'table[aria-label="Working copies"]',
      "Removed; the branch stays",
      20_000,
    );
    assert.ok(!existsSync(copy), "the working copy's folder is gone");
    assert.equal(git(folder, "branch", "--list", "plenipo/*", "--format=%(refname:short)"), branch);
  });

  it("acceptance (Phase 11A): a whole Development objective on Free never contacts 8 West", () => {
    assert.equal(licenseCheck.seen, 0, "a Free copy sends nothing to 8 West");
  });
});
