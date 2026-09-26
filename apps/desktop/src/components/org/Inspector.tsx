import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import type {
  CandidateNote,
  OrgSnapshot,
  OversightRole,
  PositionInfo,
  PositionPatchInput,
  TaskBrief,
  WorkView,
  WorkerInfo,
} from "@plenipo/types";

import { getWork, toCommandError } from "../../api/commands";
import { TASK_STATE_LABEL } from "../../ledger/format";
import {
  OVERSIGHT_LABEL,
  OVERSIGHT_NOUN,
  STAFFING_LABEL,
  STATUS_LABEL,
  WORKER_STATE_LABEL,
  ago,
  plural,
  runtimeLabel,
  runtimeReady,
} from "../../org/format";
import { ORG_ID, OWNER_ID } from "../../org/layout";
import { workerStatus } from "../../org/nodes";
import {
  canTakeObjective,
  moveChoices,
  oversightOrder,
  oversightRefusal,
  positionMap,
} from "../../org/rules";
import { rankName, roleLabel, titlesOf, withArticle } from "../../org/titles";
import { Glyph } from "./Glyph";
import { StatusPill } from "./OrgNode";

const MAX_OBJECTIVE = 20_000;
const OWNER_VALUE = "__owner__";

/** What the inspector can ask the Organization view to do. */
export interface InspectorActions {
  /** Run a change; resolves with the refusal to show, or `null` once applied. */
  run: (work: () => Promise<OrgSnapshot>) => Promise<string | null>;
  giveObjective: (positionId: string, objective: string) => Promise<string | null>;
  hire: (reportsTo: string | null) => void;
  newDepartment: (reportsTo: string | null) => void;
  newProject: (departmentId: string | null) => void;
  newRole: () => void;
  editDepartment: (id: string) => void;
  editProject: (id: string) => void;
  rename: () => void;
  confirm: (request: {
    title: string;
    message: ReactNode;
    confirmLabel: string;
    work: () => Promise<OrgSnapshot>;
  }) => void;
  openSession: (sessionId: string) => void;
  openTask: (taskId: string) => void;
  api: {
    fill: (id: string) => Promise<OrgSnapshot>;
    vacate: (id: string) => Promise<OrgSnapshot>;
    update: (id: string, patch: PositionPatchInput) => Promise<OrgSnapshot>;
    move: (id: string, reportsTo: string | null) => Promise<OrgSnapshot>;
    archive: (id: string) => Promise<OrgSnapshot>;
    assign: (overseer: string, target: string, role: OversightRole) => Promise<OrgSnapshot>;
    endOversight: (id: string) => Promise<OrgSnapshot>;
    removeDepartment: (id: string) => Promise<OrgSnapshot>;
    archiveProject: (id: string) => Promise<OrgSnapshot>;
  };
}

interface Props {
  snapshot: OrgSnapshot;
  selectedId: string;
  revision: number;
  actions: InspectorActions;
  onSelect: (id: string) => void;
  onClose: () => void;
}

export function Inspector({ snapshot, selectedId, revision, actions, onSelect, onClose }: Props) {
  const position = snapshot.positions.find((p) => p.id === selectedId) ?? null;
  const worker = findWorker(snapshot, selectedId);
  let heading: string;
  let body: ReactNode;
  if (selectedId === OWNER_ID) {
    heading = "You";
    body = <OwnerPanel snapshot={snapshot} actions={actions} onSelect={onSelect} />;
  } else if (selectedId === ORG_ID) {
    heading = snapshot.name;
    body = <OrganizationPanel snapshot={snapshot} actions={actions} />;
  } else if (position) {
    heading = position.title;
    body = (
      <PositionPanel
        key={position.id}
        p={position}
        snapshot={snapshot}
        revision={revision}
        actions={actions}
        onSelect={onSelect}
      />
    );
  } else if (worker) {
    heading = "Worker";
    body = (
      <WorkerPanel
        worker={worker.worker}
        position={worker.position}
        snapshot={snapshot}
        actions={actions}
        onSelect={onSelect}
      />
    );
  } else {
    heading = "Gone";
    body = <p className="muted">This worker has finished and left the organization.</p>;
  }
  return (
    <aside className="inspector" aria-label={`Details: ${heading}`}>
      <header className="inspector__header">
        <h2>{heading}</h2>
        <button type="button" className="modal__close" aria-label="Close details" onClick={onClose}>
          ×
        </button>
      </header>
      <div className="inspector__body" data-canvas-scroll>
        {body}
      </div>
    </aside>
  );
}

function findWorker(
  snapshot: OrgSnapshot,
  id: string,
): { worker: WorkerInfo; position: PositionInfo } | null {
  if (!id.startsWith("worker:")) return null;
  const agent = id.slice("worker:".length);
  for (const position of snapshot.positions) {
    const worker = position.workers.find((w) => w.agentId === agent);
    if (worker) return { worker, position };
  }
  return null;
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="inspector__section">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

function Refusal({ error }: { error: string | null }) {
  return error ? (
    <p className="form-error" role="alert">
      {error}
    </p>
  ) : null;
}

function useRun(actions: InspectorActions) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const go = async (work: () => Promise<OrgSnapshot>) => {
    setPending(true);
    setError(null);
    const failure = await actions.run(work);
    setPending(false);
    setError(failure);
    return failure === null;
  };
  return { pending, error, go, setError };
}

// ---- Owner and organization ----------------------------------------------------------------

function OwnerPanel({
  snapshot,
  actions,
  onSelect,
}: {
  snapshot: OrgSnapshot;
  actions: InspectorActions;
  onSelect: (id: string) => void;
}) {
  const reports = snapshot.positions.filter((p) => p.active && p.reportsTo === null);
  const t = titlesOf(snapshot);
  return (
    <>
      <p className="muted">
        You run this organization as its {rankName(t, "owner")}. {rankName(t, "superintendent", 2)}{" "}
        and {rankName(t, "departmentManager", 2)} report to you; give them objectives and they hand
        the work down their teams.
      </p>
      <Section title="Reporting to you">
        {reports.length === 0 ? (
          <p className="muted">Nobody yet.</p>
        ) : (
          <ul className="inspector__list">
            {reports.map((p) => (
              <li key={p.id}>
                <button type="button" className="link" onClick={() => onSelect(p.id)}>
                  {p.title}
                </button>{" "}
                <StatusPill status={p.status} label={STATUS_LABEL[p.status]} />
              </li>
            ))}
          </ul>
        )}
      </Section>
      <div className="actions">
        <button type="button" className="button button--small" onClick={() => actions.hire(null)}>
          Hire {withArticle(rankName(t, "superintendent"))}
        </button>
        <button
          type="button"
          className="button button--small button--quiet"
          onClick={() => actions.newDepartment(null)}
        >
          New department
        </button>
      </div>
    </>
  );
}

function OrganizationPanel({
  snapshot,
  actions,
}: {
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  const s = snapshot.stats;
  return (
    <>
      <div className="actions">
        <button
          type="button"
          className="button button--small button--quiet"
          onClick={actions.rename}
        >
          Rename
        </button>
        <button
          type="button"
          className="button button--small button--quiet"
          onClick={actions.newRole}
        >
          New role
        </button>
      </div>
      <dl className="facts facts--compact">
        <Fact label="Departments" value={s.departments} />
        <Fact label="Projects" value={s.projects} />
        <Fact label="Positions" value={s.positions} />
        <Fact label="Staffed" value={s.staffed} />
        <Fact label="Vacant" value={s.vacant} />
        <Fact label="Live workers" value={s.activeWorkers} />
        <Fact label="Working" value={s.working} />
        <Fact label="Waiting" value={s.waiting} />
        <Fact label="Queued" value={s.queued} />
        <Fact label="Done (24 h)" value={s.completed24h} />
        <Fact label="Failed (24 h)" value={s.failed24h} />
      </dl>
      <Section title="AI tools">
        <ul className="inspector__list">
          {snapshot.runtimes.map((r) => (
            <li key={r.id}>
              {r.label}{" "}
              <span className={`pill ${r.ready ? "pill--ok" : "pill--warn"}`}>
                {r.ready ? "Ready" : "Not ready"}
              </span>
            </li>
          ))}
        </ul>
      </Section>
      {snapshot.notices.length > 0 && (
        <Section title="Notices">
          <ul className="inspector__list">
            {snapshot.notices.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </Section>
      )}
    </>
  );
}

function Fact({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

// ---- Positions ------------------------------------------------------------------------------

function PositionPanel({
  p,
  snapshot,
  revision,
  actions,
  onSelect,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  revision: number;
  actions: InspectorActions;
  onSelect: (id: string) => void;
}) {
  const byId = positionMap(snapshot);
  const supervisor = p.reportsTo ? byId.get(p.reportsTo) : null;
  const department = snapshot.departments.find((d) => d.id === p.departmentId) ?? null;
  const project = snapshot.projects.find((x) => x.id === p.projectId) ?? null;
  const role = snapshot.roles.find((r) => r.id === p.roleId);
  const t = titlesOf(snapshot);
  const status = useRun(actions);

  return (
    <>
      <div className="inspector__identity">
        <span className={`topo-node__glyph topo-node__glyph--${p.kind}`}>
          <Glyph name={role?.glyph ?? "worker"} size={20} />
        </span>
        <div>
          <div>
            {role ? roleLabel(t, role) : p.roleName} · {STAFFING_LABEL[p.staffing]}
          </div>
          <StatusPill status={p.status} label={STATUS_LABEL[p.status]} />
          {p.statusDetail && <p className="inspector__detail">{p.statusDetail}</p>}
        </div>
      </div>

      {!p.active ? (
        <p className="muted">
          Archived {p.archivedAt ? ago(p.archivedAt) : ""}. Its history remains in the Ledger.
        </p>
      ) : (
        <>
          {p.staffing === "persistent" && <ObjectivePanel p={p} actions={actions} />}
          {p.staffing === "onDemand" && (
            <p className="muted inspector__note">
              On call: its team&apos;s lead hands it tasks, and a new worker is brought in for each
              one.
            </p>
          )}
        </>
      )}

      <dl className="kv">
        <dt>Reports to</dt>
        <dd>
          {supervisor ? (
            <button type="button" className="link" onClick={() => onSelect(supervisor.id)}>
              {supervisor.title}
            </button>
          ) : (
            `You (${rankName(t, "owner")})`
          )}
        </dd>
        <dt>Rank</dt>
        <dd>{rankName(t, p.kind)}</dd>
        <dt>AI tool</dt>
        <dd>
          {p.runtimeId ? runtimeLabel(snapshot, p.runtimeId) : "None available now"}{" "}
          {p.runtimeId && !runtimeReady(snapshot, p.runtimeId) && (
            <span className="pill pill--warn">Not ready</span>
          )}
        </dd>
        <dt>Model</dt>
        <dd>{p.runtimeId ? (p.model ?? "The AI tool's default") : "—"}</dd>
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
        {p.agent && (
          <>
            <dt>Agent</dt>
            <dd>
              Hired {ago(p.agent.hiredAt)}
              {p.agent.sessionId && (
                <>
                  {" "}
                  ·{" "}
                  <button
                    type="button"
                    className="link"
                    onClick={() => actions.openSession(p.agent?.sessionId ?? "")}
                  >
                    Open conversation
                  </button>
                </>
              )}
            </dd>
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

      {p.active && p.route && <RouteSection p={p} snapshot={snapshot} />}

      {p.currentTask && (
        <Section title="Current objective">
          <TaskRow task={p.currentTask} onOpen={actions.openTask} />
        </Section>
      )}

      {p.staffing === "onDemand" && p.active && (
        <Section title={`Live workers (${p.workers.length})`}>
          {p.workers.length === 0 ? (
            <p className="muted">
              None right now. Workers appear here while they work and leave when done.
            </p>
          ) : (
            <ul className="inspector__list">
              {p.workers.map((w) => (
                <li key={w.agentId} className="inspector__worker">
                  <button
                    type="button"
                    className="link"
                    onClick={() => onSelect(`worker:${w.agentId}`)}
                  >
                    {w.objective || "(no objective)"}
                  </button>
                  <StatusPill status={workerStatus(w.state)} label={WORKER_STATE_LABEL[w.state]} />
                </li>
              ))}
            </ul>
          )}
        </Section>
      )}

      {p.active && (
        <OversightPanel p={p} snapshot={snapshot} actions={actions} onSelect={onSelect} />
      )}

      <WorkPanel positionId={p.id} revision={revision} onOpen={actions.openTask} />

      {p.headsDepartmentId && department && (
        <Section title={`Department: ${department.name}`}>
          {department.description && <p className="muted">{department.description}</p>}
          <p className="muted">
            {plural(department.projectIds.length, "project")}
            {department.active ? "" : " · inactive"}
          </p>
          <div className="actions">
            <button
              type="button"
              className="button button--small button--quiet"
              onClick={() => actions.editDepartment(department.id)}
            >
              Edit department
            </button>
            <button
              type="button"
              className="button button--small button--quiet"
              onClick={() => actions.newProject(department.id)}
            >
              New project
            </button>
            <button
              type="button"
              className="button button--small button--danger"
              onClick={() =>
                actions.confirm({
                  title: `Remove ${department.name}?`,
                  message: (
                    <p>
                      The department is deleted and {p.title} is archived. A department with
                      projects cannot be removed; archive its projects first.
                    </p>
                  ),
                  confirmLabel: "Remove department",
                  work: () => actions.api.removeDepartment(department.id),
                })
              }
            >
              Remove department
            </button>
          </div>
        </Section>
      )}

      {p.coordinatesProjectId && project && (
        <Section title={`Project: ${project.name}`}>
          {project.description && <p className="muted">{project.description}</p>}
          <dl className="kv">
            <dt>Allowed AI tools</dt>
            <dd>
              {project.allowedRuntimes.length === 0
                ? "None — its team cannot take work"
                : project.allowedRuntimes.map((r) => runtimeLabel(snapshot, r)).join(", ")}
            </dd>
            {project.repositoryUrl && (
              <>
                <dt>Repository</dt>
                <dd>{project.repositoryUrl}</dd>
              </>
            )}
            {project.localPath && (
              <>
                <dt>Local folder</dt>
                <dd>
                  {project.localPath} <span className="pill">Recorded only</span>
                </dd>
              </>
            )}
            {project.capabilityProfile && (
              <>
                <dt>Capabilities</dt>
                <dd>
                  {project.capabilityProfile} <span className="pill">Recorded only</span>
                </dd>
              </>
            )}
          </dl>
          {project.active && (
            <div className="actions">
              <button
                type="button"
                className="button button--small button--quiet"
                onClick={() => actions.editProject(project.id)}
              >
                Edit project
              </button>
              <button
                type="button"
                className="button button--small button--danger"
                onClick={() =>
                  actions.confirm({
                    title: `Archive ${project.name}?`,
                    message: (
                      <p>
                        The project and its whole team are archived: {p.title} and every position
                        under it. This is refused while any of them has unfinished work. History
                        remains in the Ledger.
                      </p>
                    ),
                    confirmLabel: "Archive project",
                    work: () => actions.api.archiveProject(project.id),
                  })
                }
              >
                Archive project
              </button>
            </div>
          )}
        </Section>
      )}

      {p.active && (
        <ManagePanel
          key={`${p.title}|${p.automatic ? "auto" : `${p.runtimeId}|${p.model ?? ""}`}`}
          p={p}
          snapshot={snapshot}
          actions={actions}
          run={status}
        />
      )}
    </>
  );
}

function ObjectivePanel({ p, actions }: { p: PositionInfo; actions: InspectorActions }) {
  const [objective, setObjective] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sent, setSent] = useState(false);
  if (!p.agent) {
    return (
      <p className="hint">
        {p.title} is vacant. Hire an agent into it (below) to give it objectives.
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
      <label className="field">
        <span>Objective for {p.title}</span>
        <textarea
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
      </label>
      {busy && <p className="muted">Busy with its current objective; wait until it finishes.</p>}
      {sent && (
        <p className="status status--ok" role="status">
          Objective given. Follow it here or in Workers.
        </p>
      )}
      <Refusal error={error} />
      <button
        type="submit"
        className="button"
        disabled={pending || busy || objective.trim() === ""}
      >
        {pending ? "Sending…" : "Give objective"}
      </button>
      <p className="muted inspector__note">
        {p.agent.sessionId ? "Continues its conversation." : "Starts its first conversation."}
      </p>
    </form>
  );
}

function OversightPanel({
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
  const oversees = snapshot.oversight.filter((o) => o.overseerId === p.id);
  const overseenBy = snapshot.oversight.filter((o) => o.targetId === p.id);
  const glyph = snapshot.roles.find((r) => r.id === p.roleId)?.glyph ?? "";
  const order = oversightOrder(glyph);
  const [role, setRole] = useState<OversightRole>(order[0] ?? "review");
  const targets = snapshot.positions.filter(
    (t) => t.active && oversightRefusal(snapshot, p, t, role) === null,
  );
  const [target, setTarget] = useState("");
  const { pending, error, go } = useRun(actions);
  const chosen = targets.find((t) => t.id === target) ?? targets[0] ?? null;
  const title = (id: string) => byId.get(id)?.title ?? "a former position";

  const end = (id: string) => void go(() => actions.api.endOversight(id));
  if (p.staffing === "persistent" && overseenBy.length === 0 && oversees.length === 0) {
    return null;
  }
  return (
    <Section title="Oversight">
      {oversees.length > 0 && (
        <ul className="inspector__list">
          {oversees.map((o) => (
            <li key={o.id}>
              {OVERSIGHT_LABEL[o.role]} for{" "}
              <button type="button" className="link" onClick={() => onSelect(o.targetId)}>
                {title(o.targetId)}
              </button>
              &apos;s team{" "}
              <button
                type="button"
                className="button button--small button--quiet"
                disabled={pending}
                onClick={() => end(o.id)}
              >
                End
              </button>
            </li>
          ))}
        </ul>
      )}
      {overseenBy.length > 0 && (
        <ul className="inspector__list">
          {overseenBy.map((o) => (
            <li key={o.id}>
              <button type="button" className="link" onClick={() => onSelect(o.overseerId)}>
                {title(o.overseerId)}
              </button>{" "}
              is this team&apos;s {OVERSIGHT_NOUN[o.role]}{" "}
              <button
                type="button"
                className="button button--small button--quiet"
                disabled={pending}
                onClick={() => end(o.id)}
              >
                End
              </button>
            </li>
          ))}
        </ul>
      )}
      {p.staffing === "onDemand" && (
        <form
          className="inspector__assign"
          aria-label="Assign oversight"
          onSubmit={(e) => {
            e.preventDefault();
            if (chosen) void go(() => actions.api.assign(p.id, chosen.id, role));
          }}
        >
          <label className="field">
            <span>Assign as</span>
            <select value={role} onChange={(e) => setRole(e.target.value as OversightRole)}>
              {order.map((r) => (
                <option key={r} value={r}>
                  {OVERSIGHT_LABEL[r]}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>Of the team led by</span>
            <select
              value={chosen?.id ?? ""}
              onChange={(e) => setTarget(e.target.value)}
              disabled={targets.length === 0}
            >
              {targets.length === 0 && <option value="">No team to oversee</option>}
              {targets.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.title}
                </option>
              ))}
            </select>
          </label>
          <button type="submit" className="button button--small" disabled={pending || !chosen}>
            Assign
          </button>
        </form>
      )}
      <Refusal error={error} />
    </Section>
  );
}

function ManagePanel({
  p,
  snapshot,
  actions,
  run,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
  run: ReturnType<typeof useRun>;
}) {
  const byId = positionMap(snapshot);
  const t = titlesOf(snapshot);
  const choices = moveChoices(snapshot, p);
  const [moveTo, setMoveTo] = useState<string>("");
  const [title, setTitle] = useState(p.title);
  // "" means automatic: the role's model choices pick the AI tool and model.
  const fixedRuntime = p.automatic ? "" : (p.runtimeId ?? "");
  const fixedModel = p.automatic ? "" : (p.model ?? "");
  const [runtimeId, setRuntimeId] = useState(fixedRuntime);
  const [model, setModel] = useState(fixedModel);
  const { pending, error, go } = run;
  const leads = p.staffing === "persistent";
  const automatic = runtimeId === "";

  const save = (e: FormEvent) => {
    e.preventDefault();
    const patch: PositionPatchInput = {};
    if (title.trim() !== p.title) patch.title = title.trim();
    if (runtimeId !== fixedRuntime) patch.runtimeId = runtimeId;
    if (!automatic && model.trim() !== fixedModel) patch.model = model.trim();
    if (Object.keys(patch).length > 0) void go(() => actions.api.update(p.id, patch));
  };
  const replacesAgent =
    p.agent !== null && (runtimeId !== fixedRuntime || (!automatic && model.trim() !== fixedModel));

  return (
    <Section title="Manage">
      <div className="actions">
        {leads && (
          <button type="button" className="button button--small" onClick={() => actions.hire(p.id)}>
            Hire into team
          </button>
        )}
        {leads && p.agent === null && (
          <button
            type="button"
            className="button button--small"
            disabled={pending}
            onClick={() => void go(() => actions.api.fill(p.id))}
          >
            Hire an agent
          </button>
        )}
        {leads && p.agent !== null && (
          <button
            type="button"
            className="button button--small button--quiet"
            onClick={() =>
              actions.confirm({
                title: `Let ${p.title}'s agent go?`,
                message: (
                  <p>
                    The position stays, vacant; its agent retires and its conversation ends. Its
                    history remains in the Ledger. This is refused while it has unfinished work.
                  </p>
                ),
                confirmLabel: "Let agent go",
                work: () => actions.api.vacate(p.id),
              })
            }
          >
            Let agent go
          </button>
        )}
        {!p.headsDepartmentId && !p.coordinatesProjectId && (
          <button
            type="button"
            className="button button--small button--danger"
            onClick={() =>
              actions.confirm({
                title: `Archive ${p.title}?`,
                message: (
                  <p>
                    The position is archived and its agent retires; oversight assignments end. It
                    must not lead anyone or have unfinished work. History remains in the Ledger.
                  </p>
                ),
                confirmLabel: "Archive position",
                work: () => actions.api.archive(p.id),
              })
            }
          >
            Archive
          </button>
        )}
      </div>

      <form
        className="inspector__move"
        aria-label="Change who it reports to"
        onSubmit={(e) => {
          e.preventDefault();
          const to = moveTo === OWNER_VALUE ? null : moveTo;
          if (moveTo) void go(() => actions.api.move(p.id, to));
        }}
      >
        <label className="field">
          <span>Move to report to</span>
          <select
            value={moveTo}
            onChange={(e) => setMoveTo(e.target.value)}
            disabled={choices.length === 0}
          >
            <option value="">
              {choices.length === 0 ? "No other position fits" : "Choose who it reports to…"}
            </option>
            {choices.map((id) =>
              id === null ? (
                <option key={OWNER_VALUE} value={OWNER_VALUE}>
                  You ({rankName(t, "owner")})
                </option>
              ) : (
                <option key={id} value={id}>
                  {byId.get(id)?.title ?? id}
                </option>
              ),
            )}
          </select>
        </label>
        <button type="submit" className="button button--small" disabled={pending || moveTo === ""}>
          Move
        </button>
      </form>

      <details className="advanced">
        <summary>Edit title or AI model</summary>
        <form aria-label="Edit position" onSubmit={save}>
          <label className="field">
            <span>Title</span>
            <input value={title} maxLength={120} onChange={(e) => setTitle(e.target.value)} />
          </label>
          <label className="field">
            <span>AI tool</span>
            <select value={runtimeId} onChange={(e) => setRuntimeId(e.target.value)}>
              <option value="">Automatic (the role&apos;s model choices)</option>
              {snapshot.runtimes.map((r) => (
                <option key={r.id} value={r.id}>
                  {r.label}
                  {r.ready ? "" : " (not ready)"}
                </option>
              ))}
            </select>
          </label>
          {automatic ? (
            <p className="hint">
              Plenipo picks the AI tool and model from {p.roleName}&apos;s model choices in Settings
              → AI models, and says why.
            </p>
          ) : (
            <label className="field">
              <span>Model</span>
              <input
                value={model}
                maxLength={100}
                placeholder="The AI tool's default"
                onChange={(e) => setModel(e.target.value)}
              />
            </label>
          )}
          {replacesAgent && (
            <p className="hint">
              Changing the AI tool or model hires a new agent for this position; the current one
              retires and its conversation ends.
            </p>
          )}
          <button type="submit" className="button button--small" disabled={pending}>
            Save changes
          </button>
        </form>
      </details>
      <Refusal error={error} />
    </Section>
  );
}

// ---- Work -----------------------------------------------------------------------------------

type WorkTab = "running" | "waiting" | "queued" | "recent" | "team";
const WORK_TABS: { id: WorkTab; label: string }[] = [
  { id: "running", label: "Running" },
  { id: "waiting", label: "Waiting" },
  { id: "queued", label: "Queued" },
  { id: "recent", label: "Recent" },
  { id: "team", label: "Team" },
];

function WorkPanel({
  positionId,
  revision,
  onOpen,
}: {
  positionId: string;
  revision: number;
  onOpen: (taskId: string) => void;
}) {
  const [work, setWork] = useState<WorkView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<WorkTab>("running");
  useEffect(() => {
    let cancelled = false;
    const t = setTimeout(() => {
      getWork(positionId)
        .then((w) => {
          if (!cancelled) {
            setWork(w);
            setError(null);
          }
        })
        .catch((reason: unknown) => {
          if (!cancelled) setError(toCommandError(reason).message);
        });
    }, 100);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [positionId, revision]);

  const current = work && work.positionId === positionId ? work : null;
  const list: TaskBrief[] = current ? current[tab] : [];
  return (
    <Section title="Work">
      <div className="tabs tabs--small" role="tablist" aria-label="Work">
        {WORK_TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            className="tabs__tab"
            aria-selected={tab === t.id}
            onClick={() => setTab(t.id)}
          >
            {t.label}
            {current && current[t.id].length > 0 ? ` (${current[t.id].length})` : ""}
          </button>
        ))}
      </div>
      <div role="tabpanel" aria-label={`${tab} work`}>
        {error ? (
          <p className="form-error">{error}</p>
        ) : !current ? (
          <p className="muted">Loading…</p>
        ) : list.length === 0 ? (
          <p className="muted">Nothing here.</p>
        ) : (
          <ul className="inspector__tasks">
            {list.map((t) => (
              <li key={t.id}>
                <TaskRow task={t} onOpen={onOpen} showOwner={tab === "team"} />
              </li>
            ))}
          </ul>
        )}
      </div>
    </Section>
  );
}

function TaskRow({
  task,
  onOpen,
  showOwner = false,
}: {
  task: TaskBrief;
  onOpen: (taskId: string) => void;
  showOwner?: boolean;
}) {
  return (
    <div className="inspector__task">
      <button type="button" className="link" onClick={() => onOpen(task.id)}>
        {task.objective || "(no objective)"}
      </button>
      <span className={`badge badge--task-${task.state}`}>{TASK_STATE_LABEL[task.state]}</span>
      <span className="muted inspector__task-meta">
        {showOwner && task.positionTitle ? `${task.positionTitle} · ` : ""}
        {ago(task.completedAt ?? task.startedAt ?? task.createdAt)}
      </span>
    </div>
  );
}

// ---- Routing (Phase 6) ----------------------------------------------------------------------

const VERDICT_TEXT: Record<CandidateNote["verdict"], string> = {
  chosen: "chosen",
  skipped: "skipped",
  notNeeded: "not needed",
};

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
          position would get: {route.choice?.label ?? "no model now"}.
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
      {p.automatic && (
        <p className="muted inspector__note">Change the model choices in Settings → AI models.</p>
      )}
    </Section>
  );
}

// ---- Workers --------------------------------------------------------------------------------

function WorkerPanel({
  worker,
  position,
  snapshot,
  actions,
  onSelect,
}: {
  worker: WorkerInfo;
  position: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
  onSelect: (id: string) => void;
}) {
  return (
    <>
      <p className="inspector__objective-text">{worker.objective || "(no objective)"}</p>
      <StatusPill status={workerStatus(worker.state)} label={WORKER_STATE_LABEL[worker.state]} />
      <dl className="kv">
        <dt>Position</dt>
        <dd>
          <button type="button" className="link" onClick={() => onSelect(position.id)}>
            {position.title}
          </button>
        </dd>
        <dt>AI tool</dt>
        <dd>
          {runtimeLabel(snapshot, worker.runtimeId)}
          {worker.model ? ` · ${worker.model}` : ""}
        </dd>
        {worker.routing && (
          <>
            <dt>Why</dt>
            <dd>{worker.routing}</dd>
          </>
        )}
        <dt>Brought in</dt>
        <dd>{ago(worker.spawnedAt)}</dd>
        {worker.startedAt && (
          <>
            <dt>Started</dt>
            <dd>{ago(worker.startedAt)}</dd>
          </>
        )}
      </dl>
      <p className="muted inspector__note">
        On-call worker: it leaves the organization when its task is done; its history stays in the
        Ledger.
      </p>
      <div className="actions">
        <button
          type="button"
          className="button button--small button--quiet"
          onClick={() => actions.openTask(worker.taskId)}
        >
          Open task
        </button>
        {worker.sessionId && (
          <button
            type="button"
            className="button button--small button--quiet"
            onClick={() => actions.openSession(worker.sessionId ?? "")}
          >
            Open conversation
          </button>
        )}
      </div>
    </>
  );
}
