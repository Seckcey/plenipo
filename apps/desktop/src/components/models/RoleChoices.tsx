import { useState, type FormEvent } from "react";
import type {
  CostPreference,
  CrossCompany,
  Effort,
  RolePolicyView,
  RoutingSnapshot,
} from "@plenipo/types";
import { Button, Disclosure } from "@plenipo/ui";

import { setRolePolicy } from "../../api/commands";
import { COST_PREFERENCE_LABEL, CROSS_COMPANY_LABEL } from "../../routing/format";
import { useChange, type Apply } from "../../routing/useChange";
import { AnyEffort, ModelOrder, NeverFromBefore } from "./RuleEditor";
import { Refusal } from "./shared";

/** A role's model choices, edited from Settings → AI models' table (Phase 25, item 2.6). */
export function PolicyEditor({
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
        onApply={onApply}
      />
      <AnyEffort snapshot={snapshot} value={effort} onChange={setEffort} />

      {/* What most roles never need (Phase 25, item 2.6). */}
      <Disclosure title="More" headingLevel={4} rememberAs="models:role-more">
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
        <NeverFromBefore snapshot={snapshot} value={never} onChange={setNever} />
      </Disclosure>
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
