import { useState, type FormEvent } from "react";
import type {
  CostClass,
  Effort,
  ModelFeature,
  ModelInfo,
  ModelInput,
  RoutingSnapshot,
} from "@plenipo/types";

import { removeModel, saveModel } from "../../api/commands";
import { ago } from "../../org/format";
import {
  COSTS,
  COST_LABEL,
  EFFORT_LABEL,
  FEATURES,
  FEATURE_LABEL,
  effortLevels,
  tokens,
} from "../../routing/format";
import { Modal } from "../org/Modal";
import { useChange, type Apply } from "../../routing/useChange";
import { ModelPicker } from "./ModelPicker";
import { Refusal } from "./shared";

/** A model to add, or one to change. */
type Draft = { model: ModelInfo } | { add: Partial<ModelInput> };

/** The model registry: the owner's models, and the ones the AI tools reported running. */
export function ModelList({ snapshot, onApply }: { snapshot: RoutingSnapshot; onApply: Apply }) {
  const [draft, setDraft] = useState<Draft | null>(null);
  const { pending, error, run } = useChange(onApply);
  const tool = (id: string) => snapshot.tools.find((t) => t.runtimeId === id)?.label ?? id;
  const unlisted = snapshot.seen.filter((s) => !s.listed);

  return (
    <section aria-labelledby="models-title">
      <div className="section-header">
        <h3 id="models-title">Your models</h3>
        <button
          type="button"
          className="button button--small"
          onClick={() => setDraft({ add: {} })}
        >
          Add a model
        </button>
      </div>
      <p className="muted">
        The models your roles can choose from. Plenipo cannot ask the AI tools what a model can do
        or costs, so you say it here; a model not marked as able to do something is treated as
        unable.
      </p>
      <table className="table">
        <thead>
          <tr>
            <th scope="col">Name</th>
            <th scope="col">AI tool</th>
            <th scope="col">Model the tool runs</th>
            <th scope="col">Can also</th>
            <th scope="col">Context</th>
            <th scope="col">Cost</th>
            <th scope="col">Effort</th>
            <th scope="col">
              <span className="visually-hidden">Actions</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {snapshot.models.map((m) => (
            <tr key={m.id}>
              <th scope="row">
                {m.label}
                {m.builtIn && <span className="table__sub">Built in</span>}
              </th>
              <td>{tool(m.runtimeId)}</td>
              <td>{m.name ?? "Its default"}</td>
              <td>{m.features.map((f) => FEATURE_LABEL[f]).join(", ") || "—"}</td>
              <td>{m.contextTokens ? tokens(m.contextTokens) : "—"}</td>
              <td>{COST_LABEL[m.cost]}</td>
              <td>{m.effort ? EFFORT_LABEL[m.effort] : "Tool's default"}</td>
              <td className="models__actions">
                <button
                  type="button"
                  className="button button--small button--quiet"
                  aria-label={`Edit ${m.label}`}
                  onClick={() => setDraft({ model: m })}
                >
                  Edit
                </button>
                {!m.builtIn && (
                  <button
                    type="button"
                    className="button button--small button--quiet"
                    aria-label={`Remove ${m.label}`}
                    disabled={pending}
                    onClick={() => void run(() => removeModel(m.id))}
                  >
                    Remove
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <Refusal error={error} />
      {unlisted.length > 0 && (
        <>
          <h4>Seen in use</h4>
          <p className="muted">Models your AI tools reported running that are not in your list.</p>
          <ul className="settings">
            {unlisted.map((s) => (
              <li key={`${s.runtimeId}/${s.name}`}>
                <strong>{s.name}</strong> on {s.runtimeLabel} — {s.runs} run
                {s.runs === 1 ? "" : "s"}, last {ago(s.lastUsed)}{" "}
                <button
                  type="button"
                  className="button button--small button--quiet"
                  onClick={() =>
                    setDraft({ add: { runtimeId: s.runtimeId, name: s.name, label: s.name } })
                  }
                >
                  Add to your models
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
      {draft && (
        <ModelDialog
          snapshot={snapshot}
          draft={draft}
          onCancel={() => setDraft(null)}
          onApply={(next) => {
            onApply(next);
            setDraft(null);
          }}
        />
      )}
    </section>
  );
}

function ModelDialog({
  snapshot,
  draft,
  onCancel,
  onApply,
}: {
  snapshot: RoutingSnapshot;
  draft: Draft;
  onCancel: () => void;
  onApply: Apply;
}) {
  const existing = "model" in draft ? draft.model : null;
  const add: Partial<ModelInput> = "add" in draft ? draft.add : {};
  const [runtimeId, setRuntimeId] = useState(
    existing?.runtimeId ?? add.runtimeId ?? snapshot.tools[0]?.runtimeId ?? "",
  );
  const [name, setName] = useState(existing?.name ?? add.name ?? "");
  const [label, setLabel] = useState(existing?.label ?? add.label ?? "");
  // Until the owner names it, the model's name follows the one chosen ("sonnet" → "Sonnet").
  const [labelEdited, setLabelEdited] = useState(existing !== null);
  const [features, setFeatures] = useState<ModelFeature[]>(
    existing?.features ?? add.features ?? [],
  );
  const [context, setContext] = useState(
    (existing?.contextTokens ?? add.contextTokens)?.toString() ?? "",
  );
  const [cost, setCost] = useState<CostClass>(existing?.cost ?? add.cost ?? "standard");
  const [effort, setEffort] = useState<Effort | "">(existing?.effort ?? add.effort ?? "");
  const { pending, error, run } = useChange(onApply);
  const builtIn = existing?.builtIn ?? false;
  const levels = effortLevels(snapshot, runtimeId);
  // A level the chosen AI tool does not accept falls back to its default.
  const chosenEffort = effort !== "" && levels.includes(effort) ? effort : "";

  const tool = snapshot.tools.find((t) => t.runtimeId === runtimeId);
  const chooseName = (next: string) => {
    setName(next);
    if (labelEdited) return;
    const alias = tool?.modelAliases.includes(next) ?? false;
    setLabel(alias ? next.charAt(0).toUpperCase() + next.slice(1) : next);
  };
  // A model already in your list (other than this one) cannot be added again.
  const inYourList = (n: string) =>
    snapshot.models.some(
      (m) => m.id !== existing?.id && m.runtimeId === runtimeId && (m.name ?? "") === n,
    )
      ? "already in your list"
      : null;

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const input: ModelInput = {
      runtimeId,
      label: label.trim(),
      features,
      cost,
      ...(existing ? { id: existing.id } : {}),
      ...(name.trim() && !builtIn ? { name: name.trim() } : {}),
      ...(context.trim() ? { contextTokens: Number(context) } : {}),
      ...(chosenEffort ? { effort: chosenEffort } : {}),
    };
    void run(() => saveModel(input));
  };

  return (
    <Modal title={existing ? `Edit ${existing.label}` : "Add a model"} onClose={onCancel}>
      <form
        className="modal__body"
        aria-label={existing ? "Edit model" : "Add a model"}
        onSubmit={submit}
      >
        <label className="field">
          <span>AI tool</span>
          <select
            value={runtimeId}
            disabled={builtIn}
            onChange={(e) => {
              setRuntimeId(e.target.value);
              chooseName("");
            }}
          >
            {snapshot.tools.map((t) => (
              <option key={t.runtimeId} value={t.runtimeId}>
                {t.label}
              </option>
            ))}
          </select>
        </label>
        <ModelPicker
          routing={snapshot}
          runtimeId={runtimeId}
          value={builtIn ? "" : name}
          onChange={chooseName}
          yours={false}
          unavailable={inYourList}
          disabled={builtIn}
        />
        <label className="field">
          <span>Your name for it</span>
          <input
            value={label}
            maxLength={80}
            required
            onChange={(e) => {
              setLabel(e.target.value);
              setLabelEdited(true);
            }}
          />
        </label>
        <fieldset className="fieldset">
          <legend>It can also</legend>
          {FEATURES.map((f) => (
            <label key={f} className="check">
              <input
                type="checkbox"
                checked={features.includes(f)}
                onChange={(e) =>
                  setFeatures(e.target.checked ? [...features, f] : features.filter((x) => x !== f))
                }
              />
              <span>{FEATURE_LABEL[f]}</span>
            </label>
          ))}
        </fieldset>
        <label className="field">
          <span>Context size (tokens, optional)</span>
          <input
            type="number"
            min={1}
            inputMode="numeric"
            value={context}
            onChange={(e) => setContext(e.target.value)}
          />
        </label>
        <label className="field">
          <span>Cost</span>
          <select value={cost} onChange={(e) => setCost(e.target.value as CostClass)}>
            {COSTS.map((c) => (
              <option key={c} value={c}>
                {COST_LABEL[c]}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Effort</span>
          <select
            value={chosenEffort}
            disabled={levels.length === 0}
            onChange={(e) => setEffort(e.target.value as Effort | "")}
          >
            <option value="">The AI tool&apos;s default</option>
            {levels.map((l) => (
              <option key={l} value={l}>
                {EFFORT_LABEL[l]}
              </option>
            ))}
          </select>
          <small className="field__hint">
            How hard the model thinks before it answers. Higher is slower and uses more of your
            plan. A role can choose its own effort for this model.
          </small>
        </label>
        <Refusal error={error} />
        <footer className="modal__footer">
          <button type="button" className="button button--quiet" onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" className="button" disabled={pending || label.trim() === ""}>
            {pending ? "Saving…" : existing ? "Save model" : "Add model"}
          </button>
        </footer>
      </form>
    </Modal>
  );
}
