/** The Team tab: who it reports to (and Move), its team, its reviewer, QA, and security
 * assignments, and the department or project it leads. */
import { useId, useState } from "react";
import type { OrgSnapshot, OversightRole, PositionInfo } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { POSITION_STATUS } from "../../../org/cards";
import {
  OVERSIGHT_LABEL,
  OVERSIGHT_NOUN,
  STATUS_LABEL,
  plural,
  runtimeLabel,
} from "../../../org/format";
import { moveChoices, oversightOrder, oversightRefusal, positionMap } from "../../../org/rules";
import { rankName, titlesOf } from "../../../org/titles";
import { Field, ItemLink, Option, Options, Refusal, Section } from "./parts";
import { useRun } from "./useRun";
import type { InspectorActions } from "./types";

const OWNER_VALUE = "__owner__";

export function TeamTab({
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
  const t = titlesOf(snapshot);
  const supervisor = p.reportsTo ? byId.get(p.reportsTo) : null;
  const team = snapshot.positions.filter((x) => x.active && x.reportsTo === p.id);
  const lentIn = snapshot.positions.filter((x) => x.active && x.loan?.toLeadId === p.id);
  return (
    <>
      {p.loan && <LentSection p={p} actions={actions} onSelect={onSelect} />}
      <Section title="Reports to">
        <p>
          {supervisor ? (
            <ItemLink onClick={() => onSelect(supervisor.id)}>{supervisor.title}</ItemLink>
          ) : (
            `You (${rankName(t, "owner")})`
          )}
        </p>
        {p.active && <MoveForm p={p} snapshot={snapshot} actions={actions} />}
      </Section>
      {p.staffing === "persistent" && (
        <Section title={`Its team (${team.length})`}>
          {team.length === 0 ? (
            <p className="muted">Nobody reports to it yet.</p>
          ) : (
            <ul className="inspector__list">
              {team.map((m) => (
                <li key={m.id}>
                  <ItemLink onClick={() => onSelect(m.id)}>{m.title}</ItemLink>{" "}
                  <StatusPill status={POSITION_STATUS[m.status]} label={STATUS_LABEL[m.status]} />
                  {m.loan && <span className="muted"> · lent to {m.loan.to}&apos;s team</span>}
                </li>
              ))}
            </ul>
          )}
        </Section>
      )}
      {lentIn.length > 0 && (
        <Section title={`Lent to its team (${lentIn.length})`}>
          <ul className="inspector__list">
            {lentIn.map((m) => (
              <li key={m.id}>
                <ItemLink onClick={() => onSelect(m.id)}>{m.title}</ItemLink>{" "}
                <span className="muted">
                  lent from {m.reportsTo ? (byId.get(m.reportsTo)?.title ?? "another") : "your"}
                  &apos;s team
                  {m.loan?.until === "objective" ? ", for one objective" : ""}
                </span>
              </li>
            ))}
          </ul>
        </Section>
      )}
      {p.active && (
        <OversightPanel p={p} snapshot={snapshot} actions={actions} onSelect={onSelect} />
      )}
      <DepartmentSection p={p} snapshot={snapshot} actions={actions} />
      <ProjectSection p={p} snapshot={snapshot} actions={actions} />
    </>
  );
}

/** Where a lent agent helps now, and Send home (ADR-054 §5–§6). */
function LentSection({
  p,
  actions,
  onSelect,
}: {
  p: PositionInfo;
  actions: InspectorActions;
  onSelect: (id: string) => void;
}) {
  const { pending, error, go } = useRun(actions);
  const loan = p.loan;
  if (!loan) return null;
  return (
    <Section title="Lent to another team">
      <p>
        Helping <ItemLink onClick={() => onSelect(loan.toLeadId)}>{loan.to}</ItemLink>&apos;s team
        {loan.project ? ` on ${loan.project}` : ""}
        {loan.until === "objective"
          ? loan.objectiveTaskId
            ? ", until this objective is done."
            : ", for its next objective."
          : ", until you send it home."}{" "}
        That team decides its permission limit, working copy, and AI tools while it helps.
      </p>
      {loan.goingHome ? (
        <p className="muted">Going home after the task it is on.</p>
      ) : (
        <div className="option">
          <Button
            size="sm"
            variant="primary"
            disabled={pending}
            onClick={() => void go(() => actions.api.sendHome(p.id))}
          >
            Send home
          </Button>
          <span className="muted">If it is working, it finishes this task first.</span>
        </div>
      )}
      <Refusal error={error} />
    </Section>
  );
}

function MoveForm({
  p,
  snapshot,
  actions,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  const byId = positionMap(snapshot);
  const t = titlesOf(snapshot);
  const choices = moveChoices(snapshot, p);
  const [moveTo, setMoveTo] = useState("");
  const { pending, error, go } = useRun(actions);
  const moveHint = useId();
  return (
    <form
      className="inspector__move"
      aria-label="Change who it reports to"
      onSubmit={(e) => {
        e.preventDefault();
        const to = moveTo === OWNER_VALUE ? null : moveTo;
        // Once moved, the choice is where it already reports: start the form over.
        if (moveTo) {
          void go(() => actions.api.move(p.id, to)).then((moved) => moved && setMoveTo(""));
        }
      }}
    >
      <Field
        label="Move to report to"
        hint={
          p.staffing === "persistent"
            ? "Its whole team moves with it."
            : "It joins the chosen lead's team."
        }
      >
        {({ id, hintId }) => (
          <select
            id={id}
            aria-describedby={hintId}
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
        )}
      </Field>
      <div className="option">
        <Button
          type="submit"
          variant="primary"
          size="sm"
          aria-describedby={moveHint}
          disabled={pending || moveTo === ""}
        >
          Move
        </Button>
        <span id={moveHint} className="option__hint">
          Makes it report to the one chosen above.
        </span>
      </div>
      <Refusal error={error} />
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
  const endHint = useId();
  const assignHint = useId();
  const chosen = targets.find((t) => t.id === target) ?? targets[0] ?? null;
  const title = (id: string) => byId.get(id)?.title ?? "a former position";

  const end = (id: string) => void go(() => actions.api.endOversight(id));
  if (p.staffing === "persistent" && overseenBy.length === 0 && oversees.length === 0) {
    return null;
  }
  const ends = oversees.length > 0 || overseenBy.length > 0;
  return (
    <Section title="Reviewer, QA, and security">
      {oversees.length > 0 && (
        <ul className="inspector__list">
          {oversees.map((o) => (
            <li key={o.id}>
              {OVERSIGHT_LABEL[o.role]} for{" "}
              <ItemLink onClick={() => onSelect(o.targetId)}>{title(o.targetId)}</ItemLink>
              &apos;s team{" "}
              <Button
                variant="quiet"
                size="sm"
                disabled={pending}
                aria-describedby={endHint}
                onClick={() => end(o.id)}
              >
                End
              </Button>
            </li>
          ))}
        </ul>
      )}
      {overseenBy.length > 0 && (
        <ul className="inspector__list">
          {overseenBy.map((o) => (
            <li key={o.id}>
              <ItemLink onClick={() => onSelect(o.overseerId)}>{title(o.overseerId)}</ItemLink> is
              this team&apos;s {OVERSIGHT_NOUN[o.role]}{" "}
              <Button
                variant="quiet"
                size="sm"
                disabled={pending}
                aria-describedby={endHint}
                onClick={() => end(o.id)}
              >
                End
              </Button>
            </li>
          ))}
        </ul>
      )}
      {ends && (
        <p id={endHint} className="muted inspector__note">
          End stops the assignment; the team no longer hands it work to check.
        </p>
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
          <Field
            label="Assign as"
            hint="What it checks for the team: its work, its testing, or its safety."
          >
            {({ id, hintId }) => (
              <select
                id={id}
                aria-describedby={hintId}
                value={role}
                onChange={(e) => setRole(e.target.value as OversightRole)}
              >
                {order.map((r) => (
                  <option key={r} value={r}>
                    {OVERSIGHT_LABEL[r]}
                  </option>
                ))}
              </select>
            )}
          </Field>
          <Field label="Of the team led by" hint="The lead whose team hands it work to check.">
            {({ id, hintId }) => (
              <select
                id={id}
                aria-describedby={hintId}
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
            )}
          </Field>
          <div className="option">
            <Button
              type="submit"
              variant="primary"
              size="sm"
              aria-describedby={assignHint}
              disabled={pending || !chosen}
            >
              Assign
            </Button>
            <span id={assignHint} className="option__hint">
              It stays where it is and joins that team&apos;s list of who checks the work.
            </span>
          </div>
        </form>
      )}
      <Refusal error={error} />
    </Section>
  );
}

/** The department it leads: its page, Edit, New project, and Archive department (ADR-043). */
function DepartmentSection({
  p,
  snapshot,
  actions,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  const department = p.headsDepartmentId
    ? snapshot.departments.find((d) => d.id === p.headsDepartmentId)
    : undefined;
  if (!department) return null;
  const live = department.archivedAt === null && !department.deleted;
  const projects = snapshot.projects.filter((x) => x.departmentId === department.id && !x.deleted);
  return (
    <Section title={`Department: ${department.name}`}>
      {department.description && <p className="muted">{department.description}</p>}
      <p className="muted">
        {plural(projects.length, "project")}
        {live ? (department.active ? "" : " · inactive") : " · archived"}
      </p>
      <Options>
        {actions.openPage && (
          <Option
            label="Open the department's page"
            icon="chevronRight"
            hint="Its projects, people, and history on a page of its own."
            onClick={() => actions.openPage?.({ view: "department", id: department.id })}
          />
        )}
        {live && (
          <>
            <Option
              label="Edit department"
              hint="Change its name, description, or whether it takes new projects."
              onClick={() => actions.editDepartment(department.id)}
            />
            <Option
              label="New project"
              hint="Start a project in this department, with its supervisor."
              onClick={() => actions.newProject(department.id)}
            />
            <Option
              label="Archive department"
              variant="danger"
              hint="Archives the department with its projects and people. You can bring it back."
              onClick={() =>
                actions.confirm({
                  title: `Archive ${department.name}?`,
                  message: (
                    <p>
                      The department is archived with everything in it: its{" "}
                      {plural(projects.filter((x) => x.active).length, "project")} and every agent
                      under {p.title}. This is refused while any of them has unfinished work. You
                      can bring it back from the Archived list, as it was.
                    </p>
                  ),
                  confirmLabel: "Archive department",
                  work: () => actions.api.archiveDepartment(department.id),
                })
              }
            />
          </>
        )}
      </Options>
    </Section>
  );
}

/** The project it leads: its settings, page, Edit, and Archive project. */
function ProjectSection({
  p,
  snapshot,
  actions,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  const project = p.coordinatesProjectId
    ? snapshot.projects.find((x) => x.id === p.coordinatesProjectId)
    : undefined;
  if (!project) return null;
  return (
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
        <dt>Folder</dt>
        <dd>
          {project.localPath ? (
            <span className="path">{project.localPath}</span>
          ) : (
            "None — its workers get no file, program, or git tools"
          )}
        </dd>
        <dt>Permission limit</dt>
        <dd>{project.capabilityProfile ?? "No limit"}</dd>
      </dl>
      <Options>
        {actions.openPage && (
          <Option
            label="Open the project's page"
            icon="chevronRight"
            hint="Its objectives, pull requests, and history on a page of its own."
            onClick={() => actions.openPage?.({ view: "project", id: project.id })}
          />
        )}
        {project.active && (
          <>
            <Option
              label="Edit project"
              hint="Change its folder, repository, allowed AI tools, or permission limit."
              onClick={() => actions.editProject(project.id)}
            />
            <Option
              label="Archive project"
              variant="danger"
              hint="Archives the project with its whole team. You can bring it back."
              onClick={() =>
                actions.confirm({
                  title: `Archive ${project.name}?`,
                  message: (
                    <p>
                      The project and its whole team are archived: {p.title} and every position
                      under it. This is refused while any of them has unfinished work. You can bring
                      it back from the Archived list.
                    </p>
                  ),
                  confirmLabel: "Archive project",
                  work: () => actions.api.archiveProject(project.id),
                })
              }
            />
          </>
        )}
      </Options>
    </Section>
  );
}
