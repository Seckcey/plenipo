// Plenipo's helper in each page of its browser (Phase 10, ADR-020). It runs in an isolated world
// named "plenipo": it shares the page's document but not its scripts, so the page cannot call
// these functions or the "plenipoControl" binding, which exists only in this world.
//
// It reads the page (text and numbered controls), gives the facts Plenipo's sensitive-action
// check needs about a control, finds CAPTCHAs (their widget's checkbox is a control, and the page
// says whether the check is passed, ADR-032), draws the on-page sign that a worker is using the
// browser, and reports the owner's own clicks and key presses (the owner taking control).
(() => {
  if (globalThis.__plenipo) return;
  const refs = new Map(); // "e12" -> element
  const refOf = new WeakMap(); // element -> "e12"
  let next = 1;
  // Until when Plenipo's own input is expected (it lapses by itself).
  let actingUntil = 0;
  let overlay = null; // { host, root, state }
  let wanted = { state: "off", worker: "" };
  // The helper runs in every frame of the page. Only the top page draws the sign: it covers the
  // frames too, and a sign inside a small frame (a CAPTCHA's widget) would cover its controls
  // (ADR-032).
  const isTop = window === window.top;

  const INTERACTIVE =
    'a[href], button, input, select, textarea, summary, [role="button"], [role="link"], ' +
    '[role="checkbox"], [role="radio"], [role="tab"], [role="menuitem"], [role="option"], ' +
    '[role="switch"], [role="combobox"], [role="textbox"], [contenteditable=""], ' +
    '[contenteditable="true"], [onclick], [tabindex]:not([tabindex="-1"])';
  const CAPTCHA_FRAME =
    /recaptcha|hcaptcha|turnstile|challenges\.cloudflare\.com|arkoselabs|funcaptcha|captcha/i;
  const CAPTCHA_BOX = ".g-recaptcha, .h-captcha, .cf-turnstile, [data-sitekey], #captcha, .captcha";
  // Where a check's provider writes its answer into the page once the check is passed (outside
  // the check's own frame, so Plenipo can read it; ADR-032).
  const CAPTCHA_ANSWER =
    'textarea[name="g-recaptcha-response"], textarea[id^="g-recaptcha-response"], ' +
    'textarea[name="h-captcha-response"], input[name="cf-turnstile-response"], ' +
    'input[name="fc-token"]';
  // A check's widget (the row with its checkbox) is about 300 × 78 pixels; a puzzle it opens
  // is much taller.
  const CAPTCHA_WIDGET_HEIGHT = 200;
  const SECRET_AUTOCOMPLETE = /password|one-time-code|cc-number|cc-csc|cc-exp/i;

  const clean = (s, n) => (s || "").replace(/\s+/g, " ").trim().slice(0, n);
  const visible = (el) => {
    if (!el.isConnected) return false;
    const r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) return false;
    const s = getComputedStyle(el);
    return s.visibility !== "hidden" && s.display !== "none" && Number(s.opacity) > 0;
  };
  const labelOf = (el) => {
    const byId = el.id && document.querySelector(`label[for="${CSS.escape(el.id)}"]`);
    return (
      el.getAttribute("aria-label") ||
      (el.labels && el.labels[0] && el.labels[0].innerText) ||
      (byId && byId.innerText) ||
      el.getAttribute("title") ||
      el.getAttribute("alt") ||
      el.getAttribute("placeholder") ||
      ""
    );
  };
  const nameOf = (el) => {
    const tag = el.tagName.toLowerCase();
    let name = labelOf(el);
    if (!name && tag === "input" && /^(submit|button|reset)$/i.test(el.type)) name = el.value;
    if (!name && tag !== "input" && tag !== "textarea" && tag !== "select") name = el.innerText;
    if (!name && tag === "input" && el.type === "image") name = el.alt || "image button";
    return clean(name, 120);
  };
  const secretField = (el) =>
    el.tagName.toLowerCase() === "input" &&
    (el.type === "password" || SECRET_AUTOCOMPLETE.test(el.autocomplete || ""));
  const refFor = (el) => {
    let r = refOf.get(el);
    if (!r) {
      r = `e${next++}`;
      refOf.set(el, r);
      refs.set(r, el);
    }
    return r;
  };
  // A check's own frame (the widget, or a puzzle it opened).
  const captchaFrame = (el) =>
    el.tagName.toLowerCase() === "iframe" &&
    (CAPTCHA_FRAME.test(el.src || "") || CAPTCHA_FRAME.test(el.title || ""));
  const captchaFrames = () => Array.from(document.querySelectorAll("iframe")).filter(captchaFrame);
  // A badge that works by itself (invisible reCAPTCHA, reCAPTCHA v3): nothing to click.
  const badgeFrame = (f) => /size=invisible/i.test(f.src || "") || !!f.closest(".grecaptcha-badge");
  // The check's widget: the small frame with its checkbox, which a worker may click.
  const widgetFrame = (f) =>
    visible(f) && !badgeFrame(f) && f.getBoundingClientRect().height <= CAPTCHA_WIDGET_HEIGHT;
  // A puzzle the check opened (pictures to pick): a tall frame, shown only then.
  const puzzleFrame = (f) => visible(f) && f.getBoundingClientRect().height > CAPTCHA_WIDGET_HEIGHT;
  const providerOf = (s) =>
    /recaptcha/i.test(s)
      ? "reCAPTCHA"
      : /h-?captcha/i.test(s)
        ? "hCaptcha"
        : /turnstile|challenges\.cloudflare\.com/i.test(s)
          ? "Cloudflare Turnstile"
          : /arkoselabs|funcaptcha/i.test(s)
            ? "Arkose"
            : "CAPTCHA";
  const frameProvider = (f) => providerOf(`${f.src || ""} ${f.title || ""}`);
  // What the widget's checkbox says, by its provider.
  const checkboxName = (f) => {
    const p = frameProvider(f);
    const words = {
      reCAPTCHA: "I'm not a robot",
      hCaptcha: "I am human",
      "Cloudflare Turnstile": "Verify you are human",
    }[p];
    return `${words || "I am a person"} (${p})`;
  };
  // Where the widget's checkbox is: at the left of a normal widget (reCAPTCHA 304 × 78,
  // hCaptcha 303 × 78, Cloudflare Turnstile 300 × 65), 28 pixels in and centered; near the top
  // center of a compact one; else the middle.
  const checkboxPoint = (r) => {
    if (r.width >= 200) return [r.left + 28, r.top + r.height / 2];
    if (r.height > 100) return [r.left + r.width / 2, r.top + 36];
    return [r.left + r.width / 2, r.top + r.height / 2];
  };
  // The check is passed: its provider has written the answer into the page.
  const captchaSolved = () =>
    Array.from(document.querySelectorAll(CAPTCHA_ANSWER)).some(
      (el) => (el.value || "").trim().length > 0,
    );
  // The page's check that a person is using the site, in one look: whether one shows, who
  // makes it, whether it is passed, whether it opened a puzzle, whether it is only a badge that
  // works by itself, and the reference of its checkbox.
  const captchaState = () => {
    const frames = captchaFrames();
    const widget = frames.find(widgetFrame) || null;
    const puzzle = frames.some(puzzleFrame);
    const box = Array.from(document.querySelectorAll(CAPTCHA_BOX)).find(visible) || null;
    const text = (document.body && document.body.innerText) || "";
    const words = /i'?m not a robot|verify (that )?you are (a )?human/i.test(text.slice(0, 20000));
    const present = !!widget || puzzle || !!box || words;
    const source = widget || frames.find(puzzleFrame) || frames[0] || null;
    return {
      present,
      provider: source ? frameProvider(source) : providerOf((box && box.className) || ""),
      solved: captchaSolved(),
      challenge: puzzle,
      invisible: !present && frames.some(badgeFrame),
      checkbox: widget ? refFor(widget) : null,
    };
  };
  const inCaptcha = (el) => !!el.closest(CAPTCHA_BOX) || captchaFrame(el);
  const formFacts = (form) => {
    if (!form) return null;
    let action;
    try {
      action = new URL(form.getAttribute("action") || location.href, location.href).href;
    } catch {
      action = "";
    }
    return {
      hasPassword: !!form.querySelector('input[type="password"]'),
      method: (form.getAttribute("method") || "get").toLowerCase(),
      action,
      buttons: Array.from(
        form.querySelectorAll('button, input[type="submit"], input[type="image"]'),
      )
        .map(nameOf)
        .filter(Boolean)
        .slice(0, 6),
    };
  };
  const facts = (el) => {
    const tag = el.tagName.toLowerCase();
    const type = (el.getAttribute("type") || (tag === "button" ? "submit" : "")).toLowerCase();
    const form = el.form || el.closest("form");
    const submit =
      (tag === "button" && type === "submit" && !!form) ||
      (tag === "input" && (type === "submit" || type === "image"));
    const captcha = inCaptcha(el);
    const widget = captcha && captchaFrame(el);
    const r = el.getBoundingClientRect();
    // A click goes to a control's middle; in a check's widget, to its checkbox.
    const [x, y] = widget ? checkboxPoint(r) : [r.left + r.width / 2, r.top + r.height / 2];
    const top = document.elementFromPoint(x, y);
    return {
      found: true,
      tag,
      type,
      role: widget ? "checkbox" : el.getAttribute("role") || "",
      name: widget ? checkboxName(el) : nameOf(el),
      href: tag === "a" ? el.href || "" : "",
      submit,
      secret: secretField(el),
      password: tag === "input" && type === "password",
      autocomplete: el.autocomplete || "",
      editable:
        (tag === "input" &&
          !/^(submit|button|reset|checkbox|radio|image|file|hidden|range|color)$/.test(type)) ||
        tag === "textarea" ||
        el.isContentEditable,
      disabled: !!el.disabled || el.getAttribute("aria-disabled") === "true",
      visible: visible(el),
      captcha,
      solved: captcha && captchaSolved(),
      form: formFacts(form),
      x,
      y,
      clear: !!top && (top === el || el.contains(top) || top.contains(el)),
    };
  };

  const P = {
    // The page in words: its address, title, visible text, and numbered controls.
    read(maxChars, maxElements) {
      const elements = [];
      for (const el of document.querySelectorAll(`${INTERACTIVE}, iframe`)) {
        if (elements.length >= maxElements) break;
        if (!visible(el) || (overlay && overlay.host.contains(el))) continue;
        const tag = el.tagName.toLowerCase();
        if (tag === "iframe") {
          // Of the page's frames, only a check's widget is a control: its checkbox.
          if (!captchaFrame(el) || !widgetFrame(el)) continue;
          elements.push({
            ref: refFor(el),
            tag,
            type: "",
            role: "checkbox",
            name: checkboxName(el),
            captcha: true,
          });
          continue;
        }
        const item = {
          ref: refFor(el),
          tag,
          type: (el.getAttribute("type") || "").toLowerCase(),
          role: el.getAttribute("role") || "",
          name: nameOf(el),
        };
        if (tag === "a") item.href = el.href || "";
        if (tag === "input" || tag === "textarea" || tag === "select") {
          if (secretField(el)) item.secret = true;
          else if (el.type === "checkbox" || el.type === "radio") item.checked = !!el.checked;
          else item.value = clean(el.value, 200);
        }
        if (el.disabled) item.disabled = true;
        elements.push(item);
      }
      const text = (document.body && document.body.innerText) || "";
      const check = captchaState();
      return {
        url: location.href,
        title: document.title,
        text: text.slice(0, maxChars),
        truncated: text.length > maxChars,
        elements,
        captcha: check.present,
        captchaInfo: check,
        passwordFields: document.querySelectorAll('input[type="password"]').length,
      };
    },
    // The page's check that a person is using the site, in one look (ADR-032).
    captchaState() {
      return captchaState();
    },
    // Facts about a numbered control, scrolled into view.
    facts(ref) {
      const el = refs.get(ref);
      if (!el || !el.isConnected) return { found: false };
      el.scrollIntoView({ block: "center", inline: "center" });
      return facts(el);
    },
    // Facts about the control that has the keyboard focus.
    focused() {
      const el = document.activeElement;
      if (!el || el === document.body || el === document.documentElement) return { found: false };
      return facts(el);
    },
    // Put the keyboard focus in a control and select what it holds, so typing replaces it.
    prepareTyping(ref) {
      const el = refs.get(ref);
      if (!el || !el.isConnected) return false;
      el.focus();
      if (typeof el.select === "function") el.select();
      else if (el.isContentEditable) {
        const range = document.createRange();
        range.selectNodeContents(el);
        const sel = getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
      }
      return document.activeElement === el || el.contains(document.activeElement);
    },
    // Choose an option in a list, by its value or its words.
    choose(ref, wantedOption) {
      const el = refs.get(ref);
      if (!el || el.tagName.toLowerCase() !== "select") return { ok: false, why: "not a list" };
      const w = String(wantedOption).trim().toLowerCase();
      const option = Array.from(el.options).find(
        (o) => o.value.toLowerCase() === w || clean(o.text, 200).toLowerCase() === w,
      );
      if (!option) {
        return {
          ok: false,
          why: "no such option",
          options: Array.from(el.options)
            .map((o) => clean(o.text, 80))
            .slice(0, 40),
        };
      }
      el.value = option.value;
      el.dispatchEvent(new Event("input", { bubbles: true }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
      return { ok: true, chosen: clean(option.text, 80) };
    },
    scroll(dy) {
      window.scrollBy({ top: dy, behavior: "instant" });
      return { y: Math.round(window.scrollY), height: document.documentElement.scrollHeight };
    },
    acting(on) {
      actingUntil = on ? Date.now() + 5000 : 0;
    },
    // The on-page sign: "active" (a worker is using the browser), "owner" (the owner took
    // control), "handed" (the owner is solving a check that a person is using the site),
    // "stopped", or "off".
    sign(state, worker) {
      wanted = { state, worker: String(worker || "") };
      draw();
    },
    // Hidden while Plenipo takes a screenshot, so it never covers the page.
    signHidden(hidden) {
      if (overlay) overlay.host.style.display = hidden ? "none" : "";
    },
  };

  const draw = () => {
    if (!isTop) return;
    if (!document.documentElement) {
      setTimeout(draw, 50);
      return;
    }
    if (wanted.state === "off") {
      if (overlay) overlay.host.remove();
      overlay = null;
      return;
    }
    if (!overlay || !overlay.host.isConnected) {
      const host = document.createElement("plenipo-sign");
      host.style.cssText =
        "all: initial; position: fixed; inset: 0; z-index: 2147483647; pointer-events: none;";
      const root = host.attachShadow({ mode: "closed" });
      document.documentElement.appendChild(host);
      overlay = { host, root, state: "" };
    }
    if (overlay.state === wanted.state + wanted.worker) return;
    overlay.state = wanted.state + wanted.worker;
    const color = { active: "#1f6feb", owner: "#1a7f37", stopped: "#cf222e", handed: "#8250df" }[
      wanted.state
    ];
    const words = {
      active: `${wanted.worker || "A worker"} is using this browser for Plenipo`,
      owner: `You have control. ${wanted.worker || "The worker"} stopped.`,
      stopped: "Stopped by you in Plenipo.",
      handed: `Please solve this check yourself, then press Approve in Plenipo. ${
        wanted.worker || "The worker"
      } waits.`,
    }[wanted.state];
    overlay.root.innerHTML = `
      <style>
        .frame { position: fixed; inset: 0; border: 4px solid ${color}; pointer-events: none; }
        .pill { position: fixed; right: 12px; bottom: 12px; display: flex; gap: 8px; align-items: center;
          padding: 6px 8px 6px 12px; border-radius: 999px; background: ${color}; color: #fff;
          font: 13px/1.3 system-ui, sans-serif; box-shadow: 0 2px 8px rgba(0,0,0,.3); pointer-events: auto; }
        button { font: inherit; border: 0; border-radius: 999px; padding: 4px 10px; background: #fff;
          color: ${color}; cursor: pointer; }
      </style>
      <div class="frame"></div>
      <div class="pill" role="status"><span></span>${
        wanted.state === "active" ? '<button type="button">Take over</button>' : ""
      }</div>`;
    overlay.root.querySelector("span").textContent = words;
    const button = overlay.root.querySelector("button");
    if (button) {
      button.addEventListener("click", (e) => {
        e.stopPropagation();
        report("takeOver");
      });
    }
  };

  const report = (kind) => {
    try {
      globalThis.plenipoControl(JSON.stringify({ kind }));
    } catch {
      // The binding is missing only while the tab is being set up.
    }
  };
  // The owner's own click or key press while a worker has the browser: the owner takes over.
  const owner = (e) => {
    if (!e.isTrusted || Date.now() < actingUntil || wanted.state !== "active") return;
    if (overlay && e.composedPath().includes(overlay.host)) return;
    report("input");
  };
  addEventListener("mousedown", owner, true);
  addEventListener("keydown", owner, true);

  globalThis.__plenipo = P;
})();
