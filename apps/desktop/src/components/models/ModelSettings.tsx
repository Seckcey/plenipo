import type { LimitBehavior, RoutingSnapshot } from "@plenipo/types";

import { clearUsageLimit, setRoutingOptions } from "../../api/commands";
import { LIMIT_LABEL, until } from "../../routing/format";
import { useRouting } from "../../routing/useRouting";
import { ModelList } from "./ModelList";
import { RoleChoices } from "./RoleChoices";
import { useChange, type Apply } from "../../routing/useChange";
import { Refusal } from "./shared";

/**
 * Settings → AI models: which model each role's workers get (and why), the models to choose
 * from, the AI tools with their sign-in and usage limits, and what a usage limit does.
 */
export function ModelSettings() {
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
      <RoleChoices snapshot={s} onApply={routing.apply} />
      <ModelList snapshot={s} onApply={routing.apply} />
      <ToolList snapshot={s} onApply={routing.apply} />
      <LimitChoice snapshot={s} onApply={routing.apply} />
    </div>
  );
}

function ToolList({ snapshot, onApply }: { snapshot: RoutingSnapshot; onApply: Apply }) {
  const { pending, error, run } = useChange(onApply);
  return (
    <section aria-labelledby="tools-title">
      <h3 id="tools-title">AI tools</h3>
      <table className="table">
        <thead>
          <tr>
            <th scope="col">AI tool</th>
            <th scope="col">Company</th>
            <th scope="col">Can take work</th>
            <th scope="col">Usage limit</th>
          </tr>
        </thead>
        <tbody>
          {snapshot.tools.map((t) => (
            <tr key={t.runtimeId}>
              <th scope="row">{t.label}</th>
              <td>{t.companyLabel}</td>
              <td>
                <span className={`pill ${t.available ? "pill--ok" : "pill--warn"}`}>
                  {t.available ? "Yes" : "Not now"}
                </span>{" "}
                <span className="table__sub">{t.status}</span>
              </td>
              <td>
                {t.usageLimit ? (
                  <>
                    Reached {t.usageLimit.resetsAt ? "— resets" : "— tried again"}{" "}
                    {until(t.usageLimit.until)}{" "}
                    <button
                      type="button"
                      className="button button--small button--quiet"
                      disabled={pending}
                      onClick={() => void run(() => clearUsageLimit(t.runtimeId))}
                    >
                      Try again now
                    </button>
                  </>
                ) : (
                  "—"
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <Refusal error={error} />
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
