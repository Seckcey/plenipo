// The desktop app's Content Security Policy (apps/desktop/src-tauri/tauri.conf.json) and what
// the app's pages may rely on. Scripts come only from the app's own files, and `style-src`
// allows the app's own files only. Inline styles are allowed by name, for one reason: the
// terminal (xterm.js) makes its own `<style>` elements (`style-src-elem`) and sets `style`
// attributes for true colors and contrast fixes (`style-src-attr`). The app's own code needs
// neither: React sets styles through the style object, which the policy does not restrict. When
// the terminal stops needing them, the last test here fails, and the policy can drop them.
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(join(root, p), "utf8");

/** The policy as a map: directive -> its sources. */
function directives(csp) {
  const out = new Map();
  for (const part of csp.split(";")) {
    const [name, ...sources] = part.trim().split(/\s+/);
    if (name) out.set(name, sources);
  }
  return out;
}

/** Every file under `dir` whose name ends with one of `suffixes`. */
function filesUnder(dir, suffixes) {
  const found = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) found.push(...filesUnder(path, suffixes));
    else if (suffixes.some((s) => name.endsWith(s))) found.push(path);
  }
  return found;
}

const policy = () =>
  directives(JSON.parse(read("apps/desktop/src-tauri/tauri.conf.json")).app.security.csp);

test("the desktop app's policy allows scripts and styles from the app's own files only", () => {
  const csp = policy();
  assert.deepEqual(csp.get("default-src"), ["'self'"]);
  assert.deepEqual(csp.get("script-src"), ["'self'"]);
  assert.deepEqual(csp.get("style-src"), ["'self'"], "style-src allows the app's own files only");
  for (const [name, sources] of csp) {
    assert.ok(!sources.includes("'unsafe-eval'"), `${name} allows 'unsafe-eval'`);
  }
});

test("inline styles are allowed only where the terminal needs them", () => {
  const csp = policy();
  assert.deepEqual(csp.get("style-src-elem"), ["'self'", "'unsafe-inline'"]);
  assert.deepEqual(csp.get("style-src-attr"), ["'unsafe-inline'"]);
  for (const [name, sources] of csp) {
    if (name !== "style-src-elem" && name !== "style-src-attr") {
      assert.ok(!sources.includes("'unsafe-inline'"), `${name} allows 'unsafe-inline'`);
    }
  }
});

test("the app's own code sets no inline styles, <style> elements, or raw HTML", () => {
  const files = [
    join(root, "apps/desktop/index.html"),
    ...filesUnder(join(root, "apps/desktop/src"), [".tsx", ".ts", ".html"]),
    ...filesUnder(join(root, "packages/ui/src"), [".tsx", ".ts", ".html"]),
  ].filter((f) => !/\.test\.tsx?$|[\\/]test[\\/]|test-setup\.ts$/.test(f));
  assert.ok(files.length > 20, "the app's files were found");
  for (const file of files) {
    const text = readFileSync(file, "utf8");
    assert.ok(!/\sstyle=["'`]/.test(text), `${file}: an inline style attribute`);
    assert.ok(!/setAttribute\(["'`]style["'`]/.test(text), `${file}: sets a style attribute`);
    assert.ok(!/dangerouslySetInnerHTML|\.innerHTML\s*=/.test(text), `${file}: raw HTML`);
    assert.ok(!/<style[\s>]/.test(text), `${file}: an inline <style> element`);
  }
});

test("the terminal still needs the inline styles the policy allows", () => {
  const require = createRequire(join(root, "apps/desktop/package.json"));
  const xterm = readFileSync(require.resolve("@xterm/xterm"), "utf8");
  assert.ok(
    xterm.includes('createElement("style")'),
    "xterm.js no longer makes <style> elements: drop 'unsafe-inline' from style-src-elem",
  );
  assert.ok(
    xterm.includes('setAttribute("style"'),
    "xterm.js no longer sets style attributes: drop style-src-attr",
  );
});
