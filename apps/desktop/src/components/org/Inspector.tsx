/**
 * The properties panel (Phase 17): the details of whatever is selected on the Organization page.
 * An agent's details come in six tabs — Overview, Job, AI model, Work, Team, and Manage — and
 * every option has a line under it saying what it does. The panel can be widened from its edge.
 */
import { useState, type ReactNode } from "react";
import type { OrgSnapshot, PositionInfo, WorkerInfo } from "@plenipo/types";
import { ResizeHandle, StatusPill, Tabs } from "@plenipo/ui";

import { POSITION_STATUS } from "../../org/cards";
import { STATUS_LABEL, WORKER_STATE_LABEL, ago, runtimeLabel } from "../../org/format";
import { ORG_ID, OWNER_ID } from "../../org/layout";
import { workerStatus } from "../../org/nodes";
import { rankName, titlesOf, withArticle } from "../../org/titles";
import { PILL_TONE } from "../tones";
import { JobTab } from "./inspector/JobTab";
import { ManageTab } from "./inspector/ManageTab";
import { ModelTab } from "./inspector/ModelTab";
import { OverviewTab } from "./inspector/OverviewTab";
import { Fact, ItemLink, Option, Options, Section } from "./inspector/parts";
import { TeamTab } from "./inspector/TeamTab";
import { PANEL_MAX, PANEL_MIN, PANEL_TABS, type PanelTab } from "./inspector/panel";
import type { InspectorActions } from "./inspector/types";
import { WorkTab } from "./inspector/WorkTab";

export type { InspectorActions } from "./inspector/types";

interface Props {
  snapshot: OrgSnapshot;
  selectedId: string;
  revision: number;
  actions: InspectorActions;
  onSelect: (id: string) => void;
  onClose: () => void;
  width: number;
  onWidth: (next: number) => void;
}

export function Inspector({
  snapshot,
  selectedId,
  revision,
  actions,
  onSelect,
  onClose,
  width,
  onWidth,
}: Props) {
  // The tab stays while you select other agents, and starts at Overview when the panel opens.
  const [tab, setTab] = useState<PanelTab>("overview");
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
        tab={tab}
        onTab={setTab}
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
      <ResizeHandle
        label="Widen or narrow the details"
        value={width}
        min={PANEL_MIN}
        max={PANEL_MAX}
        edge="left"
        onChange={onWidth}
      />
      <header className="inspector__header">
        <h2>{heading}</h2>
        <button type="button" className="modal__close" aria-label="Close details" onClick={onClose}>
          ×
        </button>
      </header>
      {body}
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

/** An agent's details, in tabs. */
function PositionPanel({
  p,
  snapshot,
  revision,
  actions,
  onSelect,
  tab,
  onTab,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  revision: number;
  actions: InspectorActions;
  onSelect: (id: string) => void;
  tab: PanelTab;
  onTab: (tab: PanelTab) => void;
}) {
  const content: Record<PanelTab, () => ReactNode> = {
    overview: () => <OverviewTab p={p} snapshot={snapshot} actions={actions} onSelect={onSelect} />,
    job: () => <JobTab p={p} snapshot={snapshot} actions={actions} />,
    model: () => <ModelTab p={p} snapshot={snapshot} actions={actions} />,
    work: () => (
      <WorkTab
        p={p}
        snapshot={snapshot}
        revision={revision}
        actions={actions}
        onSelect={onSelect}
      />
    ),
    team: () => <TeamTab p={p} snapshot={snapshot} actions={actions} onSelect={onSelect} />,
    manage: () => <ManageTab p={p} snapshot={snapshot} actions={actions} />,
  };
  return (
    <>
      <Tabs<PanelTab>
        label="Details"
        idPrefix="details"
        className="inspector__tabs"
        value={tab}
        onChange={onTab}
        tabs={PANEL_TABS}
      />
      <div
        className="inspector__body"
        role="tabpanel"
        id={`details-panel-${tab}`}
        aria-labelledby={`details-tab-${tab}`}
        data-canvas-scroll
      >
        {content[tab]()}
      </div>
    </>
  );
}

// ---- You and the organization ---------------------------------------------------------------

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
    <div className="inspector__body" data-canvas-scroll>
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
                <ItemLink onClick={() => onSelect(p.id)}>{p.title}</ItemLink>{" "}
                <StatusPill status={POSITION_STATUS[p.status]} label={STATUS_LABEL[p.status]} />
              </li>
            ))}
          </ul>
        )}
      </Section>
      <Options>
        <Option
          label={`Hire ${withArticle(rankName(t, "superintendent"))}`}
          variant="primary"
          hint="A full-time leader who reports to you and runs departments for you."
          onClick={() => actions.hire(null)}
        />
        <Option
          label="New department"
          hint="A department with its manager, reporting to you."
          onClick={() => actions.newDepartment(null)}
        />
      </Options>
    </div>
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
  const ownRoles = snapshot.roles.filter((r) => !r.template);
  const ownSpecialties = snapshot.roles.flatMap((r) =>
    r.specialties.filter((x) => !x.builtIn).map((x) => ({ ...x, roleName: r.name })),
  );
  return (
    <div className="inspector__body" data-canvas-scroll>
      <Options>
        <Option label="Rename" hint="Change the organization's name." onClick={actions.rename} />
        <Option
          label="New role"
          hint="A role of your own, with its job in your words."
          onClick={actions.newRole}
        />
      </Options>
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
        <Fact label="Average experience" value={snapshot.averageExperience} />
      </dl>
      {ownRoles.length > 0 && (
        <Section title="Roles you created">
          <ul className="inspector__list">
            {ownRoles.map((r) => (
              <li key={r.id}>
                <Option
                  label={`Edit ${r.name}`}
                  hint="Change what the role does; its agents get the new instructions."
                  onClick={() => actions.editRole(r.id)}
                />
              </li>
            ))}
          </ul>
        </Section>
      )}
      {ownSpecialties.length > 0 && (
        <Section title="Specialties you created">
          <ul className="inspector__list">
            {ownSpecialties.map((x) => (
              <li key={x.id}>
                <Option
                  label={`Edit ${x.name} (${x.roleName})`}
                  hint="Change its lines, or remove it once no agent on the chart has it."
                  onClick={() => actions.editSpecialty(x.id)}
                />
              </li>
            ))}
          </ul>
        </Section>
      )}
      <Section title="AI tools">
        <ul className="inspector__list">
          {snapshot.runtimes.map((r) => (
            <li key={r.id}>
              {r.label}{" "}
              <StatusPill
                status={r.ready ? PILL_TONE.ok : PILL_TONE.warn}
                label={r.ready ? "Ready" : "Not ready"}
              />
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
    </div>
  );
}

// ---- Workers ----------------------------------------------------------------------------------

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
    <div className="inspector__body" data-canvas-scroll>
      <p className="inspector__objective-text">{worker.objective || "(no objective)"}</p>
      <StatusPill
        status={POSITION_STATUS[workerStatus(worker.state)]}
        label={WORKER_STATE_LABEL[worker.state]}
      />
      <dl className="kv">
        <dt>Position</dt>
        <dd>
          <ItemLink onClick={() => onSelect(position.id)}>{position.title}</ItemLink>
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
      <Options>
        <Option
          label="Open task"
          hint="The task it is doing, with its steps and answer."
          onClick={() => actions.openTask(worker.taskId)}
        />
        {worker.sessionId && (
          <Option
            label="Open conversation"
            hint="What the worker and its lead said."
            onClick={() => actions.openSession(worker.sessionId ?? "")}
          />
        )}
      </Options>
    </div>
  );
}
