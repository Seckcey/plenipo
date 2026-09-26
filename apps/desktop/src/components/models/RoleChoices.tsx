import { Fragment, useState, type FormEvent } from "react";
import type {
  CostPreference,
  CrossCompany,
  Effort,
  ModelFeature,
  RolePolicyView,
  RoutingSnapshot,
} from "@plenipo/types";

import { setRolePolicy } from "../../api/commands";
import {
  COST_PREFERENCE_LABEL,
  CROSS_COMPANY_LABEL,
  EFFORT_LABEL,
  FEATURES,
  FEATURE_LABEL,
  choiceLabel,
  companies,
  effortLevels,
  modelLabel,
} from "../../routing/format";
import { useChange, type Apply } from "../../routing/useChange";
import { Refusal } from "./shared";

/**
 * Model choices for each role: where its next worker goes now, why, and an editor for the
 * role's ordered models and requirements.
 */
export function RoleChoices({ snapshot, onApply }: { snapshot: RoutingSnapshot; onApply: Apply }) {
  const [editing, setEditing] = useState<string | null>(null);
  return (
    <section aria-labelledby="role-choices-title">
      <h3 id="role-choices-title">Model choices for each role</h3>
      <p className="muted">
        Every new worker gets the first model its role&apos;s choices allow that is ready now.
        Full-time agents pick when their conversation starts and keep it. A position set to a fixed
        AI tool on the Organization page keeps that one.
      </p>
      <table className="table models__roles">
        <thead>
          <tr>
            <th scope="col">Role</th>
            <th scope="col">Next worker gets</th>
            <th scope="col">Why</th>
            <th scope="col">
              <span className="visually-hidden">Change</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {snapshot.roles.map((r) => (
            <Fragment key={r.roleId}>
              <tr aria-current={editing === r.roleId}>
                <th scope="row">
                  {r.roleName}
                  <span className="table__sub">{r.fullTime ? "Full-time" : "On call"}</span>
                </th>
                <td>
                  {r.next.choice ? (
                    choiceLabel(r.next.choice)
                  ) : (
                    <span className="pill pill--warn">None right now</span>
                  )}
                </td>
                <td className="models__why">{r.next.reason}</td>
                <td>
                  <button
                    type="button"
                    className="button button--small button--quiet"
                    aria-expanded={editing === r.roleId}
                    aria-label={`Change ${r.roleName}'s model choices`}
                    onClick={() => setEditing(editing === r.roleId ? null : r.roleId)}
                  >
                    {editing === r.roleId ? "Close" : "Change"}
                  </button>
                </td>
              </tr>
              {editing === r.roleId && (
                <tr>
                  <td colSpan={4}>
                    <PolicyEditor
                      snapshot={snapshot}
                      view={r}
                      onApply={onApply}
                      onDone={() => setEditing(null)}
                    />
                  </td>
                </tr>
              )}
            </Fragment>
          ))}
        </tbody>
      </table>
    </section>
  );
}

function PolicyEditor({
  snapshot,
  view,
  onApply,
  onDone,
}: {
  snapshot: RoutingSnapshot;
  view: RolePolicyView;
  onApply: Apply;
  onDone: () => void;
}) {
  const p = view.policy;
  const [models, setModels] = useState(p.models);
  const [needs, setNeeds] = useState<ModelFeature[]>(p.needs);
  const [minContext, setMinContext] = useState(p.minContextTokens?.toString() ?? "");
  const [never, setNever] = useState(p.neverCompanies);
  const [cost, setCost] = useState<CostPreference>(p.cost);
  const [cross, setCross] = useState<CrossCompany>(p.crossCompany);
  const [efforts, setEfforts] = useState<Partial<Record<string, Effort>>>(p.efforts);
  const { pending, error, run } = useChange(onApply);
  const byId = new Map(snapshot.models.map((m) => [m.id, m]));
  const label = (id: string) => {
    const m = byId.get(id);
    return m ? modelLabel(snapshot, m) : "A removed model";
  };
  const addable = snapshot.models.filter((m) => !models.includes(m.id));

  const move = (i: number, by: number) => {
    const next = [...models];
    const [item] = next.splice(i, 1);
    next.splice(i + by, 0, item as string);
    setModels(next);
  };
  const toggle = <T,>(list: T[], item: T, on: boolean) =>
    on ? [...list.filter((x) => x !== item), item] : list.filter((x) => x !== item);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    const min = minContext.trim() === "" ? null : Number(minContext);
    const ok = await run(() =>
      setRolePolicy(view.roleId, {
        models,
        needs,
        minContextTokens: min,
        neverCompanies: never,
        cost,
        crossCompany: cross,
        efforts,
      }),
    );
    if (ok) onDone();
  };

  return (
    <form
      className="models__editor"
      aria-label={`Model choices for ${view.roleName}`}
      onSubmit={(e) => void save(e)}
    >
      <fieldset className="fieldset">
        <legend>Models, in order</legend>
        {models.length === 0 ? (
          <p className="hint">
            None listed: {view.roleName} uses any model in your list —{" "}
            {COST_PREFERENCE_LABEL[cost].toLowerCase()}.
          </p>
        ) : (
          <ol className="models__order">
            {models.map((id, i) => (
              <li key={id}>
                <span className="models__rank">{i === 0 ? "First choice" : `Backup ${i}`}</span>
                <span className="models__name">{label(id)}</span>
                <EffortPicker
                  snapshot={snapshot}
                  modelId={id}
                  name={label(id)}
                  value={efforts[id]}
                  onChange={(e) => {
                    const next = { ...efforts };
                    if (e) next[id] = e;
                    else delete next[id];
                    setEfforts(next);
                  }}
                />
                <button
                  type="button"
                  className="button button--small button--quiet"
                  aria-label={`Move ${label(id)} up`}
                  disabled={i === 0}
                  onClick={() => move(i, -1)}
                >
                  ↑
                </button>
                <button
                  type="button"
                  className="button button--small button--quiet"
                  aria-label={`Move ${label(id)} down`}
                  disabled={i === models.length - 1}
                  onClick={() => move(i, 1)}
                >
                  ↓
                </button>
                <button
                  type="button"
                  className="button button--small button--quiet"
                  aria-label={`Take ${label(id)} off the list`}
                  onClick={() => setModels(models.filter((x) => x !== id))}
                >
                  Remove
                </button>
              </li>
            ))}
          </ol>
        )}
        <label className="field">
          <span>Add a model to the list</span>
          {/* Remounted after every change, so it always shows its prompt again (a controlled
              select kept at "" is not reset by the browser when its options change). */}
          <select
            key={models.join()}
            value=""
            disabled={addable.length === 0}
            onChange={(e) => e.target.value && setModels([...models, e.target.value])}
          >
            <option value="">
              {addable.length === 0 ? "Every model is listed" : "Choose a model…"}
            </option>
            {addable.map((m) => (
              <option key={m.id} value={m.id}>
                {modelLabel(snapshot, m)}
              </option>
            ))}
          </select>
        </label>
      </fieldset>

      <fieldset className="fieldset">
        <legend>The model must be able to</legend>
        {FEATURES.map((f) => (
          <label key={f} className="check">
            <input
              type="checkbox"
              checked={needs.includes(f)}
              onChange={(e) => setNeeds(toggle(needs, f, e.target.checked))}
            />
            <span>{FEATURE_LABEL[f]}</span>
          </label>
        ))}
        <label className="field">
          <span>Context size, at least (tokens)</span>
          <input
            type="number"
            min={1}
            inputMode="numeric"
            value={minContext}
            placeholder="No minimum"
            onChange={(e) => setMinContext(e.target.value)}
          />
          <small className="field__hint">
            How much text the model must take in at once. Tokens are pieces of words.
          </small>
        </label>
      </fieldset>

      <label className="field">
        <span>When no models are listed</span>
        <select value={cost} onChange={(e) => setCost(e.target.value as CostPreference)}>
          {(Object.keys(COST_PREFERENCE_LABEL) as CostPreference[]).map((c) => (
            <option key={c} value={c}>
              {COST_PREFERENCE_LABEL[c]}
            </option>
          ))}
        </select>
      </label>
      <label className="field">
        <span>Reviews</span>
        <select value={cross} onChange={(e) => setCross(e.target.value as CrossCompany)}>
          {(Object.keys(CROSS_COMPANY_LABEL) as CrossCompany[]).map((c) => (
            <option key={c} value={c}>
              {CROSS_COMPANY_LABEL[c]}
            </option>
          ))}
        </select>
        <small className="field__hint">
          For reviewers: a model from a different AI company than the one whose work it checks.
        </small>
      </label>
      <fieldset className="fieldset">
        <legend>Never use these AI companies</legend>
        {companies(snapshot).map((c) => (
          <label key={c.id} className="check">
            <input
              type="checkbox"
              checked={never.includes(c.id)}
              onChange={(e) => setNever(toggle(never, c.id, e.target.checked))}
            />
            <span>{c.label}</span>
          </label>
        ))}
      </fieldset>
      <Refusal error={error} />
      <div className="actions">
        <button type="submit" className="button button--small" disabled={pending}>
          {pending ? "Saving…" : "Save model choices"}
        </button>
        <button type="button" className="button button--small button--quiet" onClick={onDone}>
          Cancel
        </button>
      </div>
    </form>
  );
}

/** A role's effort for one model: the model's own setting, or a level its AI tool accepts. */
function EffortPicker({
  snapshot,
  modelId,
  name,
  value,
  onChange,
}: {
  snapshot: RoutingSnapshot;
  modelId: string;
  name: string;
  value: Effort | undefined;
  onChange: (effort: Effort | undefined) => void;
}) {
  const model = snapshot.models.find((m) => m.id === modelId);
  const levels = model ? effortLevels(snapshot, model.runtimeId, model.name) : [];
  if (!model || levels.length === 0) return null;
  const own = model.effort ? EFFORT_LABEL[model.effort].toLowerCase() : "the AI tool's default";
  return (
    <select
      className="models__effort"
      aria-label={`Effort for ${name}`}
      value={value && levels.includes(value) ? value : ""}
      onChange={(e) => onChange((e.target.value || undefined) as Effort | undefined)}
    >
      <option value="">Its effort ({own})</option>
      {levels.map((l) => (
        <option key={l} value={l}>
          {EFFORT_LABEL[l]} effort
        </option>
      ))}
    </select>
  );
}
