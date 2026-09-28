import type { LimitBehavior, RoutingSnapshot } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { setRoutingOptions } from "../../api/commands";
import { LIMIT_LABEL } from "../../routing/format";
import { useRouting } from "../../routing/useRouting";
import type { Go } from "../views";
import { ModelList } from "./ModelList";
import { RoleChoices } from "./RoleChoices";
import { RuleSettings } from "./RuleSettings";
import { useChange, type Apply } from "../../routing/useChange";
import { Refusal } from "./shared";

/**
 * Settings → AI models: the model and effort rules (the organization, departments, and agents),
 * which model each role's workers get (and why), the models to choose from, a link to the AI
 * tools page (their sign-in, usage limits, and updates), and what a usage limit does.
 */
export function ModelSettings({ go }: { go: Go }) {
  const routing = useRouting();
  const s = routing.snapshot;
  if (!s) {
    return (
      <p
        className={routing.error ? "form-error" : "muted"}
        role={routing.error ? "alert" : undefined}
      >
        {routing.error ?? "Loading the model settings…"}
      </p>
    );
  }
  return (
    <div className="models">
      {s.notices.length > 0 && (
        <ul className="notices">
          {s.notices.map((n) => (
            <li key={n}>{n}</li>
          ))}
        </ul>
      )}
      <RuleSettings snapshot={s} onApply={routing.apply} />
      <RoleChoices snapshot={s} onApply={routing.apply} />
      <ModelList snapshot={s} onApply={routing.apply} />
      <ToolsLink snapshot={s} go={go} />
      <LimitChoice snapshot={s} onApply={routing.apply} />
    </div>
  );
}

/**
 * The AI tools' usage limits, sign-in, and updates are on the AI tools page (Phase 19, ADR-060
 * §2): a line and a link take the old table's place.
 */
function ToolsLink({ snapshot, go }: { snapshot: RoutingSnapshot; go: Go }) {
  return (
    <section aria-labelledby="tools-title">
      <h3 id="tools-title">AI tools</h3>
      <p>Usage limits, sign-in, and updates are on the AI tools page.</p>
      <div className="settings-section__actions">
        <Button size="sm" onClick={() => go({ view: "runtimes", id: null })}>
          Open the AI tools page
        </Button>
      </div>
      <p className="muted">
        <strong>Pay-per-use API billing: {snapshot.apiBilling ? "On" : "Off"}.</strong> Plenipo uses
        each AI tool&apos;s subscription sign-in and skips a tool signed in with an API key.
      </p>
    </section>
  );
}

function LimitChoice({ snapshot, onApply }: { snapshot: RoutingSnapshot; onApply: Apply }) {
  const { pending, error, run } = useChange(onApply);
  const current = snapshot.options.onUsageLimit;
  return (
    <section aria-labelledby="limits-title">
      <h3 id="limits-title">When an AI tool reaches its usage limit</h3>
      <fieldset className="fieldset" disabled={pending}>
        <legend className="visually-hidden">When an AI tool reaches its usage limit</legend>
        {(Object.keys(LIMIT_LABEL) as LimitBehavior[]).map((b) => (
          <label key={b} className="check">
            <input
              type="radio"
              name="on-usage-limit"
              checked={current === b}
              onChange={() => void run(() => setRoutingOptions({ onUsageLimit: b }))}
            />
            <span>{LIMIT_LABEL[b]}</span>
          </label>
        ))}
      </fieldset>
      <Refusal error={error} />
    </section>
  );
}
