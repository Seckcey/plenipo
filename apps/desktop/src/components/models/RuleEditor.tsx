/**
 * A model and effort rule (ADR-041): models in order with the effort for each, the effort for any
 * other model, and AI companies never to use. Used for the whole organization, a department, and
 * one agent; a role's choices (RoleChoices) share the model list and the companies.
 */
import { useId, useState, type FormEvent } from "react";
import type { Effort, ModelRule, RoutingSnapshot } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import {
  EFFORT_LABEL,
  anyEffortLevels,
  companies,
  effortLevels,
  isEmptyRule,
  modelLabel,
} from "../../routing/format";

/** A model's effort in a list: the model's own setting, or a level its AI tool accepts. */
export function EffortPicker({
  snapshot,
  modelId,
  name,
  value,
  onChange,
  describedBy,
}: {
  snapshot: RoutingSnapshot;
  modelId: string;
  name: string;
  value: Effort | undefined;
  onChange: (effort: Effort | undefined) => void;
  describedBy?: string | undefined;
}) {
  const model = snapshot.models.find((m) => m.id === modelId);
  const levels = model ? effortLevels(snapshot, model.runtimeId, model.name) : [];
  if (!model || levels.length === 0) return null;
  const own = model.effort ? EFFORT_LABEL[model.effort].toLowerCase() : "the AI tool's default";
  return (
    <select
      className="models__effort"
      aria-label={`Effort for ${name}`}
      aria-describedby={describedBy}
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

/** Models in order (first choice, then backups), each with its effort; add, move, remove. */
export function ModelOrder({
  snapshot,
  models,
  efforts,
  onModels,
  onEfforts,
  empty,
}: {
  snapshot: RoutingSnapshot;
  models: string[];
  efforts: Partial<Record<string, Effort>>;
  onModels: (models: string[]) => void;
  onEfforts: (efforts: Partial<Record<string, Effort>>) => void;
  /** What happens when no model is listed. */
  empty: string;
}) {
  const hint = useId();
  const addId = useId();
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
    onModels(next);
  };
  return (
    <fieldset className="fieldset">
      <legend>Models, in order</legend>
      {models.length === 0 ? (
        <p className="hint">{empty}</p>
      ) : (
        <>
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
                  describedBy={hint}
                  onChange={(e) => {
                    const next = { ...efforts };
                    if (e) next[id] = e;
                    else delete next[id];
                    onEfforts(next);
                  }}
                />
                <Button
                  variant="quiet"
                  size="sm"
                  aria-label={`Move ${label(id)} up`}
                  aria-describedby={hint}
                  disabled={i === 0}
                  onClick={() => move(i, -1)}
                >
                  ↑
                </Button>
                <Button
                  variant="quiet"
                  size="sm"
                  aria-label={`Move ${label(id)} down`}
                  aria-describedby={hint}
                  disabled={i === models.length - 1}
                  onClick={() => move(i, 1)}
                >
                  ↓
                </Button>
                <Button
                  variant="quiet"
                  size="sm"
                  aria-label={`Take ${label(id)} off the list`}
                  aria-describedby={hint}
                  onClick={() => {
                    const next = { ...efforts };
                    delete next[id];
                    onEfforts(next);
                    onModels(models.filter((x) => x !== id));
                  }}
                >
                  Remove
                </Button>
              </li>
            ))}
          </ol>
          <p id={hint} className="field__hint">
            The first model that is ready gets the work; the backups are tried in order. Each
            model&apos;s effort is how hard it thinks. Remove takes a model off this list.
          </p>
        </>
      )}
      <div className="field">
        <label className="field__label" htmlFor={addId}>
          Add a model to the list
        </label>
        {/* Remounted after every change, so it always shows its prompt again (a controlled
            select kept at "" is not reset by the browser when its options change). */}
        <select
          id={addId}
          key={models.join()}
          aria-describedby={`${addId}-hint`}
          value=""
          disabled={addable.length === 0}
          onChange={(e) => e.target.value && onModels([...models, e.target.value])}
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
        <small id={`${addId}-hint`} className="field__hint">
          Adds a model from your list (Settings → AI models) at the end.
        </small>
      </div>
    </fieldset>
  );
}

/** AI companies never to use: they add up across the rules ("never" means never). */
export function NeverCompanies({
  snapshot,
  value,
  onChange,
}: {
  snapshot: RoutingSnapshot;
  value: string[];
  onChange: (companies: string[]) => void;
}) {
  const hint = useId();
  return (
    <fieldset className="fieldset">
      <legend>Never use these AI companies</legend>
      {companies(snapshot).map((c) => (
        <label key={c.id} className="check">
          <input
            type="checkbox"
            aria-describedby={hint}
            checked={value.includes(c.id)}
            onChange={(e) =>
              onChange(
                e.target.checked
                  ? [...value.filter((x) => x !== c.id), c.id]
                  : value.filter((x) => x !== c.id),
              )
            }
          />
          <span>{c.label}</span>
        </label>
      ))}
      <p id={hint} className="field__hint">
        Never used for this work, even when another rule lists them: these add up.
      </p>
    </fieldset>
  );
}

/** The effort for any model the rule does not set one for. */
export function AnyEffort({
  snapshot,
  value,
  onChange,
}: {
  snapshot: RoutingSnapshot;
  value: Effort | null;
  onChange: (effort: Effort | null) => void;
}) {
  const id = useId();
  const levels = anyEffortLevels(snapshot);
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        Effort for any other model
      </label>
      <select
        id={id}
        aria-describedby={`${id}-hint`}
        value={value ?? ""}
        onChange={(e) => onChange((e.target.value || null) as Effort | null)}
      >
        <option value="">None set here</option>
        {levels.map((l) => (
          <option key={l} value={l}>
            {EFFORT_LABEL[l]} effort
          </option>
        ))}
      </select>
      <small id={`${id}-hint`} className="field__hint">
        How hard a model thinks when this rule does not name it above. A model that does not take
        this level keeps its own.
      </small>
    </div>
  );
}

/** Edit a rule and save it; `onRemove` offers to remove it (a department's or an agent's). */
export function RuleEditor({
  snapshot,
  rule,
  label,
  empty,
  pending,
  onSave,
  onRemove,
  onCancel,
}: {
  snapshot: RoutingSnapshot;
  rule: ModelRule;
  /** The form's name, e.g. "Rule for the Engineering department". */
  label: string;
  /** What happens when no model is listed. */
  empty: string;
  pending: boolean;
  onSave: (rule: ModelRule) => void;
  onRemove?: (() => void) | undefined;
  onCancel?: (() => void) | undefined;
}) {
  const [models, setModels] = useState(rule.models);
  const [efforts, setEfforts] = useState<Partial<Record<string, Effort>>>(rule.efforts);
  const [effort, setEffort] = useState<Effort | null>(rule.effort);
  const [never, setNever] = useState(rule.neverCompanies);
  const saveHint = useId();
  const next: ModelRule = { models, efforts, effort, neverCompanies: never };
  const save = (e: FormEvent) => {
    e.preventDefault();
    onSave(next);
  };
  return (
    <form className="models__editor" aria-label={label} onSubmit={save}>
      <ModelOrder
        snapshot={snapshot}
        models={models}
        efforts={efforts}
        onModels={setModels}
        onEfforts={setEfforts}
        empty={empty}
      />
      <AnyEffort snapshot={snapshot} value={effort} onChange={setEffort} />
      <NeverCompanies snapshot={snapshot} value={never} onChange={setNever} />
      <div className="actions">
        <Button
          type="submit"
          variant="primary"
          size="sm"
          disabled={pending}
          aria-describedby={saveHint}
        >
          {pending ? "Saving…" : "Save rule"}
        </Button>
        {onRemove && !isEmptyRule(rule) && (
          <Button
            variant="quiet"
            size="sm"
            disabled={pending}
            aria-describedby={saveHint}
            onClick={onRemove}
          >
            Remove rule
          </Button>
        )}
        {onCancel && (
          <Button variant="quiet" size="sm" aria-describedby={saveHint} onClick={onCancel}>
            Cancel
          </Button>
        )}
      </div>
      <p id={saveHint} className="field__hint">
        Save keeps this rule; Remove rule lets the rules around it decide. A new effort reaches open
        conversations with their next task, and never hires a new agent.
      </p>
    </form>
  );
}
