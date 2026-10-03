/**
 * A model and effort rule (ADR-041): models in order with the effort for each, the effort for any
 * other model, and AI companies never to use. Used for the whole organization, a department, and
 * one agent; a role's choices (RoleChoices) share the model list and the companies.
 */
import { useId, useState, type FormEvent } from "react";
import type { Effort, ModelInfo, ModelRule, RoutingSnapshot } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { saveModel, toCommandError } from "../../api/commands";
import {
  EFFORT_LABEL,
  anyEffortLevels,
  companies,
  effortLevels,
  everyModel,
  freeModelLabel,
  isEmptyRule,
  modelLabel,
  readAddValue,
} from "../../routing/format";
import type { Apply } from "../../routing/useChange";

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
  onApply,
}: {
  snapshot: RoutingSnapshot;
  models: string[];
  efforts: Partial<Record<string, Effort>>;
  onModels: (models: string[]) => void;
  onEfforts: (efforts: Partial<Record<string, Effort>>) => void;
  /** What happens when no model is listed. */
  empty: string;
  /** Takes the model settings after a model is added to Your models from this list. */
  onApply?: Apply | undefined;
}) {
  const hint = useId();
  const addId = useId();
  // Models this list just added to Your models, named here until the settings catch up.
  const [added, setAdded] = useState<ModelInfo[]>([]);
  const [adding, setAdding] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const known = [
    ...snapshot.models,
    ...added.filter((a) => !snapshot.models.some((m) => m.id === a.id)),
  ];
  const byId = new Map(known.map((m) => [m.id, m]));
  const label = (id: string) => {
    const m = byId.get(id);
    return m ? modelLabel(snapshot, m) : "A removed model";
  };
  // Every model of every AI tool, subscriptions first (Phase 25, item 2.5).
  const groups = everyModel({ ...snapshot, models: known }, models);
  const choose = async (value: string) => {
    const picked = readAddValue(value);
    if (!picked) return;
    if ("id" in picked) {
      onModels([...models, picked.id]);
      return;
    }
    // Not in Your models yet: added there first, by itself.
    const tool = snapshot.tools.find((t) => t.runtimeId === picked.runtimeId);
    const k = [...(tool?.knownModels ?? []), ...(tool?.newModels ?? [])].find(
      (x) => x.name === picked.name,
    );
    setAdding(true);
    setRefusal(null);
    try {
      const next = await saveModel({
        runtimeId: picked.runtimeId,
        name: picked.name,
        label: freeModelLabel(
          { ...snapshot, models: known },
          k?.label ?? picked.name,
          tool?.label ?? picked.runtimeId,
        ),
        features: [],
        cost: "standard",
      });
      const model = next.models.find(
        (m) => m.runtimeId === picked.runtimeId && m.name === picked.name,
      );
      onApply?.(next);
      if (model) {
        setAdded((a) => [...a, model]);
        onModels([...models, model.id]);
      }
    } catch (reason) {
      setRefusal(toCommandError(reason).message);
    } finally {
      setAdding(false);
    }
  };
  const addable = groups.some((g) => g.options.length > 0);
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
      <label className="field">
        <span>Add a model to the list</span>
        {/* Remounted after every change, so it always shows its prompt again (a controlled
            select kept at "" is not reset by the browser when its options change). */}
        <select
          key={models.join()}
          aria-describedby={`${addId}-hint`}
          value=""
          disabled={!addable || adding}
          onChange={(e) => void choose(e.target.value)}
        >
          <option value="">
            {adding ? "Adding…" : addable ? "Choose a model…" : "Every model is listed"}
          </option>
          {groups.map((g) => (
            <optgroup key={g.label} label={g.label}>
              {g.options.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
      </label>
      <small id={`${addId}-hint`} className="field__hint field__hint--after">
        Adds it at the end. Every AI tool&apos;s models are here, your subscriptions first; one not
        in Your models yet is added there too.
      </small>
      {refusal && (
        <p className="form-error" role="alert">
          {refusal}
        </p>
      )}
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

/**
 * AI companies never to use, set at this level before Phase 25 (item 2.6): "never use" is now set
 * for the whole organization only. A list kept from before still counts, and each company can be
 * taken off it. Nothing shows when there is none.
 */
export function NeverFromBefore({
  snapshot,
  value,
  onChange,
}: {
  snapshot: RoutingSnapshot;
  value: string[];
  onChange: (companies: string[]) => void;
}) {
  if (value.length === 0) return null;
  const label = (id: string) => companies(snapshot).find((c) => c.id === id)?.label ?? id;
  return (
    <fieldset className="fieldset">
      <legend>Never used here (set before)</legend>
      <ul className="models__never">
        {value.map((id) => (
          <li key={id}>
            {label(id)}{" "}
            <Button
              variant="quiet"
              size="sm"
              aria-label={`Remove ${label(id)} from this list`}
              onClick={() => onChange(value.filter((x) => x !== id))}
            >
              Remove
            </Button>
          </li>
        ))}
      </ul>
      <p className="field__hint">
        AI companies never to use are now set for the whole organization. This list from before
        still counts until you remove it.
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
    <>
      <label className="field">
        <span>Effort for any other model</span>
        <select
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
      </label>
      <small id={`${id}-hint`} className="field__hint field__hint--after">
        How hard a model thinks when this rule does not name it above. A model that does not take
        this level keeps its own.
      </small>
    </>
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
  onApply,
  neverHere = false,
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
  /** Takes the model settings after a model is added to Your models from the list. */
  onApply?: Apply | undefined;
  /** The whole organization's rule: AI companies never to use are set here (Phase 25, 2.6). */
  neverHere?: boolean;
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
        onApply={onApply}
      />
      <AnyEffort snapshot={snapshot} value={effort} onChange={setEffort} />
      {neverHere ? (
        <NeverCompanies snapshot={snapshot} value={never} onChange={setNever} />
      ) : (
        <NeverFromBefore snapshot={snapshot} value={never} onChange={setNever} />
      )}
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
