/** The Overview tab: who it is, its status, where it sits, its AI model in one line, its
 * experience, its current objective, and Give an objective. */
import { useId, useState, type FormEvent } from "react";
import type { OrgSnapshot, PositionInfo } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { POSITION_STATUS } from "../../../org/cards";
import { archivedWithLine, experienceLine } from "../../../org/control";
import { STAFFING_LABEL, STATUS_LABEL, ago, runtimeLabel, runtimeReady } from "../../../org/format";
import { canTakeObjective, positionMap } from "../../../org/rules";
import { rankName, roleLabel, titlesOf } from "../../../org/titles";
import { EFFORT_LABEL } from "../../../routing/format";
import { PILL_TONE } from "../../tones";
import { Glyph } from "../Glyph";
import { Field, ItemLink, Option, Options, Refusal, Section, TaskRow } from "./parts";
import type { InspectorActions } from "./types";

const MAX_OBJECTIVE = 20_000;

/** "Claude Code · opus · high effort". */
function modelLine(snapshot: OrgSnapshot, p: PositionInfo): string {
  if (!p.runtimeId) return "No model can take its work now";
  const effort = p.route?.choice?.effort;
  return [
    runtimeLabel(snapshot, p.runtimeId),
    p.model ?? "the AI tool's default model",
    ...(effort ? [`${EFFORT_LABEL[effort].toLowerCase()} effort`] : []),
  ].join(" · ");
}

export function OverviewTab({
  p,
  snapshot,
  actions,
  onSelect,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
  onSelect: (id: string) => void;
}) {
  const byId = positionMap(snapshot);
  const supervisor = p.reportsTo ? byId.get(p.reportsTo) : null;
  const department = snapshot.departments.find((d) => d.id === p.departmentId) ?? null;
  const project = snapshot.projects.find((x) => x.id === p.projectId) ?? null;
  const role = snapshot.roles.find((r) => r.id === p.roleId);
  const t = titlesOf(snapshot);
  const sessionId = p.agent?.sessionId ?? null;

  return (
    <>
      <div className="inspector__identity">
        <span className={`topo-node__glyph topo-node__glyph--${p.kind}`}>
          <Glyph name={role?.glyph ?? "worker"} size={20} />
        </span>
        <div>
          <div>
            {role ? roleLabel(t, role) : p.roleName}
            {p.specialty ? ` (${p.specialty})` : ""} · {STAFFING_LABEL[p.staffing]}
          </div>
          <StatusPill status={POSITION_STATUS[p.status]} label={STATUS_LABEL[p.status]} />
          {p.statusDetail && <p className="inspector__detail">{p.statusDetail}</p>}
        </div>
      </div>

      {!p.active ? (
        <p className="muted">
          Archived {p.archivedAt ? ago(p.archivedAt) : ""} {archivedWithLine(p)}. Its history stays
          in the Ledger. Bring it back or delete it for good on the Manage tab.
        </p>
      ) : p.staffing === "persistent" ? (
        <ObjectivePanel p={p} actions={actions} />
      ) : (
        <p className="muted inspector__note">
          On call: its team&apos;s lead hands it tasks, and a new worker is brought in for each one.
        </p>
      )}

      <dl className="kv">
        <dt>Reports to</dt>
        <dd>
          {supervisor ? (
            <ItemLink onClick={() => onSelect(supervisor.id)}>{supervisor.title}</ItemLink>
          ) : (
            `You (${rankName(t, "owner")})`
          )}
        </dd>
        <dt>Rank</dt>
        <dd>{rankName(t, p.kind)}</dd>
        {p.specialty && (
          <>
            <dt>Specialty</dt>
            <dd>{p.specialty}</dd>
          </>
        )}
        <dt>AI model</dt>
        <dd>
          {modelLine(snapshot, p)}{" "}
          {p.runtimeId && !runtimeReady(snapshot, p.runtimeId) && (
            <StatusPill status={PILL_TONE.warn} label="Not ready" />
          )}
        </dd>
        <dt>Chosen by</dt>
        <dd>
          {p.automatic
            ? `Automatic: ${role?.name ?? p.roleName} model choices`
            : "You (fixed for this position)"}
        </dd>
        <dt>Department</dt>
        <dd>{department?.name ?? "—"}</dd>
        <dt>Project</dt>
        <dd>{project?.name ?? "—"}</dd>
        <dt>Experience</dt>
        <dd>
          {experienceLine(p.experience)}
          {p.experience.experienced ? " · experienced" : ""}. Your organization&apos;s average:{" "}
          {snapshot.averageExperience}.
        </dd>
        {p.agent && (
          <>
            <dt>Agent</dt>
            <dd>Hired {ago(p.agent.hiredAt)}</dd>
          </>
        )}
        {(p.history.retired > 0 || p.history.failed > 0) && (
          <>
            <dt>Former agents</dt>
            <dd>
              {p.history.retired} retired
              {p.history.failed > 0 && ` · ${p.history.failed} failed`}
            </dd>
          </>
        )}
      </dl>

      {p.currentTask && (
        <Section title="Current objective">
          <TaskRow task={p.currentTask} onOpen={actions.openTask} />
        </Section>
      )}

      {(actions.openPage || sessionId) && (
        <Options>
          {actions.openPage && (
            <Option
              label="Open its page"
              icon="chevronRight"
              hint="Its work, history, and permissions on a page of its own."
              onClick={() => actions.openPage?.({ view: "worker", id: p.id })}
            />
          )}
          {sessionId && (
            <Option
              label="Open conversation"
              hint="What its agent and you said, task by task."
              onClick={() => actions.openSession(sessionId)}
            />
          )}
        </Options>
      )}
    </>
  );
}

function ObjectivePanel({ p, actions }: { p: PositionInfo; actions: InspectorActions }) {
  const [objective, setObjective] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sent, setSent] = useState(false);
  const sendHint = useId();
  if (!p.agent) {
    return (
      <p className="hint">
        {p.title} is vacant. Hire an agent into it (on the Manage tab) to give it objectives.
      </p>
    );
  }
  const busy = p.status === "working" || p.status === "waiting";
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setPending(true);
    setError(null);
    const failure = await actions.giveObjective(p.id, objective);
    setPending(false);
    setError(failure);
    if (failure === null) {
      setObjective("");
      setSent(true);
    }
  };
  return (
    <form
      className="inspector__objective"
      aria-label="Give an objective"
      onSubmit={(e) => void submit(e)}
    >
      <Field
        label={`Objective for ${p.title}`}
        hint="Say what it should get done. It can hand parts of it to its team."
      >
        {({ id, hintId }) => (
          <textarea
            id={id}
            aria-describedby={hintId}
            value={objective}
            rows={3}
            maxLength={MAX_OBJECTIVE}
            placeholder={
              canTakeObjective(p) ? "What should it get done? It can hand work to its team." : ""
            }
            onChange={(e) => {
              setObjective(e.target.value);
              setSent(false);
            }}
          />
        )}
      </Field>
      {busy && <p className="muted">Busy with its current objective; wait until it finishes.</p>}
      {sent && (
        <p className="status status--ok" role="status">
          Objective given. Follow it here or in Workers.
        </p>
      )}
      <Refusal error={error} />
      <div className="option">
        <Button
          type="submit"
          variant="primary"
          aria-describedby={sendHint}
          disabled={pending || busy || objective.trim() === ""}
        >
          {pending ? "Sending…" : "Give objective"}
        </Button>
        <span id={sendHint} className="option__hint">
          {p.agent.sessionId
            ? "Its agent starts on it now, in the same conversation."
            : "Its agent starts on it now, in its first conversation."}
        </span>
      </div>
    </form>
  );
}
