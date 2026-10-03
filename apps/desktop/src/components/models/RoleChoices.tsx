import { Fragment, useState, type FormEvent } from "react";
import type {
  CostPreference,
  CrossCompany,
  Effort,
  RolePolicyView,
  RoutingSnapshot,
} from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { setRolePolicy } from "../../api/commands";
import {
  COST_PREFERENCE_LABEL,
  CROSS_COMPANY_LABEL,
  choiceLabel,
} from "../../routing/format";
import { useChange, type Apply } from "../../routing/useChange";
import { PILL_TONE } from "../tones";
import { AnyEffort, ModelOrder, NeverCompanies } from "./RuleEditor";
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
                    <StatusPill status={PILL_TONE.warn} label="None right now" />
                  )}
                </td>
                <td className="models__why">{r.next.reason}</td>
                <td>
                  <Button
                    variant="quiet"
                    size="sm"
                    aria-expanded={editing === r.roleId}
                    aria-label={`Change ${r.roleName}'s model choices`}
                    onClick={() => setEditing(editing === r.roleId ? null : r.roleId)}
                  >
                    {editing === r.roleId ? "Close" : "Change"}
                  </Button>
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
  const [never, setNever] = useState(p.neverCompanies);
  const [cost, setCost] = useState<CostPreference>(p.cost);
  const [cross, setCross] = useState<CrossCompany>(p.crossCompany);
  const [efforts, setEfforts] = useState<Partial<Record<string, Effort>>>(p.efforts);
  const [effort, setEffort] = useState<Effort | null>(p.effort);
  const { pending, error, run } = useChange(onApply);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    // What the model must do and its context size are no longer asked (Phase 25, items 2.3 and
    // 2.4): the saved ones are kept as they are.
    const ok = await run(() =>
      setRolePolicy(view.roleId, {
        models,
        needs: p.needs,
        minContextTokens: p.minContextTokens,
        neverCompanies: never,
        cost,
        crossCompany: cross,
        efforts,
        effort,
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
      <ModelOrder
        snapshot={snapshot}
        models={models}
        efforts={efforts}
        onModels={setModels}
        onEfforts={setEfforts}
        empty={`None listed: ${view.roleName} uses any model in your list — ${COST_PREFERENCE_LABEL[cost].toLowerCase()}.`}
      />
      <AnyEffort snapshot={snapshot} value={effort} onChange={setEffort} />

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
      <NeverCompanies snapshot={snapshot} value={never} onChange={setNever} />
      <Refusal error={error} />
      <div className="actions">
        <Button type="submit" variant="primary" size="sm" disabled={pending}>
          {pending ? "Saving…" : "Save model choices"}
        </Button>
        <Button variant="quiet" size="sm" onClick={onDone}>
          Cancel
        </Button>
      </div>
    </form>
  );
}
