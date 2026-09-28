import { useState } from "react";
import type { OrgSnapshot, PositionStatus, SavedAgentInfo } from "@plenipo/types";
import { Button, StatusPill, Tabs } from "@plenipo/ui";

import type { ArchivedKind } from "../../api/commands";
import { POSITION_STATUS } from "../../org/cards";
import {
  archivedCount,
  archivedItems,
  archivedWithLine,
  experienceLine,
  liveDepartment,
  liveProject,
} from "../../org/control";
import {
  STAFFING_LABEL,
  STATUS_LABEL,
  ago,
  plural,
  positionToolLabel,
  runtimeLabel,
} from "../../org/format";
import { positionMap } from "../../org/rules";
import { positionSearchText } from "../../org/search";
import { rankName, titlesOf } from "../../org/titles";

type Tab = "positions" | "departments" | "projects" | "archived" | "workforce";

/** What the Archived and Workforce tabs can ask the Organization view to do. */
export interface DirectoryActions {
  bringBack: (kind: ArchivedKind, id: string, name: string) => void;
  deleteForGood: (kind: ArchivedKind, id: string) => void;
  saveToWorkforce: (positionId: string, title: string) => void;
  hireSaved: (saved: SavedAgentInfo) => void;
  deleteSaved: (saved: SavedAgentInfo) => void;
}

/** The organization as tables: positions, departments, projects, what is archived, and your
 * Workforce, filterable. */
export function Directory({
  snapshot,
  query,
  selectedId,
  onSelect,
  actions,
}: {
  snapshot: OrgSnapshot;
  query: string;
  selectedId: string | null;
  onSelect: (id: string) => void;
  actions: DirectoryActions;
}) {
  const [tab, setTab] = useState<Tab>("positions");
  const [department, setDepartment] = useState("");
  const [status, setStatus] = useState<PositionStatus | "">("");
  const byId = positionMap(snapshot);
  const t = titlesOf(snapshot);
  const q = query.trim().toLowerCase();
  const title = (id: string | null) =>
    id ? (byId.get(id)?.title ?? "—") : `You (${rankName(t, "owner")})`;

  const positions = snapshot.positions.filter(
    (p) =>
      p.active &&
      (department === "" || p.departmentId === department) &&
      (status === "" || p.status === status) &&
      (q === "" || positionSearchText(snapshot, p).includes(q)),
  );
  const departments = snapshot.departments.filter(
    (d) =>
      liveDepartment(d) && (q === "" || `${d.name} ${d.description}`.toLowerCase().includes(q)),
  );
  const projects = snapshot.projects.filter(
    (p) => liveProject(p) && (q === "" || `${p.name} ${p.description}`.toLowerCase().includes(q)),
  );
  const departmentCount = snapshot.departments.filter(liveDepartment).length;
  const projectCount = snapshot.projects.filter(liveProject).length;

  return (
    <section className="directory" aria-label="Organization directory" data-canvas-scroll>
      <Tabs<Tab>
        label="Directory"
        value={tab}
        onChange={setTab}
        tabs={[
          { value: "positions", label: `Positions (${snapshot.stats.positions})` },
          { value: "departments", label: `Departments (${departmentCount})` },
          { value: "projects", label: `Projects (${projectCount})` },
          { value: "archived", label: `Archived (${archivedCount(snapshot)})` },
          { value: "workforce", label: `Workforce (${snapshot.workforce.length})` },
        ]}
      />

      {tab === "positions" && (
        <>
          <div className="directory__filters">
            <label className="field field--inline">
              <span>Department</span>
              <select value={department} onChange={(e) => setDepartment(e.target.value)}>
                <option value="">All</option>
                {snapshot.departments.filter(liveDepartment).map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.name}
                  </option>
                ))}
              </select>
            </label>
            <label className="field field--inline">
              <span>Status</span>
              <select
                value={status}
                onChange={(e) => setStatus(e.target.value as PositionStatus | "")}
              >
                <option value="">All</option>
                {(Object.keys(STATUS_LABEL) as PositionStatus[])
                  .filter((s) => s !== "archived")
                  .map((s) => (
                    <option key={s} value={s}>
                      {STATUS_LABEL[s]}
                    </option>
                  ))}
              </select>
            </label>
          </div>
          {positions.length === 0 ? (
            <p className="muted">No positions match.</p>
          ) : (
            <table className="table" aria-label="Positions">
              <thead>
                <tr>
                  <th scope="col">Position</th>
                  <th scope="col">Status</th>
                  <th scope="col">Reports to</th>
                  <th scope="col">Department</th>
                  <th scope="col">Project</th>
                  <th scope="col">AI tool</th>
                  <th scope="col">Work</th>
                </tr>
              </thead>
              <tbody>
                {positions.map((p) => (
                  <tr key={p.id} aria-current={p.id === selectedId ? "true" : undefined}>
                    <th scope="row">
                      <button type="button" className="link" onClick={() => onSelect(p.id)}>
                        {p.title}
                      </button>
                      <span className="table__sub">
                        {p.kind === "worker" ? p.roleName : rankName(t, p.kind)}
                        {p.specialty ? ` (${p.specialty})` : ""} · {STAFFING_LABEL[p.staffing]}
                      </span>
                    </th>
                    <td>
                      <StatusPill
                        status={POSITION_STATUS[p.status]}
                        label={STATUS_LABEL[p.status]}
                      />
                    </td>
                    <td>{title(p.reportsTo)}</td>
                    <td>
                      {snapshot.departments.find((d) => d.id === p.departmentId)?.name ?? "—"}
                    </td>
                    <td>{snapshot.projects.find((x) => x.id === p.projectId)?.name ?? "—"}</td>
                    <td>
                      {positionToolLabel(snapshot, p)}
                      {p.model ? ` · ${p.model}` : ""}
                    </td>
                    <td>
                      {p.staffing === "onDemand"
                        ? `${p.workers.length} live`
                        : `${p.counts.working + p.counts.waiting} open · ${p.counts.queued} queued`}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      )}

      {tab === "departments" &&
        (departments.length === 0 ? (
          <p className="muted">No departments yet.</p>
        ) : (
          <table className="table" aria-label="Departments">
            <thead>
              <tr>
                <th scope="col">Department</th>
                <th scope="col">{rankName(t, "departmentManager")}</th>
                <th scope="col">Projects</th>
                <th scope="col">State</th>
              </tr>
            </thead>
            <tbody>
              {departments.map((d) => (
                <tr key={d.id}>
                  <th scope="row">
                    {d.name}
                    {d.description && <span className="table__sub">{d.description}</span>}
                  </th>
                  <td>
                    {d.headPositionId ? (
                      <button
                        type="button"
                        className="link"
                        onClick={() => onSelect(d.headPositionId ?? "")}
                      >
                        {title(d.headPositionId)}
                      </button>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td>
                    {
                      snapshot.projects.filter((x) => x.departmentId === d.id && liveProject(x))
                        .length
                    }
                  </td>
                  <td>{d.active ? "Active" : "Inactive"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ))}

      {tab === "projects" &&
        (projects.length === 0 ? (
          <p className="muted">No projects yet.</p>
        ) : (
          <table className="table" aria-label="Projects">
            <thead>
              <tr>
                <th scope="col">Project</th>
                <th scope="col">Department</th>
                <th scope="col">{rankName(t, "projectCoordinator")}</th>
                <th scope="col">Allowed AI tools</th>
                <th scope="col">State</th>
              </tr>
            </thead>
            <tbody>
              {projects.map((p) => (
                <tr key={p.id}>
                  <th scope="row">
                    {p.name}
                    {p.description && <span className="table__sub">{p.description}</span>}
                  </th>
                  <td>{snapshot.departments.find((d) => d.id === p.departmentId)?.name ?? "—"}</td>
                  <td>
                    {p.coordinatorPositionId ? (
                      <button
                        type="button"
                        className="link"
                        onClick={() => onSelect(p.coordinatorPositionId ?? "")}
                      >
                        {title(p.coordinatorPositionId)}
                      </button>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td>
                    {p.allowedRuntimes.length === 0
                      ? "None"
                      : p.allowedRuntimes.map((r) => runtimeLabel(snapshot, r)).join(", ")}
                  </td>
                  <td>Active</td>
                </tr>
              ))}
            </tbody>
          </table>
        ))}

      {tab === "archived" && (
        <Archived snapshot={snapshot} q={q} onSelect={onSelect} actions={actions} />
      )}
      {tab === "workforce" && <Workforce snapshot={snapshot} q={q} actions={actions} />}
    </section>
  );
}

/**
 * Archived departments, projects, and agents: Bring back and Delete for good (ADR-043). Also the
 * canvas's Archived drawer (ADR-053 §11): the same rows and buttons.
 */
export function Archived({
  snapshot,
  q,
  onSelect,
  actions,
}: {
  snapshot: OrgSnapshot;
  q: string;
  onSelect: (id: string) => void;
  actions: DirectoryActions;
}) {
  const t = titlesOf(snapshot);
  const all = archivedItems(snapshot);
  const matches = (text: string) => q === "" || text.toLowerCase().includes(q);
  const departments = all.departments.filter((d) => matches(`${d.name} ${d.description}`));
  const projects = all.projects.filter((p) => matches(`${p.name} ${p.description}`));
  const positions = all.positions.filter((p) => matches(positionSearchText(snapshot, p)));
  const withIt = (kind: string, id: string) =>
    all.positions.filter((p) => p.archivedWith?.kind === kind && p.archivedWith.id === id).length;
  if (departments.length + projects.length + positions.length === 0) {
    return (
      <p className="muted">
        Nothing is archived. Archive an agent, a project, or a department from its details; it comes
        here, where you can bring it back or delete it for good.
      </p>
    );
  }
  return (
    <div className="archived">
      <p className="muted">
        Bring back puts it back as it was. Delete for good asks first, offers to save experienced
        agents to your Workforce, and keeps a short record so older work still shows the name.
      </p>
      {departments.length > 0 && (
        <table className="table" aria-label="Archived departments">
          <thead>
            <tr>
              <th scope="col">Department</th>
              <th scope="col">Archived</th>
              <th scope="col">With it</th>
              <th scope="col">
                <span className="visually-hidden">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {departments.map((d) => {
              const withProjects = snapshot.projects.filter(
                (x) => x.departmentId === d.id && x.archivedWith?.id === d.id,
              );
              const agents =
                withIt("department", d.id) +
                withProjects.reduce((n, x) => n + withIt("project", x.id), 0);
              return (
                <tr key={d.id}>
                  <th scope="row">
                    {d.name}
                    {d.description && <span className="table__sub">{d.description}</span>}
                  </th>
                  <td>{d.archivedAt ? ago(d.archivedAt) : "—"}</td>
                  <td>
                    {plural(withProjects.length, "project")}, {plural(agents, "agent")}
                  </td>
                  <td className="table__actions">
                    <Button
                      size="sm"
                      variant="quiet"
                      aria-label={`Bring back ${d.name}`}
                      onClick={() => actions.bringBack("department", d.id, d.name)}
                    >
                      Bring back
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      aria-label={`Delete ${d.name} for good`}
                      onClick={() => actions.deleteForGood("department", d.id)}
                    >
                      Delete for good…
                    </Button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      {projects.length > 0 && (
        <table className="table" aria-label="Archived projects">
          <thead>
            <tr>
              <th scope="col">Project</th>
              <th scope="col">Archived</th>
              <th scope="col">With it</th>
              <th scope="col">
                <span className="visually-hidden">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {projects.map((p) => (
              <tr key={p.id}>
                <th scope="row">
                  {p.name}
                  <span className="table__sub">
                    {snapshot.departments.find((d) => d.id === p.departmentId)?.name ??
                      "No department"}
                    {p.archivedWith ? ` · archived with the ${p.archivedWith.name} department` : ""}
                  </span>
                </th>
                <td>{p.archivedAt ? ago(p.archivedAt) : "—"}</td>
                <td>{plural(withIt("project", p.id), "agent")}</td>
                <td className="table__actions">
                  {p.archivedWith ? (
                    <Button
                      size="sm"
                      variant="quiet"
                      aria-label={`Bring back the ${p.archivedWith.name} department`}
                      onClick={() =>
                        actions.bringBack(
                          "department",
                          p.archivedWith?.id ?? "",
                          p.archivedWith?.name ?? "",
                        )
                      }
                    >
                      Bring back its department
                    </Button>
                  ) : (
                    <Button
                      size="sm"
                      variant="quiet"
                      aria-label={`Bring back ${p.name}`}
                      onClick={() => actions.bringBack("project", p.id, p.name)}
                    >
                      Bring back
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="danger"
                    aria-label={`Delete ${p.name} for good`}
                    onClick={() => actions.deleteForGood("project", p.id)}
                  >
                    Delete for good…
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {positions.length > 0 && (
        <table className="table" aria-label="Archived agents">
          <thead>
            <tr>
              <th scope="col">Agent</th>
              <th scope="col">Archived</th>
              <th scope="col">Experience</th>
              <th scope="col">
                <span className="visually-hidden">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {positions.map((p) => {
              const leads = p.headsDepartmentId !== null || p.coordinatesProjectId !== null;
              return (
                <tr key={p.id}>
                  <th scope="row">
                    <button type="button" className="link" onClick={() => onSelect(p.id)}>
                      {p.title}
                    </button>
                    <span className="table__sub">
                      {p.kind === "worker" ? p.roleName : rankName(t, p.kind)}
                      {p.specialty ? ` (${p.specialty})` : ""} · {archivedWithLine(p)}
                    </span>
                  </th>
                  <td>{p.archivedAt ? ago(p.archivedAt) : "—"}</td>
                  <td>
                    {experienceLine(p.experience)}
                    {p.experience.experienced ? " · experienced" : ""}
                  </td>
                  <td className="table__actions">
                    {!p.archivedWith && !leads && (
                      <Button
                        size="sm"
                        variant="quiet"
                        aria-label={`Bring back ${p.title}`}
                        onClick={() => actions.bringBack("position", p.id, p.title)}
                      >
                        Bring back
                      </Button>
                    )}
                    {!leads && (
                      <Button
                        size="sm"
                        variant="quiet"
                        aria-label={`Save ${p.title} to my Workforce`}
                        onClick={() => actions.saveToWorkforce(p.id, p.title)}
                      >
                        Save to my Workforce
                      </Button>
                    )}
                    {!leads && (
                      <Button
                        size="sm"
                        variant="danger"
                        aria-label={`Delete ${p.title} for good`}
                        onClick={() => actions.deleteForGood("position", p.id)}
                      >
                        Delete for good…
                      </Button>
                    )}
                    {(p.archivedWith || leads) && (
                      <span className="muted table__sub">
                        Comes back with its{" "}
                        {leads
                          ? p.headsDepartmentId
                            ? "department"
                            : "project"
                          : p.archivedWith?.kind}
                      </span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}

/** Your Workforce (ADR-045): agents you saved, to hire again into any team. */
function Workforce({
  snapshot,
  q,
  actions,
}: {
  snapshot: OrgSnapshot;
  q: string;
  actions: DirectoryActions;
}) {
  const agents = snapshot.workforce.filter(
    (w) =>
      q === "" ||
      `${w.title} ${w.roleName} ${w.specialty ?? ""} ${w.places.join(" ")}`
        .toLowerCase()
        .includes(q),
  );
  if (snapshot.workforce.length === 0) {
    return (
      <p className="muted">
        Your Workforce is empty. Save an archived agent here — or keep the experienced ones when you
        delete a project or department for good — to hire it again later with its settings,
        experience, and lessons.
      </p>
    );
  }
  return (
    <div className="workforce">
      <p className="muted">
        Agents you saved, with their settings, experience, and lessons. Your organization&apos;s
        average experience is {snapshot.averageExperience}.
      </p>
      <table className="table" aria-label="Workforce">
        <thead>
          <tr>
            <th scope="col">Agent</th>
            <th scope="col">Experience</th>
            <th scope="col">Worked in</th>
            <th scope="col">Saved</th>
            <th scope="col">
              <span className="visually-hidden">Actions</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {agents.map((w) => (
            <tr key={w.id}>
              <th scope="row">
                {w.title}
                <span className="table__sub">
                  {w.roleName}
                  {w.specialty ? ` (${w.specialty})` : ""}
                  {w.runtimeId ? ` · ${runtimeLabel(snapshot, w.runtimeId)}` : ""}
                  {w.model ? ` · ${w.model}` : ""}
                </span>
              </th>
              <td>
                {experienceLine(w.experience)}
                {w.experience.experienced ? " · experienced" : ""}
                {w.lessons.length > 0 && (
                  <details className="advanced">
                    <summary>{plural(w.lessons.length, "lesson")} it brings</summary>
                    <ul className="inspector__list">
                      {w.lessons.map((l) => (
                        <li key={l}>{l}</li>
                      ))}
                    </ul>
                  </details>
                )}
              </td>
              <td>
                {w.places.length > 0 ? w.places.join(", ") : "—"}
                {w.lastWorked && (
                  <span className="table__sub">Last worked {ago(w.lastWorked)}</span>
                )}
              </td>
              <td>{ago(w.savedAt)}</td>
              <td className="table__actions">
                <Button
                  size="sm"
                  variant="primary"
                  aria-label={`Hire ${w.title} into a team`}
                  onClick={() => actions.hireSaved(w)}
                >
                  Hire into a team…
                </Button>
                <Button
                  size="sm"
                  variant="danger"
                  aria-label={`Delete ${w.title} for good`}
                  onClick={() => actions.deleteSaved(w)}
                >
                  Delete for good…
                </Button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
