import type { PermissionsSnapshot, Safety } from "@plenipo/types";
import { Segmented } from "@plenipo/ui";

import { setSafety } from "../api/commands";
import { usePermissions } from "../guard/usePermissions";
import { useRun } from "../guard/useRun";
import { SAFETY_CHOICES, safetyWarning } from "./safety";

/**
 * Settings → Safety (ADR-201, the owner's order of 2026-10-03: "be light on restrictive
 * permissions and let the user turn it up"). One choice for the whole organization: how much
 * Plenipo asks before an agent saves files or runs programs. The warnings live here, not in the
 * way of the work. What always asks you, or stays blocked, is listed under it.
 */
export function SafetySettings() {
  const permissions = usePermissions();
  const { pending, error, run } = useRun((s: PermissionsSnapshot) => permissions.apply(s));
  const settings = permissions.snapshot?.settings;
  if (!settings) {
    return (
      <p className={permissions.error ? "form-error" : "muted"}>
        {permissions.error ?? "Loading your safety setting…"}
      </p>
    );
  }
  const choose = (next: Safety) => {
    if (next !== settings.safety) void run(() => setSafety(next));
  };
  return (
    <div className="safety">
      <section aria-labelledby="safety-choice">
        <h3 id="safety-choice">How much should Plenipo ask you?</h3>
        <Segmented<Safety>
          label="How much Plenipo asks you"
          value={settings.safety}
          options={SAFETY_CHOICES.map((c) => ({ value: c.value, label: c.label }))}
          onChange={choose}
        />
        <ul className="safety__choices">
          {SAFETY_CHOICES.map((c) => (
            <li key={c.value} aria-current={settings.safety === c.value ? "true" : undefined}>
              <strong>{c.label}</strong>
              {settings.safety === c.value && <span className="safety__now"> (on now)</span>}{" "}
              <span className="muted">{c.says}</span>
            </li>
          ))}
        </ul>
        <p
          className={settings.safety === "light" ? "notice-box notice-box--danger" : "notice-box"}
          role="note"
        >
          {safetyWarning(settings.safety)}
        </p>
        {pending && <p className="muted">Saving…</p>}
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
      </section>
      <section aria-labelledby="safety-asks">
        <h3 id="safety-asks">What still asks you, whichever you choose</h3>
        <ul className="safety__list">
          {settings.sensitive.map((k) => (
            <li key={k.kind}>
              {k.label}{" "}
              <span className="muted">{k.rule === "block" ? "(blocked)" : "(asks you)"}</span>
            </li>
          ))}
        </ul>
      </section>
      <section aria-labelledby="safety-never">
        <h3 id="safety-never">What is never allowed</h3>
        <ul className="safety__list">
          <li>Reading or changing your secrets, and the files on your blocked list.</li>
          <li>Running the programs on your Never run list.</li>
          <li>
            Opening or changing files outside the agent's own folder with Plenipo's file tools.
          </li>
        </ul>
        <p className="muted">
          Change these lists, and what each kind of agent may do, in Settings → Permissions.
        </p>
      </section>
    </div>
  );
}
