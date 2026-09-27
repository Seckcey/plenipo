import { useEffect, useState, type FormEvent } from "react";
import type { OrgSnapshot, PositionInfo, ProjectInfo, Workspace } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { giveObjective, removeWorkspace, setUpDevelopment, toCommandError } from "../api/commands";
import { ObjectiveResult } from "../components/ObjectiveResult";
import { ConfirmDialog } from "../components/org/Modal";
import { SetUpDevelopmentDialog } from "../components/org/OrgDialogs";
import { TASK_TONE } from "../components/tones";
import type { Go } from "../components/views";
import { POSITION_STATUS } from "../org/cards";
import { STATUS_LABEL, WORKER_STATE_LABEL, ago, plural } from "../org/format";
import { canTakeObjective, positionMap } from "../org/rules";
import { rankName, titlesOf } from "../org/titles";
import { useOrganization } from "../org/useOrganization";
import { useObjectiveReport, useProjectWork } from "../projects/useProjectWork";

const MAX_OBJECTIVE = 20_000;

function firstLine(text: string, max = 120): string {
  const line = text.trim().split("\n")[0] ?? "";
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/** Who can be given an objective for `project`: its department's head (who hands it on) and
 * its own lead — full-time positions only. */
function objectiveTakers(snapshot: OrgSnapshot, project: ProjectInfo): PositionInfo[] {
  const byId = positionMap(snapshot);
  const department = snapshot.departments.find((d) => d.id === project.departmentId);
  return [department?.headPositionId, project.coordinatorPositionId]
    .map((id) => (id ? byId.get(id) : undefined))
    .filter((p): p is PositionInfo => p !== undefined && p.active && p.staffing === "persistent");
}

function ObjectiveForm({
  snapshot,
  project,
  onGiven,
}: {
  snapshot: OrgSnapshot;
  project: ProjectInfo;
  onGiven: (taskId: string | null) => void;
}) {
  const t = titlesOf(snapshot);
  const takers = objectiveTakers(snapshot, project);
  const [takerId, setTakerId] = useState(
    () => (takers.find(canTakeObjective) ?? takers[0])?.id ?? "",
  );
  const [objective, setObjective] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sent, setSent] = useState<string | null>(null);
  const taker = takers.find((p) => p.id === takerId) ?? null;

  if (takers.length === 0) {
    return (
      <p className="hint">
        {project.name} has no full-time {rankName(t, "projectCoordinator")} to give objectives to.
      </p>
    );
  }
  const busy = taker !== null && (taker.status === "working" || taker.status === "waiting");
  const vacant = taker !== null && taker.agent === null;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!taker) return;
    setPending(true);
    setError(null);
    setSent(null);
    try {
      const detail = await giveObjective(taker.id, objective, project.id);
      const turn = [...detail.turns].sort((a, b) => b.number - a.number)[0];
      setObjective("");
      setSent(taker.title);
      onGiven(turn?.taskId ?? null);
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setPending(false);
    }
  };

  return (
    <form
      className="objective-form"
      aria-label="Give an objective"
      onSubmit={(e) => void submit(e)}
    >
      <label className="field">
        <span>Objective for {project.name}</span>
        <textarea
          value={objective}
          rows={3}
          maxLength={MAX_OBJECTIVE}
          placeholder="What should the team get done? Say how you will know it is done."
          onChange={(e) => {
            setObjective(e.target.value);
            setSent(null);
          }}
        />
      </label>
      <div className="objective-form__row">
        <label className="field">
          <span>Give it to</span>
          <select value={takerId} onChange={(e) => setTakerId(e.target.value)}>
            {takers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.title} ({rankName(t, p.kind)}){p.agent ? "" : " — vacant"}
              </option>
            ))}
          </select>
        </label>
        <Button
          type="submit"
          variant="primary"
          disabled={pending || busy || vacant || objective.trim() === ""}
        >
          {pending ? "Sending…" : "Give objective"}
        </Button>
      </div>
      {taker && (
        <p className="muted">
          {taker.coordinatesProjectId === project.id
            ? `${taker.title} runs it with the project's team.`
            : `${taker.title} hands it to the project's ${rankName(t, "projectCoordinator")} and reports back to you.`}
          {project.localPath && project.branchPerObjective
            ? " The team works on a new branch, in its own copy of the project folder."
            : ""}
        </p>
      )}
      {vacant && <p className="hint">{taker?.title} is vacant. Hire an agent into it first.</p>}
      {busy && (
        <p className="muted">
          {taker?.title} is busy with its current objective; wait until it finishes.
        </p>
      )}
      {sent && (
        <p className="status status--ok" role="status">
          Objective given to {sent}. Its result appears below as the team works.
        </p>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

function WorkingCopies({
  copies,
  onRemove,
}: {
  copies: Workspace[];
  onRemove: (copy: Workspace) => void;
}) {
  if (copies.length === 0) {
    return (
      <p className="muted">
        None yet. Each objective gets its own working copy of the project folder when the folder is
        a git repository.
      </p>
    );
  }
  return (
    <table className="table" aria-label="Working copies">
      <thead>
        <tr>
          <th scope="col">Branch</th>
          <th scope="col">Changes</th>
          <th scope="col">Folder</th>
          <th scope="col">
            <span className="visually-hidden">Actions</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {copies.map((w) => (
          <tr key={w.id}>
            <th scope="row">
              <code>{w.branch}</code>
              <span className="table__sub">
                {w.baseRef ? `from ${w.baseRef} · ` : ""}made {ago(w.createdAt)}
              </span>
            </th>
            <td>
              {plural(w.facts.commits.length, "commit")}
              {w.facts.uncommitted > 0 && (
                <span className="table__sub">
                  {plural(w.facts.uncommitted, "file")} not committed
                </span>
              )}
              <span className="table__sub">{w.facts.pushed ? "Pushed" : "Not pushed"}</span>
            </td>
            <td>
              {w.state === "removed" ? (
                <span className="muted">Removed; the branch stays</span>
              ) : (
                <code>{w.path}</code>
              )}
            </td>
            <td>
              {w.state === "active" && (
                <Button
                  variant="quiet"
                  size="sm"
                  aria-label={`Remove the working copy of ${w.branch}`}
                  onClick={() => onRemove(w)}
                >
                  Remove
                </Button>
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function ProjectDetail({
  snapshot,
  project,
  onOpenTask,
  onOpenApprovals,
  onOpenPage,
}: {
  snapshot: OrgSnapshot;
  project: ProjectInfo;
  onOpenTask: (taskId: string) => void;
  onOpenApprovals: () => void;
  onOpenPage?: Go | undefined;
}) {
  const t = titlesOf(snapshot);
  const { work, error, apply } = useProjectWork(project.id);
  const [selected, setSelected] = useState<string | null>(null);
  const [removing, setRemoving] = useState<Workspace | null>(null);
  const objectives = work?.objectives ?? [];
  const shown = selected ?? objectives[0]?.rootTaskId ?? null;
  const { report, error: reportError } = useObjectiveReport(shown);
  const byId = positionMap(snapshot);
  const lead = project.coordinatorPositionId ? byId.get(project.coordinatorPositionId) : undefined;
  const department = snapshot.departments.find((d) => d.id === project.departmentId);

  return (
    <div className="project">
      <header className="detail__header">
        <div>
          <h2>{project.name}</h2>
          {project.description && <p className="muted">{project.description}</p>}
        </div>
        {onOpenPage && (
          <Button
            size="sm"
            variant="quiet"
            icon="chevronRight"
            onClick={() => onOpenPage({ view: "project", id: project.id })}
          >
            Open the project's page
          </Button>
        )}
      </header>
      <dl className="kv" aria-label="About the project">
        <dt>Department</dt>
        <dd>{department?.name ?? "—"}</dd>
        <dt>{rankName(t, "projectCoordinator")}</dt>
        <dd>{lead ? `${lead.title} · ${STATUS_LABEL[lead.status]}` : "None"}</dd>
        <dt>Project folder</dt>
        <dd>{project.localPath ? <code>{project.localPath}</code> : "None (no file tools)"}</dd>
        <dt>Repository</dt>
        <dd>{project.repositoryUrl ? <code>{project.repositoryUrl}</code> : "—"}</dd>
        <dt>Branches</dt>
        <dd>
          {project.branchPerObjective
            ? "A new branch and working copy for each objective"
            : "Workers change the project folder itself"}
        </dd>
      </dl>

      <ObjectiveForm
        key={project.id}
        snapshot={snapshot}
        project={project}
        onGiven={(taskId) => {
          if (taskId) setSelected(taskId);
        }}
      />

      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}

      <h3>Objectives</h3>
      {objectives.length === 0 ? (
        <p className="muted">No objectives yet.</p>
      ) : (
        <ul className="objectives" aria-label="Objectives">
          {objectives.map((o) => (
            <li key={o.rootTaskId}>
              <button
                type="button"
                className="execution"
                aria-current={o.rootTaskId === shown ? "true" : undefined}
                onClick={() => setSelected(o.rootTaskId)}
              >
                <span>{firstLine(o.objective) || "(no objective)"}</span>
                <StatusPill status={TASK_TONE[o.state]} label={WORKER_STATE_LABEL[o.state]} />
                <span className="execution__meta">
                  {o.positionTitle ? `${o.positionTitle} · ` : ""}
                  {ago(o.createdAt)} · {plural(o.tasks, "task")}
                  {o.active > 0 ? ` · ${o.active} going` : ""}
                  {o.failed > 0 ? ` · ${o.failed} failed` : ""}
                  {o.waitingApprovals > 0
                    ? ` · ${plural(o.waitingApprovals, "approval")} waiting`
                    : ""}
                  {o.branch ? ` · ${o.branch}` : ""}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}

      {reportError && (
        <p className="status status--error" role="alert">
          {reportError}
        </p>
      )}
      {report && report.rootTaskId === shown && (
        <>
          {onOpenPage && (
            <div className="actions">
              <Button
                size="sm"
                variant="quiet"
                icon="chevronRight"
                onClick={() => onOpenPage({ view: "task", id: report.rootTaskId })}
              >
                Open the objective's page
              </Button>
            </div>
          )}
          <ObjectiveResult
            report={report}
            onOpenTask={onOpenTask}
            onOpenApprovals={onOpenApprovals}
          />
        </>
      )}

      <h3>Working copies</h3>
      <WorkingCopies copies={work?.workingCopies ?? []} onRemove={setRemoving} />

      {removing && (
        <ConfirmDialog
          title="Remove working copy"
          message={
            <>
              <p>
                Remove the folder <code>{removing.path}</code>? The branch{" "}
                <code>{removing.branch}</code> stays in the repository, with its commits.
              </p>
              {removing.facts.uncommitted > 0 && (
                <p className="hint">
                  {plural(removing.facts.uncommitted, "file")} changed but not committed will be
                  lost.
                </p>
              )}
            </>
          }
          confirmLabel="Remove"
          danger
          onCancel={() => setRemoving(null)}
          onConfirm={async () => {
            try {
              apply(await removeWorkspace(removing.id));
              setRemoving(null);
              return null;
            } catch (reason) {
              return toCommandError(reason).message;
            }
          }}
        />
      )}
    </div>
  );
}

/**
 * Projects (Phase 8): give a project an objective and follow what its team does — who worked on
 * it and on which models, the files changed, the tests run, reviews, the branch and pull
 * request, and approvals still needed.
 */
export function ProjectsView({
  onOpenTask,
  onOpenApprovals,
  onOpenPage,
  focusId = null,
  onFocusHandled,
}: {
  onOpenTask: (taskId: string) => void;
  onOpenApprovals: () => void;
  /** Opens a project's or a task's page (Phase 12). */
  onOpenPage?: Go | undefined;
  /** A project to open (chosen in the top bar's "Showing" picker). */
  focusId?: string | null;
  onFocusHandled?: () => void;
}) {
  const { snapshot, status, error, apply } = useOrganization();
  const [selected, setSelected] = useState<string | null>(focusId);
  // A project chosen in the top bar opens here (the same arrival pattern as the Organization).
  const [arrived, setArrived] = useState<string | null>(null);
  // Once the request is handled (cleared), the same place can be asked for again.
  if (!focusId && arrived !== null) setArrived(null);
  if (focusId && arrived !== focusId) {
    setArrived(focusId);
    setSelected(focusId);
  }
  useEffect(() => {
    if (focusId && arrived === focusId) onFocusHandled?.();
  }, [focusId, arrived, onFocusHandled]);
  const [settingUp, setSettingUp] = useState(false);
  const projects = (snapshot?.projects ?? []).filter((p) => p.active);
  const project = projects.find((p) => p.id === selected) ?? projects[0] ?? null;

  const t = snapshot ? titlesOf(snapshot) : null;

  return (
    <section className="view projects-view">
      <div className="section-header">
        <div>
          <h1>Projects</h1>
          <p className="view__lead">
            Give a project an objective and follow its team&apos;s work: who did what on which
            model, the files changed, tests, reviews, the branch, and approvals still needed.
          </p>
        </div>
        {snapshot && (
          <Button variant="primary" onClick={() => setSettingUp(true)}>
            Set up a Development project
          </Button>
        )}
      </div>

      {status === "loading" && <p className="status">Loading projects…</p>}
      {status === "error" && (
        <p className="status status--error" role="alert">
          Could not load the organization: {error}
        </p>
      )}

      {snapshot && projects.length === 0 && (
        <div className="empty">
          <h2>No projects yet</h2>
          <p className="muted">
            Set up a Development project: Plenipo creates the Development department with its{" "}
            {t ? rankName(t, "superintendent") : "VP"}, the project with its{" "}
            {t ? rankName(t, "projectCoordinator") : "Supervisor"}, and a team of a developer, a
            code reviewer, a QA engineer, and a documentation writer. Or create a project on the
            Organization page.
          </p>
        </div>
      )}

      {snapshot && project && (
        <div className="split">
          <div className="split__list">
            <h2>Projects</h2>
            <ul className="objectives" aria-label="Projects">
              {projects.map((p) => {
                const lead = p.coordinatorPositionId
                  ? snapshot.positions.find((x) => x.id === p.coordinatorPositionId)
                  : undefined;
                const department = snapshot.departments.find((d) => d.id === p.departmentId);
                return (
                  <li key={p.id}>
                    <button
                      type="button"
                      className="execution"
                      aria-current={p.id === project.id ? "true" : undefined}
                      onClick={() => setSelected(p.id)}
                    >
                      <span>{p.name}</span>
                      {lead && (
                        <StatusPill
                          status={POSITION_STATUS[lead.status]}
                          label={STATUS_LABEL[lead.status]}
                        />
                      )}
                      <span className="execution__meta">
                        {department?.name ?? "No department"}
                        {lead ? ` · ${lead.title}` : ""}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
          <ProjectDetail
            key={project.id}
            snapshot={snapshot}
            project={project}
            onOpenTask={onOpenTask}
            onOpenApprovals={onOpenApprovals}
            onOpenPage={onOpenPage}
          />
        </div>
      )}

      {settingUp && snapshot && (
        <SetUpDevelopmentDialog
          snapshot={snapshot}
          onCancel={() => setSettingUp(false)}
          onSubmit={async (input) => {
            try {
              const next = await setUpDevelopment(input);
              apply(next);
              setSettingUp(false);
              setSelected(next.projects.find((p) => p.name === input.project.name)?.id ?? null);
              return null;
            } catch (reason) {
              return toCommandError(reason).message;
            }
          }}
        />
      )}
    </section>
  );
}
