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
import { ObjectiveFilesList } from "../../../files/ObjectiveFiles";
import { useObjectiveFiles } from "../../../files/useObjectiveFiles";
import { EFFORT_LABEL } from "../../../routing/format";
import { useOpenWatch } from "../../../terminal/useTerminal";
import { PILL_TONE } from "../../tones";
import { LiveConversation } from "../../../live/LiveConversation";
import { liveWork } from "../../../live/words";
import { AskQuestionButton } from "../../sideChat/AskQuestion";
import { canAsk } from "../../sideChat/canAsk";
import { StopButton } from "../../stop/StopWork";
import { workToStop } from "../../stop/stopWork";
import { Glyph } from "../Glyph";
import { Field, ItemLink, Option, Options, Refusal, Section, TaskRow } from "./parts";
import type { InspectorActions } from "./types";

const MAX_OBJECTIVE = 20_000;

/** "Claude Code · opus · high effort". */
function modelLine(snapshot: OrgSnapshot, p: PositionInfo): string {
  if (!p.runtimeId) {
    return p.active
      ? "No model can take its work now"
      : "Automatic: the rules pick one when it is brought back";
  }
  const effort = p.route?.choice?.effort;
  return [
    runtimeLabel(snapshot, p.runtimeId),
    p.model ?? "the AI tool's default model",
    ...(effort ? [`${EFFORT_LABEL[effort].toLowerCase()} effort`] : []),
  ].join(" · ");
}

/** Who picked its model (ADR-041): you, its own rule, its department's or the organization's
 * rule, or else its role's model choices. */
function chosenBy(p: PositionInfo, roleName: string): string {
  if (!p.automatic) return "You (fixed for this agent)";
  const from = p.route?.modelFrom;
  switch (from?.layer) {
    case "agent":
      return "Automatic: its own rule";
    case "department":
    case "organization":
      return `Automatic: ${from.name}'s rule`;
    default:
      return `Automatic: ${roleName} model choices`;
  }
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
  const stopWork = workToStop(p);
  const liveNow = liveWork(p)[0] ?? null;
  const stopHint = useId();
  const askHint = useId();
  // Watch (Phase 18, ADR-055): what its workers change, in the terminal panel. Hidden where
  // there is no terminal panel, and for an archived or vacant position.
  const openWatch = useOpenWatch();
  const watch =
    openWatch && p.active && (p.agent || p.staffing !== "persistent")
      ? () => openWatch(p.id, p.title)
      : null;

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
          {/* Ask it a question while it works (Phase 25, item 3.5). */}
          {canAsk(p) && (
            <div className="inspector__stop">
              <AskQuestionButton p={p} onAsked={actions.openSession} describedBy={askHint} />
              <span id={askHint} className="muted">
                A side chat: it answers from what it knows, and its work goes on.
              </span>
            </div>
          )}
          {/* Stop its work now, after a question (Phase 25, item 3.3). */}
          {stopWork.length > 0 && (
            <div className="inspector__stop">
              <StopButton
                who={p.title}
                work={stopWork}
                fullTime={p.staffing === "persistent"}
                describedBy={stopHint}
              />
              <span id={stopHint} className="muted">
                {p.staffing === "persistent"
                  ? "Stops its task now, after a question. Its conversation stays."
                  : "Stops its workers' tasks now, after a question."}
              </span>
            </div>
          )}
        </div>
      </div>

      {/* The last 3 lines of what it says and does, live (Phase 25, item 3.1). */}
      {liveNow && (
        <LiveConversation
          taskId={liveNow.taskId}
          sessionId={liveNow.sessionId}
          startedAt={liveNow.startedAt}
          running
          who={p.title}
          lines={3}
        />
      )}

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
        <dd>{chosenBy(p, role?.name ?? p.roleName)}</dd>
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

      {(actions.openPage || sessionId || watch) && (
        <Options>
          {watch && (
            <Option
              label="Watch"
              icon="file"
              ariaLabel={`Watch ${p.title}`}
              hint={
                p.status === "working"
                  ? "See the code it writes as it writes it, in the terminal panel. Read-only."
                  : "See the files it changed in its latest objective, in the terminal panel. Read-only."
              }
              onClick={watch}
            />
          )}
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
  // Files can go on an objective of a project's lead (its workers have the project's folder).
  const [files, dropTarget] = useObjectiveFiles(p.coordinatesProjectId !== null);
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
    const toSend = files.toSend();
    const failure = await actions.giveObjective(
      p.id,
      objective,
      toSend.length > 0 && p.coordinatesProjectId
        ? { projectId: p.coordinatesProjectId, files: toSend }
        : undefined,
    );
    setPending(false);
    setError(failure);
    if (failure === null) {
      setObjective("");
      files.clear();
      setSent(true);
    }
  };
  return (
    <form
      ref={dropTarget}
      {...files.dropProps}
      className="inspector__objective"
      aria-label="Give an objective"
      data-tour="give-objective"
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
      {p.coordinatesProjectId !== null && <ObjectiveFilesList state={files} />}
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
