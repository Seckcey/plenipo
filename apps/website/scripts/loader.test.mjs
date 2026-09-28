import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { runInNewContext } from "node:vm";

const source = await readFile(new URL("../main.js", import.meta.url), "utf8");
const settle = () => new Promise((resolve) => setImmediate(resolve));

// Run the actual page script with only browser/module I/O replaced. These checks
// cover loading races and focus ownership; real layout and navigation need browser QA.
function page() {
  const elements = new Map();
  const requests = [];
  const styles = [];
  let mounts = 0;
  let unmounts = 0;
  let renderFailure;
  const document = {
    activeElement: null,
    querySelectorAll: () => [],
    querySelector: (name) => element(name),
    getElementById: (id) => (id ? element(id) : null),
    addEventListener() {},
    createElement: () => ({ remove() {} }),
    head: { append: (style) => styles.push(style) },
  };
  function element(id) {
    if (!elements.has(id)) {
      elements.set(id, {
        _hidden: id === "demo-root" || id === "stop-demo",
        get hidden() {
          return this._hidden;
        },
        set hidden(value) {
          this._hidden = value;
          // Browsers blur a focused control when it or its ancestor is hidden.
          if (
            value &&
            (document.activeElement === this ||
              (id === "demo-fallback" && document.activeElement === element("start-demo")))
          )
            document.activeElement = element("body");
        },
        set disabled(value) {
          if (value && document.activeElement === this) document.activeElement = element("body");
        },
        attributes: {},
        setAttribute(name, value) {
          this.attributes[name] = value;
        },
        getAttribute(name) {
          return this.attributes[name];
        },
        dataset: {},
        parentElement: { dataset: {} },
        listeners: {},
        addEventListener(name, callback) {
          this.listeners[name] = callback;
        },
        querySelector: (name) => element(name),
        focus() {
          document.activeElement = this;
        },
      });
    }
    return elements.get(id);
  }
  element("start-demo").dataset = {
    demoModule: "/demo/demo-test.js",
    demoStyle: "/demo/demo-test.css",
  };
  const initialFocus = element("page-link");
  document.activeElement = initialFocus;
  const importDemo = (url) =>
    new Promise((resolve, reject) => {
      requests.push({ url, resolve, reject });
    });
  assert.equal(source.split("import(moduleUrl.href)").length, 2);
  runInNewContext(source.replace("import(moduleUrl.href)", "importDemo(moduleUrl.href)"), {
    document,
    window: {
      location: { href: "https://example.test/", hash: "" },
      addEventListener() {},
      matchMedia: () => ({ addEventListener() {} }),
    },
    URL,
    importDemo,
  });
  const click = (id) => {
    element(id).focus();
    element(id).listeners.click();
  };
  const complete = async () => {
    const style = styles.at(-1);
    style.sheet = {};
    style.onload();
    requests.at(-1).resolve({
      mountDemo(_root, ready, fail) {
        mounts += 1;
        renderFailure = fail;
        ready();
        return () => (unmounts += 1);
      },
    });
    await settle();
  };
  return {
    document,
    element,
    requests,
    initialFocus,
    click,
    complete,
    mounts: () => mounts,
    unmounts: () => unmounts,
    failRender: () => renderFailure(),
  };
}

test("page entry starts one load without a click and preserves focus when ready", async () => {
  const p = page();
  assert.equal(p.requests.length, 1);
  assert.equal(p.element("start-demo").getAttribute("aria-disabled"), "true");
  assert.equal(p.element("demo-fallback").hidden, false);
  // Even a duplicate event cannot mount a second root during the pending load.
  p.element("start-demo").listeners.click();
  assert.equal(p.requests.length, 1);
  await p.complete();
  assert.equal(p.mounts(), 1);
  assert.equal(p.document.activeElement, p.initialFocus);
  assert.equal(p.element("demo-fallback").hidden, true);
  assert.equal(p.element("stop-demo").hidden, false);
});

test("returning to static stays there until explicit reactivation", async () => {
  const p = page();
  await p.complete();
  p.click("stop-demo");
  await settle();
  assert.equal(p.unmounts(), 1);
  assert.equal(p.requests.length, 1);
  assert.equal(p.element("demo-fallback").hidden, false);
  assert.equal(p.element("demo-root").hidden, true);
  p.click("start-demo");
  assert.equal(p.document.activeElement, p.element("start-demo"));
  await p.complete();
  assert.equal(p.mounts(), 2);
  assert.equal(p.document.activeElement, p.element('[role="tab"]'));
});

test("failed automatic import retains the readable fallback and retries a fresh URL", async () => {
  const p = page();
  p.requests[0].reject(new Error("network unavailable"));
  await settle();
  assert.equal(p.element("demo-fallback").hidden, false);
  assert.equal(p.element("start-demo").getAttribute("aria-disabled"), "false");
  assert.match(p.element("demo-load-status").textContent, /could not load/);
  p.click("start-demo");
  assert.match(p.requests[1].url, /demo-test\.js\?retry=1$/);
  await p.complete();
  assert.equal(p.mounts(), 1);
});

test("a slow explicit retry does not steal focus after the visitor moves elsewhere", async () => {
  const p = page();
  p.requests[0].reject(new Error("network unavailable"));
  await settle();
  p.click("start-demo");
  p.initialFocus.focus();
  await p.complete();
  assert.equal(p.document.activeElement, p.initialFocus);
});

test("render failure restores the static controls and permits a clean new root", async () => {
  const p = page();
  await p.complete();
  p.failRender();
  assert.equal(p.element("demo-fallback").hidden, false);
  assert.equal(p.element("start-demo").getAttribute("aria-disabled"), "false");
  p.click("start-demo");
  await p.complete();
  assert.equal(p.unmounts(), 1);
  assert.equal(p.mounts(), 2);
});
