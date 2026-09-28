/** The Work tab: its tasks, its live workers, and what it may do (its permissions, and the
 * permission limit of its project and department). */
import { useEffect, useState } from "react";
import type { OrgSnapshot, PositionInfo, TaskBrief, WorkView } from "@plenipo/types";
import { StatusPill, Tabs } from "@plenipo/ui";

import { getWork, toCommandError } from "../../../api/commands";
import { capabilityLabel, setSummary } from "../../../guard/format";
import { usePermissions } from "../../../guard/usePermissions";
import { POSITION_STATUS } from "../../../org/cards";
import { WORKER_STATE_LABEL } from "../../../org/format";
import { workerStatus } from "../../../org/nodes";
import { ItemLink, Option, Options, Section, TaskRow } from "./parts";
import type { InspectorActions } from "./types";

type WorkList = "running" | "waiting" | "queued" | "recent" | "team";
const WORK_TABS: { id: WorkList; label: string }[] = [
  { id: "running", label: "Running" },
  { id: "waiting", label: "Waiting" },
  { id: "queued", label: "Queued" },
  { id: "recent", label: "Recent" },
  { id: "team", label: "Its team" },
];

export function WorkTab({
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
  return (
    <>
      <WorkPanel positionId={p.id} revision={revision} onOpen={actions.openTask} />
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
                  <ItemLink onClick={() => onSelect(`worker:${w.agentId}`)}>
                    {w.objective || "(no objective)"}
                  </ItemLink>
                  <StatusPill
                    status={POSITION_STATUS[workerStatus(w.state)]}
                    label={WORKER_STATE_LABEL[w.state]}
                  />
                </li>
              ))}
            </ul>
          )}
        </Section>
      )}
      <PermissionsSection p={p} snapshot={snapshot} actions={actions} />
    </>
  );
}

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
  const [tab, setTab] = useState<WorkList>("running");
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
      <Tabs<WorkList>
        label="Work"
        value={tab}
        onChange={setTab}
        tabs={WORK_TABS.map((t) => {
          const count = current ? current[t.id].length : 0;
          return { value: t.id, label: count > 0 ? `${t.label} (${count})` : t.label };
        })}
      />
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

/** What it may do: its role's permission set, narrowed by its project's and department's
 * permission limits; and the permissions its specialty usually needs. */
function PermissionsSection({
  p,
  snapshot,
  actions,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  const permissions = usePermissions();
  const settings = permissions.snapshot?.settings ?? null;
  const setName = (id: string | undefined) =>
    id ? (settings?.sets.find((s) => s.id === id)?.name ?? id) : null;
  const roleSet = settings?.roles.find((r) => r.roleId === p.roleId)?.setId;
  const set = settings?.sets.find((s) => s.id === roleSet);
  const projectLimit = setName(settings?.projects.find((x) => x.id === p.projectId)?.setId);
  const departmentLimit = setName(
    settings?.departments.find((d) => d.id === p.departmentId)?.setId,
  );
  const suggested =
    snapshot.roles.find((r) => r.id === p.roleId)?.specialties.find((s) => s.id === p.specialtyId)
      ?.suggest.permissions ?? [];
  return (
    <Section title="Permissions">
      {!settings ? (
        <p className="muted">{permissions.error ?? "Loading its permissions…"}</p>
      ) : (
        <dl className="kv">
          <dt>Its permissions</dt>
          <dd>
            {set
              ? `${set.name} (from the ${p.roleName} role): ${setSummary(set)}`
              : "None: the role has no permission set, so it can only talk."}
          </dd>
          {p.projectId && (
            <>
              <dt>Its project&apos;s limit</dt>
              <dd>{projectLimit ?? "No limit"}</dd>
            </>
          )}
          {p.departmentId && (
            <>
              <dt>Its department&apos;s limit</dt>
              <dd>{departmentLimit ?? "No limit"}</dd>
            </>
          )}
        </dl>
      )}
      {suggested.length > 0 && (
        <p className="muted inspector__note">
          Its specialty usually needs: {suggested.map(capabilityLabel).join(", ")}. Nothing changes
          until you change the role&apos;s permissions.
        </p>
      )}
      {actions.openPage && (
        <Options>
          <Option
            label="Change permissions"
            icon="chevronRight"
            hint="Settings → Permissions: what each role may do, and each project's limit."
            onClick={() => actions.openPage?.({ view: "settings", id: "permissions" })}
          />
        </Options>
      )}
    </Section>
  );
}
