import { useState } from "react";
import type { OrgSnapshot, PositionStatus } from "@plenipo/types";

import { STAFFING_LABEL, STATUS_LABEL, positionToolLabel, runtimeLabel } from "../../org/format";
import { positionMap } from "../../org/rules";
import { positionSearchText } from "../../org/search";
import { rankName, titlesOf } from "../../org/titles";
import { StatusPill } from "./OrgNode";

type Tab = "positions" | "departments" | "projects";

/** The organization as tables: positions, departments, and projects, filterable. */
export function Directory({
  snapshot,
  query,
  selectedId,
  onSelect,
}: {
  snapshot: OrgSnapshot;
  query: string;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const [tab, setTab] = useState<Tab>("positions");
  const [department, setDepartment] = useState("");
  const [status, setStatus] = useState<PositionStatus | "">("");
  const [archived, setArchived] = useState(false);
  const byId = positionMap(snapshot);
  const t = titlesOf(snapshot);
  const q = query.trim().toLowerCase();
  const title = (id: string | null) =>
    id ? (byId.get(id)?.title ?? "—") : `You (${rankName(t, "owner")})`;

  const positions = snapshot.positions.filter(
    (p) =>
      (archived || p.active) &&
      (department === "" || p.departmentId === department) &&
      (status === "" || p.status === status) &&
      (q === "" || positionSearchText(snapshot, p).includes(q)),
  );
  const departments = snapshot.departments.filter(
    (d) => q === "" || `${d.name} ${d.description}`.toLowerCase().includes(q),
  );
  const projects = snapshot.projects.filter(
    (p) => q === "" || `${p.name} ${p.description}`.toLowerCase().includes(q),
  );

  return (
    <section className="directory" aria-label="Organization directory" data-canvas-scroll>
      <div className="tabs" role="tablist" aria-label="Directory">
        {(
          [
            ["positions", `Positions (${snapshot.stats.positions})`],
            ["departments", `Departments (${snapshot.departments.length})`],
            ["projects", `Projects (${snapshot.projects.length})`],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            className="tabs__tab"
            aria-selected={tab === id}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === "positions" && (
        <>
          <div className="directory__filters">
            <label className="field field--inline">
              <span>Department</span>
              <select value={department} onChange={(e) => setDepartment(e.target.value)}>
                <option value="">All</option>
                {snapshot.departments.map((d) => (
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
            <label className="check">
              <input
                type="checkbox"
                checked={archived}
                onChange={(e) => setArchived(e.target.checked)}
              />
              <span>Show archived</span>
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
                        {p.kind === "worker" ? p.roleName : rankName(t, p.kind)} ·{" "}
                        {STAFFING_LABEL[p.staffing]}
                      </span>
                    </th>
                    <td>
                      <StatusPill status={p.status} label={STATUS_LABEL[p.status]} />
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
                  <td>{d.projectIds.length}</td>
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
                    {p.coordinatorPositionId && p.active ? (
                      <button
                        type="button"
                        className="link"
                        onClick={() => onSelect(p.coordinatorPositionId ?? "")}
                      >
                        {title(p.coordinatorPositionId)}
                      </button>
                    ) : (
                      title(p.coordinatorPositionId)
                    )}
                  </td>
                  <td>
                    {p.allowedRuntimes.length === 0
                      ? "None"
                      : p.allowedRuntimes.map((r) => runtimeLabel(snapshot, r)).join(", ")}
                  </td>
                  <td>{p.active ? "Active" : "Archived"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ))}
    </section>
  );
}
