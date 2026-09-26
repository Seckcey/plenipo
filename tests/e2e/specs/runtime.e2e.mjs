// Phase 1 end-to-end: Desktop UI -> Plenipo Core -> supervised process -> events -> UI.

import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";

import {
  appPids,
  clickButton,
  DETAIL,
  launch,
  LOG,
  makeHome,
  nav,
  pidAlive,
  screenshot,
  selectedPid,
  textOf,
  waitForText,
  waitPidGone,
  waitUntil,
} from "../lib/app.mjs";

const home = makeHome();

describe("Phase 1 runtime supervisor (real app)", () => {
  let app;
  before(async () => {
    app = await launch(home);
  });
  after(async () => {
    await app?.close();
  });

  it("opens the branded shell with navigation and no errors", async () => {
    const { browser } = app;
    await waitForText(browser, ".shell__wordmark", "Plenipo");
    const labels = await browser.$$("nav button").map((b) => b.getText());
    for (const expected of ["Organization", "Runtimes", "Activity", "Settings", "Diagnostics"]) {
      assert.ok(
        labels.some((l) => l.startsWith(expected)),
        `nav has ${expected}`,
      );
    }
    assert.equal(await (await browser.$('[role="alert"]')).isExisting(), false);
  });

  it("launches a process and streams stdout and stderr live", async () => {
    const { browser } = app;
    await nav(browser, "Runtimes");
    await clickButton(browser, "Start Echo test");

    // Incremental: early lines are visible while the process is still running.
    const early = await waitUntil(async () => {
      const t = await textOf(browser, LOG);
      return t.includes("stdout line 1 of 10") ? t : null;
    }, "first stdout line");
    assert.ok(!early.includes("echo complete"), "output arrived before the process finished");

    await waitForText(browser, LOG, "echo complete");
    await waitForText(browser, DETAIL, "Succeeded");
    const log = await textOf(browser, LOG);
    assert.match(log, /greeting: hello from Plenipo/);
    const stderr = await browser.$$(`${LOG} [data-stream="stderr"]`).map((e) => e.getText());
    assert.deepEqual(stderr, ["stderr notice 3", "stderr notice 6", "stderr notice 9"]);
    await screenshot(browser, "runtimes-echo-succeeded");
  });

  it("detects an abnormal exit", async () => {
    const { browser } = app;
    await clickButton(browser, "Start Failing process");
    await waitForText(browser, DETAIL, "Failed · exit 3");
    await waitForText(browser, LOG, "error: simulated failure");
  });

  it("keeps a running process through a UI reload, then cancels it", async () => {
    const { browser } = app;
    await clickButton(browser, "Start Long-running process");
    await waitForText(browser, LOG, "heartbeat 2");
    const pid = await selectedPid(browser);
    assert.ok(pidAlive(pid));

    await browser.refresh();

    // View, selection, and live output are restored from Core; the process kept running.
    await waitForText(browser, DETAIL, "Running");
    const beat = async () =>
      Math.max(
        0,
        ...[...(await textOf(browser, LOG)).matchAll(/heartbeat (\d+)/g)].map((m) => +m[1]),
      );
    const before = await waitUntil(beat, "heartbeat after reload");
    await waitUntil(async () => (await beat()) > before + 1, "heartbeats continuing after reload");
    assert.equal(await selectedPid(browser), pid);
    assert.ok(pidAlive(pid), "reload did not affect the process");
    await screenshot(browser, "runtimes-running-after-reload");

    await clickButton(browser, "Cancel");
    await waitForText(browser, DETAIL, "Cancelled");
    await waitForText(browser, DETAIL, "Cancelled by user");
    await waitPidGone(pid);
  });

  it("does not let the webview run arbitrary commands", async (t) => {
    const { browser } = app;
    const attempts = await browser.execute(async () => {
      const invoke = window.__TAURI_INTERNALS__.invoke;
      const attempt = (cmd, args) =>
        invoke(cmd, args).then(
          () => "ALLOWED",
          (e) => (typeof e === "string" ? e : JSON.stringify(e)),
        );
      return {
        shellProfile: await attempt("start_execution", { profileId: "/bin/sh" }),
        injectedArgs: await attempt("start_execution", {
          profileId: "sh",
          executable: "/bin/sh",
          args: ["-c", "touch /tmp/pwned"],
        }),
        shellPlugin: await attempt("plugin:shell|execute", { program: "sh", args: [] }),
        fsPlugin: await attempt("plugin:fs|read_text_file", { path: "/etc/passwd" }),
        unknown: await attempt("run_command", { command: "id" }),
      };
    });
    t.diagnostic(`refusals: ${JSON.stringify(attempts)}`);
    for (const [name, result] of Object.entries(attempts)) {
      assert.notEqual(result, "ALLOWED", `${name} must be refused`);
    }
    assert.match(attempts.shellProfile, /invalid profile id/);
  });

  it("quitting terminates owned processes and history survives a restart", async () => {
    const { browser } = app;
    await clickButton(browser, "Start Long-running process");
    await waitForText(browser, LOG, "heartbeat 1");
    const child = await selectedPid(browser);
    const [appPid] = appPids();
    assert.ok(appPid, "found the Plenipo process");

    process.kill(appPid, "SIGTERM"); // same path as tray "Quit": graceful shutdown
    await waitPidGone(appPid);
    await waitPidGone(child);
    await app.close();

    // Relaunch with the same data directory.
    app = await launch(home);
    const b = app.browser;
    await waitForText(b, ".shell__wordmark", "Plenipo");
    await nav(b, "Runtimes");
    const item = await b.$('//button[contains(@aria-label, "Long-running process — Cancelled")]');
    await item.waitForExist({ timeout: 10_000 });
    await item.click();
    await waitForText(b, DETAIL, "Cancelled because Plenipo was shutting down");
    await waitForText(b, LOG, "Output from a previous Plenipo session is not retained");
    // Earlier executions are still in history.
    await screenshot(b, "runtimes-history-after-restart");
    const history = await b.$$(".executions button").map((e) => e.getAttribute("aria-label"));
    for (const label of ["Echo test — Succeeded", "Failing process — Failed · exit 3"]) {
      assert.ok(
        history.some((h) => h.startsWith(label)),
        `history has ${label}`,
      );
    }
  });
});
