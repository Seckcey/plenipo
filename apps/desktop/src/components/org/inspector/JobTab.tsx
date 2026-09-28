/** The Job tab: its role and working instructions, its specialty, learning, and lessons. */
import type { OrgSnapshot, PositionInfo, RoleInfo, RoleJob, SpecialtyInfo } from "@plenipo/types";

import { setAgentLearning, setRoleLearns } from "../../../api/commands";
import { RoleLessons } from "../../../learning/Lessons";
import { useLearning, type Learning } from "../../../learning/useLearning";
import { learningLine, specialtiesOf } from "../../../org/control";
import { Toggle } from "../../SwitchSettings";
import { Field, Option, Options, Refusal, Section } from "./parts";
import { useRun } from "./useRun";
import type { InspectorActions } from "./types";

const JOB_HEADINGS: [keyof RoleJob, string][] = [
  ["duties", "Its job"],
  ["returns", "What it hands back"],
  ["limits", "What it must not do"],
  ["askLead", "When it asks its lead for help"],
];

/** The four parts of a job, as lists under their headings. */
function JobLines({ job }: { job: RoleJob }) {
  const lists = JOB_HEADINGS.filter(([k]) => job[k].length > 0);
  return (
    <>
      {lists.map(([k, heading]) => (
        <div key={k}>
          <h4>{heading}</h4>
          <ul className="inspector__list">
            {job[k].map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      ))}
    </>
  );
}

const hasLines = (job: RoleJob) => JOB_HEADINGS.some(([k]) => job[k].length > 0);

export function JobTab({
  p,
  snapshot,
  actions,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  const role = snapshot.roles.find((r) => r.id === p.roleId);
  const learning = useLearning();
  return (
    <>
      {role && <RoleSection role={role} actions={actions} />}
      {role && <SpecialtySection p={p} role={role} snapshot={snapshot} actions={actions} />}
      {role && <LearningSection p={p} role={role} learning={learning} actions={actions} />}
    </>
  );
}

/** What the position's role does: its description and working instructions (ADR-019). */
function RoleSection({ role, actions }: { role: RoleInfo; actions: InspectorActions }) {
  return (
    <Section title={`What the ${role.name} role does`}>
      {role.description && <p className="muted">{role.description}</p>}
      {hasLines(role.job) && (
        <details className="inspector__job">
          <summary>Working instructions</summary>
          <JobLines job={role.job} />
        </details>
      )}
      {!role.template && (
        <Options>
          <Option
            label="Edit role"
            hint="Change what this role does; every agent with it gets the new instructions."
            onClick={() => actions.editRole(role.id)}
          />
        </Options>
      )}
    </Section>
  );
}

/** Its specialty (ADR-042): lines added to its role's instructions for one area of work. */
function SpecialtySection({
  p,
  role,
  snapshot,
  actions,
}: {
  p: PositionInfo;
  role: RoleInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  const run = useRun(actions);
  const specialties = specialtiesOf(snapshot, role.id);
  const chosen: SpecialtyInfo | undefined = specialties.find((s) => s.id === p.specialtyId);
  if (!p.active) {
    return (
      <Section title="Specialty">
        <p className="muted">{p.specialty ?? "None"}. Bring it back to change its specialty.</p>
      </Section>
    );
  }
  return (
    <Section title="Specialty">
      {specialties.length > 0 ? (
        <Field
          label="Specialty"
          hint="Adds lines for one area of work to its instructions. Changing it never hires a new agent."
        >
          {({ id, hintId }) => (
            <select
              id={id}
              aria-describedby={hintId}
              value={p.specialtyId ?? ""}
              disabled={run.pending}
              onChange={(e) =>
                void run.go(() => actions.api.update(p.id, { specialtyId: e.target.value }))
              }
            >
              <option value="">None: the {role.name} role&apos;s job alone</option>
              {specialties.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                  {s.builtIn ? "" : " (yours)"}
                </option>
              ))}
            </select>
          )}
        </Field>
      ) : (
        <p className="muted">The {role.name} role has no specialties yet.</p>
      )}
      {chosen && hasLines(chosen.job) && (
        <details className="inspector__job">
          <summary>What {chosen.name} adds to its instructions</summary>
          <JobLines job={chosen.job} />
        </details>
      )}
      <Options>
        <Option
          label="Add your own specialty"
          hint={`Write a specialty of your own for the ${role.name} role.`}
          onClick={() => actions.newSpecialty(role.id)}
        />
      </Options>
      <Refusal error={run.error} />
    </Section>
  );
}

const FOLLOW = "follow";

/** Whether it learns (ADR-041): its own setting, its role's, and Worker learning for everyone. */
function LearningSection({
  p,
  role,
  learning,
  actions,
}: {
  p: PositionInfo;
  role: RoleInfo;
  learning: Learning;
  actions: InspectorActions;
}) {
  const run = useRun(actions);
  const s = learning.snapshot;
  const roleLearns = s ? !s.offRoles.includes(role.id) : role.learns;
  const own = s ? (p.id in s.agents ? (s.agents[p.id] ?? null) : null) : p.learning.own;
  const value = own === null ? FOLLOW : own ? "on" : "off";
  const after = (next: unknown) => {
    void learning.reload();
    return next;
  };
  return (
    <Section title="Learning">
      <p className="muted">{learningLine(p.learning, role.name)}</p>
      {p.active && (
        <Field
          label="This agent learns"
          hint="Whether it writes down lessons from its work and gets its role's kept lessons."
        >
          {({ id, hintId }) => (
            <select
              id={id}
              aria-describedby={hintId}
              value={value}
              disabled={run.pending}
              onChange={(e) => {
                const next = e.target.value;
                void run.change(() =>
                  setAgentLearning(p.id, next === FOLLOW ? null : next === "on").then(after),
                );
              }}
            >
              <option value={FOLLOW}>
                Like its role ({roleLearns ? "learns" : "does not learn"})
              </option>
              <option value="on">Always learns</option>
              <option value="off">Never learns</option>
            </select>
          )}
        </Field>
      )}
      <Toggle
        label={`${role.name} agents learn`}
        hint={`On or off for every ${role.name} agent without a setting of its own.`}
        checked={roleLearns}
        disabled={run.pending}
        onChange={(on) => void run.change(() => setRoleLearns(role.id, on).then(after))}
      />
      <Refusal error={run.error} />
      <RoleLessons learning={learning} roleId={role.id} roleName={role.name} />
    </Section>
  );
}
