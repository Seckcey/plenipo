import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test as nodeTest } from "node:test";

function test(name, fn) {
  nodeTest(name, { skip: process.platform !== "linux" && "Coastline's updater runs on Linux" }, fn);
}

const updater = fileURLToPath(new URL("../deploy/auto-release.sh", import.meta.url));
const notes = "# Release notes\n\nA published Windows release.\n";
const mock = `#!/usr/bin/env node
const fs = require("node:fs");
const path = require("node:path");
const args = process.argv.slice(2);
const settings = JSON.parse(fs.readFileSync(process.env.UPDATE_FIXTURE));
const statePath = process.env.UPDATE_FIXTURE + ".state";
const state = fs.existsSync(statePath) ? JSON.parse(fs.readFileSync(statePath)) : {image: settings.previousImage};
const tool = path.basename(process.argv[1]);
fs.appendFileSync(process.env.UPDATE_FIXTURE + ".calls", JSON.stringify({tool, args, image: process.env.PLENIPO_IMAGE}) + "\\n");
const output = value => process.stdout.write(typeof value === "string" ? value : JSON.stringify(value));
if (tool === "id") output("1000\\n");
else if (tool === "docker") {
  if (args.includes("version")) output("Docker Compose fixture\\n");
  else if (args.includes("build")) {}
  else if (args.includes("up")) {
    state.image = process.env.PLENIPO_IMAGE;
    fs.writeFileSync(statePath, JSON.stringify(state));
  } else if (args[0] === "ps") output("live-web\\n");
  // The systemd service has no Compose working directory or implicit config.
  else if (args.includes("ps")) process.exit(2);
  else if (args[0] === "port") output("127.0.0.1:14380\\n");
  else if (args.includes("{{.Config.Image}}")) output(state.image + "\\n");
  else if (args.includes("{{.RestartCount}}")) output("0\\n");
  else if (args.includes("inspect")) output("sha256:fixture-image\\n");
  else process.exit(2);
} else if (tool === "curl") {
  const url = args.find(arg => /^https?:/.test(arg));
  if (url.includes("/releases/download/")) {}
  else if (url.includes("/releases/")) output({tag_name: "v1.2.3", draft: false, prerelease: false,
    assets: [{name: "Plenipo_1.2.3_x64-setup.exe", state: "uploaded"}]});
  else if (url.includes("/actions/workflows/")) {
    const workflow = url.includes("/ci.yml/") ? "ci" : "website";
    const check = settings[workflow] || {status: "completed", conclusion: "success"};
    if (check.error) process.exit(22);
    output({workflow_runs: check.missing ? [] : [{head_sha: check.head || new URL(url).searchParams.get("head_sha"), ...check}]});
  } else if (url.endsWith("/release.json")) {
    output({version: settings.version || "1.2.3", sourceRevision: state.image === settings.previousImage ? settings.previous : settings.revision, releaseNotes: true});
  } else if (url.endsWith("/no-such-page")) output("404");
  else if (url.endsWith("/healthz")) output("ok");
  else if (/\\/(terms|privacy)\\/$/.test(url)) {
    const policy = url.endsWith("/terms/") ? "terms" : "privacy";
    if (settings.badPolicy === policy) process.exit(22);
    if (settings.wrongPolicy === policy) { output("<h1>Wrong page</h1>"); process.exit(0); }
    const title = policy === "terms" ? "Terms of service" : "Privacy statement";
    output('<link rel="canonical" href="https://plenipo.8westit.com/' + policy + '/"><h1>' + title + '</h1>');
  } else if (url.endsWith("/")) {
    output('<link rel="canonical" href="https://plenipo.8westit.com/"><h1 id="hero-title">Plenipo</h1><section id="whats-new">What&rsquo;s new in v1.2.3<');
  } else process.exit(2);
} else process.exit(2);
`;

async function fixture(t, change = "website", options = {}) {
  const root = await mkdtemp(join(tmpdir(), "plenipo-auto-release-"));
  t.after(async () => {
    // Real updater exports are immutable; unlock only this fixture for removal.
    const result = spawnSync("chmod", ["-R", "u+w", root], { encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    await rm(root, { recursive: true, force: true });
  });
  const repo = join(root, "repo");
  const app = join(root, "app");
  const bin = join(root, "bin");
  for (const directory of [repo, app, bin]) await mkdir(directory);
  function git(...args) {
    const result = spawnSync("git", args, { cwd: repo, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  }
  async function file(path, content) {
    await mkdir(join(repo, path, ".."), { recursive: true });
    await writeFile(join(repo, path), content);
  }
  git("init", "--initial-branch=main");
  git("config", "user.name", "Updater test");
  git("config", "user.email", "updater@example.test");
  await file("apps/website/index.html", "Old site\n");
  await file("apps/website/Dockerfile", "FROM scratch\n");
  await file("apps/website/compose.yaml", "services: {}\n");
  await file("apps/website/compose.coastline.yaml", "networks: {}\n");
  await file("apps/website/release-notes/.gitkeep", "");
  await file("docs/releases/v1.2.3.md", notes);
  await file(".github/workflows/website.yml", "name: Website\n");
  await file(".github/workflows/ci.yml", "name: CI\n");
  git("add", ".");
  git("commit", "-m", "Previous website");
  git("tag", "v1.2.3");
  const previous = git("rev-parse", "HEAD");
  if (change === "website") {
    await file("apps/website/index.html", "New site\n");
    await file("apps/website/legal/terms.md", "# Terms of service\n");
    await file("apps/website/legal/privacy.md", "# Privacy statement\n");
  } else if (change === "notes")
    await file("docs/releases/v1.2.3.md", notes + "\nCorrected notes.\n");
  else if (change === "workflow")
    await file(".github/workflows/website.yml", "name: Updated Website\n");
  else await file("crates/example.rs", "// Unrelated app change\n");
  git("add", ".");
  git("commit", "-m", `Change ${change}`);
  const revision = git("rev-parse", "HEAD");
  await mkdir(join(app, "auto-release"));
  const previousImage = `plenipo-website:${previous}-v1.2.3`;
  await writeFile(
    join(app, "auto-release/current.env"),
    `NOTES_SHA256=${createHash("sha256").update(notes).digest("hex")}\nIMAGE=${previousImage}\n`,
  );
  const settingsPath = join(root, "settings.json");
  await writeFile(settingsPath, JSON.stringify({ previous, revision, previousImage, ...options }));
  for (const tool of ["id", "curl", "docker"]) {
    await writeFile(join(bin, tool), mock);
    await chmod(join(bin, tool), 0o755);
  }
  const allocationLock = join(root, "allocations.lock");
  await writeFile(allocationLock, "");
  return {
    app,
    previous,
    revision,
    run(...args) {
      return spawnSync("bash", [updater, ...args], {
        encoding: "utf8",
        timeout: 30_000,
        env: {
          ...process.env,
          PATH: `${bin}:${process.env.PATH}`,
          APP_DIR: app,
          REPO_URL: repo,
          REPO_API: "https://example.test/repo",
          SOURCE_REF: "main",
          PROJECT: "plenipo-fixture",
          PORT: "14380",
          SUBNET: "10.204.229.0/28",
          ALLOCATION_LOCK: allocationLock,
          UPDATE_FIXTURE: settingsPath,
        },
      });
    },
    async calls() {
      return (await readFile(settingsPath + ".calls", "utf8"))
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line));
    },
    async history() {
      return JSON.parse((await readFile(join(app, "auto-release/history.jsonl"), "utf8")).trim());
    },
  };
}

function noSwap(calls) {
  assert.ok(
    !calls.some(
      (call) => call.tool === "docker" && (call.args.includes("build") || call.args.includes("up")),
    ),
  );
}

for (const change of ["website", "workflow"]) {
  test(`${change} changes plan a deploy without changing the published version or notes`, async (t) => {
    const site = await fixture(t, change);
    const result = site.run("--check");
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /website files changed/);
    assert.match(
      result.stdout,
      new RegExp(`Plan: website code main at ${site.revision}, version 1.2.3`),
    );
    noSwap(await site.calls());
  });
}

test("unrelated app changes leave the running site and its source untouched", async (t) => {
  const site = await fixture(t, "app");
  const result = site.run();
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Up to date/);
  const calls = await site.calls();
  noSwap(calls);
  assert.ok(!calls.some((call) => call.args.some((arg) => arg.includes("/actions/"))));
});

test("corrected notes use current CI and the earlier checks for identical website files", async (t) => {
  const site = await fixture(t, "notes");
  const result = site.run("--check");
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /notes for v1.2.3 changed/);
  const calls = await site.calls();
  assert.ok(
    calls.some((call) =>
      call.args.some((arg) => arg.includes(`/ci.yml/runs?head_sha=${site.revision}`)),
    ),
  );
  assert.ok(
    calls.some((call) =>
      call.args.some((arg) => arg.includes(`/website.yml/runs?head_sha=${site.previous}`)),
    ),
  );
  noSwap(calls);
});

for (const ci of [
  { status: "queued" },
  { status: "in_progress" },
  { status: "completed", conclusion: "failure" },
  { status: "completed", conclusion: "cancelled" },
  { missing: true },
  { status: "completed", conclusion: "success", head: "wrong-source" },
]) {
  test(`CI ${JSON.stringify(ci)} keeps the running site`, async (t) => {
    const site = await fixture(t, "website", { ci });
    const result = site.run();
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Waiting for passing ci.yml checks/);
    noSwap(await site.calls());
  });
}

test("a failed website check and --force cannot bypass approval", async (t) => {
  const site = await fixture(t, "website", {
    website: { status: "completed", conclusion: "failure" },
  });
  const result = site.run("--force");
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Waiting for passing website.yml checks/);
  noSwap(await site.calls());
});

test("a GitHub API error fails before a build or container swap", async (t) => {
  const site = await fixture(t, "website", { ci: { error: true } });
  const result = site.run();
  assert.equal(result.status, 1);
  assert.match(result.stdout, /GitHub did not answer/);
  noSwap(await site.calls());
});

test("a checked website deploy records its source and verifies both policies", async (t) => {
  const site = await fixture(t);
  const result = site.run();
  assert.equal(result.status, 0, result.stderr + result.stdout);
  const history = await site.history();
  assert.equal(history.result, "deployed");
  assert.equal(history.revision, site.revision);
  assert.equal(history.version, "1.2.3");
  assert.match(history.websiteFingerprint, /^[a-f0-9]{64}$/);
  const calls = await site.calls();
  for (const lookup of calls.filter((call) => call.tool === "docker" && call.args[0] === "ps")) {
    assert.ok(lookup.args.includes("label=com.docker.compose.project=plenipo-fixture"));
    assert.ok(lookup.args.includes("label=com.docker.compose.service=web"));
  }
  for (const page of ["terms", "privacy"]) {
    assert.ok(calls.some((call) => call.args.includes(`http://127.0.0.1:14380/${page}/`)));
  }
});

for (const [options, message] of [
  [{ badPolicy: "privacy" }, /privacy page failed/],
  [{ wrongPolicy: "privacy" }, /privacy page has the wrong identity/],
])
  test(`an invalid policy (${Object.keys(options)[0]}) rolls back and leaves the previous record`, async (t) => {
    const site = await fixture(t, "website", options);
    const before = await readFile(join(site.app, "auto-release/current.env"), "utf8");
    const result = site.run();
    assert.equal(result.status, 1);
    assert.match(result.stdout, message);
    assert.match(result.stdout, /Went back to/);
    assert.equal((await site.history()).result, "rolled-back");
    assert.equal(await readFile(join(site.app, "auto-release/current.env"), "utf8"), before);
    const swaps = (await site.calls()).filter(
      (call) => call.tool === "docker" && call.args.includes("up"),
    );
    assert.equal(swaps.length, 2);
    assert.match(swaps[0].image, new RegExp(site.revision));
    assert.match(swaps[1].image, new RegExp(site.previous));
  });
