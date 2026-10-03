import { useEffect, useId, useState, type ReactNode } from "react";
import type { AiToolsPage, PermissionsSnapshot, RoutingSnapshot, Switches } from "@plenipo/types";
import { Switch } from "@plenipo/ui";

import {
  getAiTools,
  getRouting,
  setAiToolsAutoUpdate,
  setRoutingOptions,
  setSwitches,
  toCommandError,
} from "../api/commands";
import { usePermissions } from "../guard/usePermissions";
import { useRun } from "../guard/useRun";
import { AUTO_UPDATE_HINT, AUTO_UPDATE_LABEL } from "./aiTools/words";

/**
 * One on/off switch: the library's switch (a button with the switch role and its name), with
 * its name, "On" or "Off", and what "off" or "on" means beside it. `name`: what a screen reader
 * calls it, when the same switch is shown more than once ("Update Codex by itself");
 * `label` otherwise.
 */
export function Toggle({
  label,
  name,
  hint,
  checked,
  disabled = false,
  onChange,
}: {
  label: string;
  name?: string;
  hint: ReactNode;
  checked: boolean;
  disabled?: boolean;
  onChange: (on: boolean) => void;
}) {
  const hintId = useId();
  return (
    <div className="toggle">
      <Switch
        label={name ?? label}
        checked={checked}
        disabled={disabled}
        showState={false}
        onChange={onChange}
        describedBy={hintId}
      />
      <div className="toggle__words">
        <span className="toggle__label">
          {label} <span className="toggle__state">{checked ? "On" : "Off"}</span>
        </span>
        <span id={hintId} className="muted">
          {hint}
        </span>
      </div>
    </div>
  );
}

type Key = keyof Switches;

/**
 * Update AI tools by themselves (ADR-059 §8): the same setting as the switch on the AI tools
 * page. Off to start with.
 */
function AiToolsUpdateSwitch() {
  const [page, setPage] = useState<AiToolsPage | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const { pending, error, run } = useRun<AiToolsPage>(setPage);
  useEffect(() => {
    let live = true;
    Promise.resolve()
      .then(getAiTools)
      .then(
        (p) => {
          if (live) setPage(p);
        },
        (reason: unknown) => {
          if (live) setLoadError(toCommandError(reason).message);
        },
      );
    return () => {
      live = false;
    };
  }, []);
  if (!page) {
    return (
      <p className={loadError ? "form-error" : "muted"}>
        {loadError
          ? `Couldn't read ${AUTO_UPDATE_LABEL}: ${loadError}`
          : `Loading ${AUTO_UPDATE_LABEL}…`}
      </p>
    );
  }
  return (
    <>
      <Toggle
        label={AUTO_UPDATE_LABEL}
        hint={AUTO_UPDATE_HINT}
        checked={page.autoUpdate}
        disabled={pending}
        onChange={(on) => void run(() => setAiToolsAutoUpdate(on))}
      />
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </>
  );
}

/** The lines work can step down from, in percent of a plan used. */
const STEP_DOWN_LINES = [70, 80, 90] as const;

/**
 * Step down instead of stopping (Phase 25, item 4.5; ADR-255): a choice kept with the AI
 * models settings, shown here with the other switches. On to start with, from 80% used.
 */
function StepDownSwitch() {
  const [snapshot, setSnapshot] = useState<RoutingSnapshot | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const { pending, error, run } = useRun<RoutingSnapshot>(setSnapshot);
  useEffect(() => {
    let live = true;
    Promise.resolve()
      .then(getRouting)
      .then(
        (s) => {
          if (live) setSnapshot(s);
        },
        (reason: unknown) => {
          if (live) setLoadError(toCommandError(reason).message);
        },
      );
    return () => {
      live = false;
    };
  }, []);
  if (!snapshot) {
    return (
      <p className={loadError ? "form-error" : "muted"}>
        {loadError ? `Couldn't read Step down: ${loadError}` : "Loading Step down…"}
      </p>
    );
  }
  const options = snapshot.options;
  return (
    <>
      <Toggle
        label="Step down instead of stopping"
        hint={`On to start with. When a plan passes ${options.stepDownAt}% used, new work runs at a lower effort; closer to the limit, on a smaller model from the same company; at the limit, on your key for the same company if paid keys are on. Each step is shown on the worker and recorded. Reviewers, and agents you set to their own model, never step down. Off: work keeps its model until the limit.`}
        checked={options.stepDown}
        disabled={pending}
        onChange={(on) => void run(() => setRoutingOptions({ ...options, stepDown: on }))}
      />
      <label className="field field--inline">
        <span>Start stepping down at</span>
        <select
          value={options.stepDownAt}
          disabled={pending || !options.stepDown}
          onChange={(e) =>
            void run(() => setRoutingOptions({ ...options, stepDownAt: Number(e.target.value) }))
          }
        >
          {(STEP_DOWN_LINES as readonly number[]).includes(options.stepDownAt) ? null : (
            <option value={options.stepDownAt}>{options.stepDownAt}% used</option>
          )}
          {STEP_DOWN_LINES.map((line) => (
            <option key={line} value={line}>
              {line}% used
            </option>
          ))}
        </select>
      </label>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </>
  );
}

/**
 * Settings → Switches (ADR-023): turn whole features on or off, and choose what workers may do on
 * your allowed websites without asking. The rules that keep you in charge have no switch.
 */
export function SwitchSettings({
  learning,
  phone,
  community,
}: {
  learning?: ReactNode;
  phone?: ReactNode;
  community?: ReactNode;
}) {
  const permissions = usePermissions();
  const { pending, error, run } = useRun((s: PermissionsSnapshot) => permissions.apply(s));
  const s = permissions.snapshot?.settings.switches;
  if (!s) {
    return (
      <p className={permissions.error ? "form-error" : "muted"}>
        {permissions.error ?? "Loading the switches…"}
      </p>
    );
  }
  const flip = (key: Key) => (on: boolean) => void run(() => setSwitches({ ...s, [key]: on }));
  return (
    <div className="switches">
      <section aria-labelledby="switches-features">
        <h3 id="switches-features">What workers may use</h3>
        <Toggle
          label="Plenipo's browser"
          hint="Off: no worker opens any website, whatever its permissions. Turning it off stops any worker using it now."
          checked={s.browser}
          disabled={pending}
          onChange={flip("browser")}
        />
        <Toggle
          label="Screen, mouse, and keyboard"
          hint="The last resort, and off to start with. On: workers whose permissions allow it can see your screen and, after asking you each time, use the mouse and keyboard."
          checked={s.desktop}
          disabled={pending}
          onChange={flip("desktop")}
        />
        <Toggle
          label="Remote computers (SSH)"
          hint="Off to start with. On: workers whose permissions allow it run commands on the servers you set up in Settings → Servers, with the rules you gave each one. Turning it off disconnects any worker using a server now."
          checked={s.servers}
          disabled={pending}
          onChange={flip("servers")}
        />
        <Toggle
          label="Let workers use paid AI keys"
          hint="Off to start with: workers use only your subscriptions. On: workers may use the paid AI keys you save on the AI tools page. A spending limit is up to you (Settings → Spending caps): without one, paid work has no dollar limit, and every paid task is still priced and listed."
          checked={s.paidAiKeys}
          disabled={pending}
          onChange={flip("paidAiKeys")}
        />
        <Toggle
          label="Let leads hire missing workers on their own"
          hint="Off to start with: when a supervisor or manager needs a job nobody in its department does, Plenipo asks you first on Home. On: it hires one on call for that team by itself."
          checked={s.hireOnItsOwn}
          disabled={pending}
          onChange={flip("hireOnItsOwn")}
        />
        {learning}
      </section>
      <section aria-labelledby="switches-websites">
        <h3 id="switches-websites">On your allowed websites, without asking you</h3>
        <p className="notice-box" role="note">
          <strong>Off means workers ask you first.</strong> When one of these is on, workers do it
          without asking, but only on the websites on your Allowed list (Settings → Permissions →
          Websites). A website could trick a worker into doing it, so turn these on only for
          websites you trust. Everything is still recorded in the Activity trail, and a sensitive
          action you set to Blocked stays blocked.
        </p>
        <Toggle
          label="Sending forms and messages"
          hint="Submitting a form, posting, or sending a message. Also email and chat through a Connection, but only when everyone it goes to is on that connection's Send without asking to list (Settings → Connections)."
          checked={s.sendWithoutAsking}
          disabled={pending}
          onChange={flip("sendWithoutAsking")}
        />
        <Toggle
          label="Buying and paying"
          hint="Buy now, Place order, Pay, Checkout."
          checked={s.buyWithoutAsking}
          disabled={pending}
          onChange={flip("buyWithoutAsking")}
        />
        <Toggle
          label="Signing in"
          hint="Pressing Sign in or Log in. Workers still never type a password: you sign in yourself in Plenipo's browser."
          checked={s.signInWithoutAsking}
          disabled={pending}
          onChange={flip("signInWithoutAsking")}
        />
      </section>
      <section aria-labelledby="switches-other">
        <h3 id="switches-other">Checks and screenshots</h3>
        <Toggle
          label="Hand me checks that a person is using a website"
          hint="CAPTCHAs. On: the worker tries the check up to 3 times; if it is still there, it shows you the page and waits while you solve it, then carries on. Off: the worker stops and tells you without trying."
          checked={s.captchaToOwner}
          disabled={pending}
          onChange={flip("captchaToOwner")}
        />
        <Toggle
          label="Screenshots in the Activity trail"
          hint="A picture of every significant step. Off: only approval cards keep a picture of the page, so you still see what you approve."
          checked={s.screenshots}
          disabled={pending}
          onChange={flip("screenshots")}
        />
      </section>
      <section aria-labelledby="switches-ai-tools">
        <h3 id="switches-ai-tools">AI tools</h3>
        <AiToolsUpdateSwitch />
        <StepDownSwitch />
      </section>
      {phone && (
        <section aria-labelledby="switches-phone">
          <h3 id="switches-phone">Your phone</h3>
          {phone}
        </section>
      )}
      {community && (
        <section aria-labelledby="switches-community">
          <h3 id="switches-community">Community</h3>
          {community}
        </section>
      )}
      <p className="muted switches__always">
        Always on, with no switch: workers never type passwords or secrets and never try a CAPTCHA
        more than 3 times; the sign shows whenever a worker uses the browser, your mouse, or a
        server; and Stop halts it at once.
      </p>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
