import type { ReactNode } from "react";
import type { PermissionsSnapshot, Switches } from "@plenipo/types";

import { setSwitches } from "../api/commands";
import { usePermissions } from "../guard/usePermissions";
import { useRun } from "../guard/useRun";

/** One on/off switch: a button with the switch role, its name, and what "off" or "on" means. */
export function Toggle({
  label,
  hint,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  hint: ReactNode;
  checked: boolean;
  disabled?: boolean;
  onChange: (on: boolean) => void;
}) {
  return (
    <div className="toggle">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        className={`toggle__switch${checked ? " is-on" : ""}`}
        disabled={disabled}
        onClick={() => onChange(!checked)}
      >
        <span className="toggle__knob" aria-hidden="true" />
      </button>
      <div className="toggle__words">
        <span className="toggle__label">
          {label} <span className="toggle__state">{checked ? "On" : "Off"}</span>
        </span>
        <span className="muted">{hint}</span>
      </div>
    </div>
  );
}

type Key = keyof Switches;

/**
 * Settings → Switches (ADR-021): turn whole features on or off, and choose what workers may do on
 * your allowed websites without asking. The rules that keep you in charge have no switch.
 */
export function SwitchSettings({ learning }: { learning?: ReactNode }) {
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
          hint="Submitting a form, posting, or sending a message."
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
          hint="CAPTCHAs. On: the worker stops, shows you the page, and waits while you solve it, then carries on. Off: the worker stops and tells you. Workers never try to solve these checks."
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
      <p className="muted switches__always">
        Always on, with no switch: workers never type passwords or secrets and never try to get past
        a CAPTCHA; the sign shows whenever a worker uses the browser or your mouse; and Stop halts
        it at once.
      </p>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
