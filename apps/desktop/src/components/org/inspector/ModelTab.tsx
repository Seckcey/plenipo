/**
 * The AI model tab: why this model and effort (naming the rule that decided), automatic or a fixed
 * AI tool and model, its effort, its own rule, and what its specialty suggests (ADR-041, ADR-042).
 */
import { useId, useState, type FormEvent } from "react";
import type {
  CandidateNote,
  Effort,
  ModelRule,
  OrgSnapshot,
  PositionInfo,
  PositionPatchInput,
  RoutingSnapshot,
} from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { setModelRule } from "../../../api/commands";
import { runtimeLabel } from "../../../org/format";
import { RuntimeOptions } from "../RuntimeOptions";
import { subscriptionInstead } from "../runtimeChoices";
import {
  EFFORT_LABEL,
  choiceLabel,
  effortLevels,
  emptyRule,
  isEmptyRule,
  modelLabel,
  ruleSummary,
} from "../../../routing/format";
import { useRouting } from "../../../routing/useRouting";
import { ModelPicker } from "../../models/ModelPicker";
import { neverUsed } from "../../models/neverUsed";
import { RuleEditor } from "../../models/RuleEditor";
import { Field, Option, Options, Refusal, Section } from "./parts";
import { useRun, type Run } from "./useRun";
import type { InspectorActions } from "./types";

const VERDICT_TEXT: Record<CandidateNote["verdict"], string> = {
  chosen: "chosen",
  skipped: "skipped",
  notNeeded: "not needed",
};

export function ModelTab({
  p,
  snapshot,
  actions,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  const { snapshot: routing, error: routingError } = useRouting();
  const run = useRun(actions);
  // While its own rule is open, the choices that change it wait: saving the rule would undo them.
  const [editing, setEditing] = useState(false);
  return (
    <>
      <RouteSection p={p} snapshot={snapshot} />
      {actions.openPage && (
        <Options>
          <Option
            label="See every rule"
            icon="chevronRight"
            hint="Settings → AI models: the rules for the organization, departments, roles, and agents."
            onClick={() => actions.openPage?.({ view: "settings", id: "aiModels" })}
          />
        </Options>
      )}
      {p.active ? (
        <>
          <FixedOrAutomatic
            key={`${p.automatic ? "auto" : `${p.runtimeId}|${p.model ?? ""}`}`}
            p={p}
            snapshot={snapshot}
            routing={routing}
            actions={actions}
          />
          {routing ? (
            <>
              <EffortSection p={p} routing={routing} run={run} locked={editing} />
              <OwnRule p={p} routing={routing} run={run} open={editing} onOpen={setEditing} />
              <Suggestions p={p} snapshot={snapshot} routing={routing} run={run} locked={editing} />
            </>
          ) : (
            <p className={routingError ? "inspector__detail" : "muted"}>
              {routingError
                ? `Its effort and rule cannot be shown: the model settings could not be read (${routingError}).`
                : "Reading the model settings…"}
            </p>
          )}
          <Refusal error={run.error} />
        </>
      ) : (
        <p className="muted">Bring it back to change its AI model, effort, or rule.</p>
      )}
    </>
  );
}

/** Where the position's next worker (or a new agent) goes, and why. */
function RouteSection({ p, snapshot }: { p: PositionInfo; snapshot: OrgSnapshot }) {
  const route = p.route;
  if (!route) return null;
  const persistent = p.staffing === "persistent";
  const conversation =
    persistent && p.agent?.sessionId && p.agent.runtimeId ? p.agent.runtimeId : null;
  return (
    <Section title={persistent ? "Why this AI model" : "Why the next worker gets this model"}>
      {conversation && p.automatic && (
        <p className="muted inspector__note">
          Its conversation stays on {runtimeLabel(snapshot, conversation)}. A new agent for this
          position would get: {route.choice ? choiceLabel(route.choice) : "no model now"}.
        </p>
      )}
      <p
        className={route.choice ? "inspector__reason" : "inspector__detail"}
        data-testid="route-reason"
      >
        {route.reason}
      </p>
      {route.candidates.length > 1 && (
        <details className="advanced">
          <summary>Every model considered</summary>
          <ul className="inspector__list">
            {route.candidates.map((c) => (
              <li key={c.modelId}>
                <strong>{c.label}</strong>: {VERDICT_TEXT[c.verdict]}
                {c.note ? ` — ${c.note}` : ""}
              </li>
            ))}
          </ul>
        </details>
      )}
    </Section>
  );
}

/** Automatic (the rules pick) or an AI tool and model you fix. */
function FixedOrAutomatic({
  p,
  snapshot,
  routing,
  actions,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  routing: RoutingSnapshot | null;
  actions: InspectorActions;
}) {
  const run = useRun(actions);
  // "" means automatic: the rules pick the AI tool and model.
  const fixedRuntime = p.automatic ? "" : (p.runtimeId ?? "");
  const fixedModel = p.automatic ? "" : (p.model ?? "");
  const [runtimeId, setRuntimeId] = useState(fixedRuntime);
  const [model, setModel] = useState(fixedModel);
  const saveHint = useId();
  const automatic = runtimeId === "";
  const changed = runtimeId !== fixedRuntime || (!automatic && model.trim() !== fixedModel);
  const replacesAgent = p.agent !== null && changed;

  const save = (e: FormEvent) => {
    e.preventDefault();
    const patch: PositionPatchInput = {};
    if (runtimeId !== fixedRuntime) patch.runtimeId = runtimeId;
    if (!automatic && model.trim() !== fixedModel) patch.model = model.trim();
    if (Object.keys(patch).length > 0) void run.go(() => actions.api.update(p.id, patch));
  };

  return (
    <Section title="AI tool and model">
      <form aria-label="AI tool and model" onSubmit={save}>
        <Field
          label="AI tool"
          hint={
            (automatic ? null : subscriptionInstead(snapshot, runtimeId)) ??
            "Automatic: the rules pick the AI tool and model, and say why. Or fix one for this agent."
          }
        >
          {({ id, hintId }) => (
            <select
              id={id}
              aria-describedby={hintId}
              value={runtimeId}
              onChange={(e) => {
                setRuntimeId(e.target.value);
                setModel(e.target.value === fixedRuntime ? fixedModel : "");
              }}
            >
              <option value="">Automatic (the rules pick)</option>
              <RuntimeOptions snapshot={snapshot} current={runtimeId} />
            </select>
          )}
        </Field>
        {!automatic && (
          <ModelPicker
            routing={routing}
            runtimeId={runtimeId}
            value={model}
            onChange={setModel}
            hint="The model this AI tool runs for this agent."
          />
        )}
        {replacesAgent && (
          <p className="hint">
            Changing the AI tool or model hires a new agent for this position; the current one
            retires and its conversation ends. Changing only the effort never does.
          </p>
        )}
        <div className="option">
          <Button
            type="submit"
            variant="primary"
            size="sm"
            aria-describedby={saveHint}
            disabled={run.pending || !changed}
          >
            Save AI tool and model
          </Button>
          <span id={saveHint} className="option__hint">
            Keeps the AI tool and model chosen above for this agent.
          </span>
        </div>
      </form>
      <Refusal error={run.error} />
    </Section>
  );
}

/** Its effort: its own, or what the rules give it. Changing it never hires a new agent. */
function EffortSection({
  p,
  routing,
  run,
  locked,
}: {
  p: PositionInfo;
  routing: RoutingSnapshot;
  run: Run;
  /** Its own rule is open below: this waits until it is saved or cancelled. */
  locked: boolean;
}) {
  const rule = p.ownRule ?? emptyRule();
  const choice = p.route?.choice ?? null;
  const modelId = choice?.modelId ?? "";
  // Its own effort for the model it runs: one set for that model comes before its general one.
  const own = (modelId ? rule.efforts[modelId] : undefined) ?? rule.effort ?? null;
  const levels = p.runtimeId ? effortLevels(routing, p.runtimeId, p.model) : [];
  const now = choice?.effort
    ? `${EFFORT_LABEL[choice.effort].toLowerCase()} effort`
    : "no effort set";
  // "Now" is the rules' answer only when its own setting is not the one deciding.
  const follow =
    p.route?.effortFrom?.layer === "agent" ? "Follow the rules" : `Follow the rules (now: ${now})`;
  const save = (effort: Effort | null) => {
    // The choice here is its effort for any model, so one set for this model must not win.
    const efforts = { ...rule.efforts };
    if (modelId) delete efforts[modelId];
    const next: ModelRule = { ...rule, efforts, effort };
    void run.change(() => setModelRule({ layer: "agent", id: p.id }, next));
  };
  return (
    <Section title="Effort">
      {levels.length === 0 ? (
        <p className="muted">
          {p.runtimeId
            ? "Its model has no effort setting."
            : "No model can take its work now, so there is no effort to set."}
        </p>
      ) : (
        <Field
          label="Effort"
          hint="How hard the model thinks before it answers. Changing it never hires a new agent."
        >
          {({ id, hintId }) => (
            <select
              id={id}
              aria-describedby={hintId}
              value={own && levels.includes(own) ? own : ""}
              disabled={run.pending || locked}
              onChange={(e) => save((e.target.value || null) as Effort | null)}
            >
              <option value="">{follow}</option>
              {levels.map((l) => (
                <option key={l} value={l}>
                  {EFFORT_LABEL[l]} effort
                </option>
              ))}
            </select>
          )}
        </Field>
      )}
      {locked && levels.length > 0 && (
        <p className="muted">Save or cancel its own rule below first.</p>
      )}
    </Section>
  );
}

/** Its own rule: models in order, efforts, and companies never to use, for this agent alone. */
function OwnRule({
  p,
  routing,
  run,
  open,
  onOpen,
}: {
  p: PositionInfo;
  routing: RoutingSnapshot;
  run: Run;
  open: boolean;
  onOpen: (open: boolean) => void;
}) {
  const rule = p.ownRule ?? emptyRule();
  const setOpen = onOpen;
  const save = async (next: ModelRule) => {
    if (await run.change(() => setModelRule({ layer: "agent", id: p.id }, next))) setOpen(false);
  };
  return (
    <Section title="Its own rule">
      <p className="muted">
        {isEmptyRule(rule)
          ? "None: its role's, department's, and organization's rules decide."
          : ruleSummary(routing, rule)}
      </p>
      {open ? (
        <RuleEditor
          snapshot={routing}
          rule={rule}
          label={`Rule for ${p.title}`}
          empty="None listed: its role's, department's, or organization's list decides."
          neverFrom={neverUsed(routing, { departmentId: p.departmentId, roleId: p.roleId })}
          pending={run.pending}
          onSave={(next) => void save(next)}
          onRemove={() => void save(emptyRule())}
          onCancel={() => setOpen(false)}
        />
      ) : (
        <Options>
          <Option
            label={isEmptyRule(rule) ? "Give it its own rule" : "Change its own rule"}
            hint="Models, efforts, and AI companies for this agent alone; its rule comes before all others."
            onClick={() => setOpen(true)}
          />
        </Options>
      )}
    </Section>
  );
}

/** What its specialty suggests (ADR-042); never applied on its own. */
function Suggestions({
  p,
  snapshot,
  routing,
  run,
  locked,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  routing: RoutingSnapshot;
  run: Run;
  /** Its own rule is open: this waits until it is saved or cancelled. */
  locked: boolean;
}) {
  const specialty = snapshot.roles
    .find((r) => r.id === p.roleId)
    ?.specialties.find((s) => s.id === p.specialtyId);
  if (!specialty) return null;
  const s = specialty.suggest;
  const models = s.models.filter((id) => routing.models.some((m) => m.id === id));
  const lines: string[] = [];
  if (models.length > 0) {
    const label = (id: string) => {
      const m = routing.models.find((x) => x.id === id);
      return m ? modelLabel(routing, m) : id;
    };
    lines.push(`These models, in order: ${models.map(label).join(", ")}.`);
  }
  if (lines.length === 0) return null;
  return (
    <Section title={`What the ${specialty.name} specialty suggests`}>
      <ul className="inspector__list">
        {lines.map((l) => (
          <li key={l}>{l}</li>
        ))}
      </ul>
      {models.length > 0 && (
        <Options>
          <Option
            label="Use these models"
            hint="Puts the suggested models first in its own rule. Nothing changes until you choose this."
            disabled={run.pending || locked}
            onClick={() =>
              void run.change(() =>
                setModelRule(
                  { layer: "agent", id: p.id },
                  { ...(p.ownRule ?? emptyRule()), models },
                ),
              )
            }
          />
        </Options>
      )}
    </Section>
  );
}
