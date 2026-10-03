import type { LimitBehavior, RoutingSnapshot } from "@plenipo/types";
import { Button, Disclosure } from "@plenipo/ui";

import { setRoutingOptions } from "../../api/commands";
import { LIMIT_LABEL } from "../../routing/format";
import { useRouting } from "../../routing/useRouting";
import type { Go } from "../views";
import { ModelList } from "./ModelList";
import { RuleSettings } from "./RuleSettings";
import { WhoUsesWhat } from "./WhoUsesWhat";
import { useChange, type Apply } from "../../routing/useChange";
import { Refusal } from "./shared";

/**
 * Settings → AI models (Phase 25, item 2.6): **Who uses what** (each role's model, backup, and
 * effort, with a row for the whole organization and each department), the models to choose from,
 * a link to the AI tools page (their sign-in, usage limits, and updates), what a usage limit
 * does, and **More** (the agents with a rule of their own).
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
      <WhoUsesWhat snapshot={s} onApply={routing.apply} />
      <ModelList snapshot={s} onApply={routing.apply} />
      <ToolsLink go={go} />
      <LimitChoice snapshot={s} onApply={routing.apply} />
      <Disclosure
        title="More"
        summary={
          s.agents.length === 0
            ? "No agent has a rule of its own"
            : `${s.agents.length === 1 ? "1 agent has" : `${s.agents.length} agents have`} a rule of its own`
        }
        rememberAs="models:more"
      >
        <RuleSettings snapshot={s} onApply={routing.apply} />
      </Disclosure>
    </div>
  );
}

/**
 * The AI tools' usage limits, sign-in, and updates are on the AI tools page (Phase 19, ADR-060
 * §2): a line and a link take the old table's place.
 */
function ToolsLink({ go }: { go: Go }) {
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
        <strong>Paid AI keys</strong> are used only while Let workers use paid AI keys is on, and
        only for positions you list them for. An AI tool signed in with its own API key outside
        Plenipo is skipped: its costs would skip your spending caps.
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
