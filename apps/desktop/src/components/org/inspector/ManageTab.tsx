/**
 * The Manage tab: rename, hire into its team, hire or let its agent go, and archive; for an
 * archived agent, Bring back, Save to my Workforce, and Delete for good (ADR-043, ADR-045).
 */
import { useId, useState } from "react";
import type { OrgSnapshot, PositionInfo } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { archivedWithLine } from "../../../org/control";
import { Field, Option, Options, Refusal, Section } from "./parts";
import { useRun } from "./useRun";
import type { InspectorActions } from "./types";

export function ManageTab({
  p,
  snapshot,
  actions,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  return p.active ? (
    <ActiveManage p={p} actions={actions} />
  ) : (
    <ArchivedManage p={p} snapshot={snapshot} actions={actions} />
  );
}

function ActiveManage({ p, actions }: { p: PositionInfo; actions: InspectorActions }) {
  const run = useRun(actions);
  const leads = p.staffing === "persistent";
  const leadsUnit = p.headsDepartmentId !== null || p.coordinatesProjectId !== null;
  return (
    <>
      <RenameForm key={p.title} p={p} actions={actions} />
      <Section title="People">
        <Options>
          {leads && (
            <Option
              label="Hire into team"
              variant="primary"
              hint="Add a position to its team: a new agent, or one from your Workforce."
              onClick={() => actions.hire(p.id)}
            />
          )}
          {leads && p.agent === null && (
            <Option
              label="Hire an agent"
              variant="primary"
              disabled={run.pending}
              hint="Puts an agent in this vacant position so it can take objectives."
              onClick={() => void run.go(() => actions.api.fill(p.id))}
            />
          )}
          {leads && p.agent !== null && (
            <Option
              label="Let agent go"
              hint="Its agent retires and its conversation ends; the position stays, vacant."
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
            />
          )}
        </Options>
      </Section>
      <Section title="Archive">
        {leadsUnit ? (
          <p className="muted">
            It leads a {p.headsDepartmentId ? "department" : "project"}: archive the{" "}
            {p.headsDepartmentId ? "department" : "project"} on the Team tab, and it goes with it.
          </p>
        ) : (
          <Options>
            <Option
              label="Archive"
              variant="danger"
              hint="Takes it off the chart; you can bring it back, save it to your Workforce, or delete it for good."
              onClick={() =>
                actions.confirm({
                  title: `Archive ${p.title}?`,
                  message: (
                    <p>
                      It leaves the chart and its agent retires; its reviewer, QA, and security
                      assignments end. It must not lead anyone or have unfinished work. You can
                      bring it back from the Archived list, as it was.
                    </p>
                  ),
                  confirmLabel: "Archive position",
                  work: () => actions.api.archive(p.id),
                })
              }
            />
          </Options>
        )}
      </Section>
      <Refusal error={run.error} />
    </>
  );
}

function RenameForm({ p, actions }: { p: PositionInfo; actions: InspectorActions }) {
  const [title, setTitle] = useState(p.title);
  const run = useRun(actions);
  const saveHint = useId();
  const changed = title.trim() !== "" && title.trim() !== p.title;
  return (
    <Section title="Title">
      <form
        aria-label="Rename"
        onSubmit={(e) => {
          e.preventDefault();
          if (changed) void run.go(() => actions.api.update(p.id, { title: title.trim() }));
        }}
      >
        <Field
          label="Title"
          hint="Unique within its team; teammates hand work to each other by title."
        >
          {({ id, hintId }) => (
            <input
              id={id}
              aria-describedby={hintId}
              value={title}
              maxLength={120}
              onChange={(e) => setTitle(e.target.value)}
            />
          )}
        </Field>
        <div className="option">
          <Button
            type="submit"
            variant="primary"
            size="sm"
            aria-describedby={saveHint}
            disabled={run.pending || !changed}
          >
            Rename
          </Button>
          <span id={saveHint} className="option__hint">
            Keeps the new title. Its agent and conversation stay.
          </span>
        </div>
      </form>
      <Refusal error={run.error} />
    </Section>
  );
}

/** Bring back, Save to my Workforce, and Delete for good, for an archived agent. */
function ArchivedManage({
  p,
  snapshot,
  actions,
}: {
  p: PositionInfo;
  snapshot: OrgSnapshot;
  actions: InspectorActions;
}) {
  const run = useRun(actions);
  if (p.deleted || p.inWorkforce) {
    return (
      <p className="muted">
        {p.inWorkforce
          ? "It moved to your Workforce. Hire it from the Workforce list."
          : "It was deleted for good. This short record keeps its name on older work."}
      </p>
    );
  }
  const department = p.headsDepartmentId
    ? snapshot.departments.find((d) => d.id === p.headsDepartmentId)
    : undefined;
  const project = p.coordinatesProjectId
    ? snapshot.projects.find((x) => x.id === p.coordinatesProjectId)
    : undefined;

  if (department || project) {
    const kind = department ? "department" : "project";
    const unit = (department ?? project)!;
    return (
      <Section title="Archived">
        <p className="muted">
          It leads the {unit.name} {kind}, so it comes back and goes with it.
        </p>
        <Options>
          <Option
            label={`Bring back the ${unit.name} ${kind}`}
            variant="primary"
            disabled={run.pending}
            hint={`Brings back the ${kind} with everything archived with it, as it was.`}
            onClick={() => void run.go(() => actions.api.bringBack(kind, unit.id))}
          />
          <Option
            label={`Delete the ${unit.name} ${kind} for good…`}
            variant="danger"
            hint="Asks first, and offers to save its experienced agents to your Workforce."
            onClick={() => actions.deleteForGood(kind, unit.id)}
          />
        </Options>
        <Refusal error={run.error} />
      </Section>
    );
  }

  const w = p.archivedWith;
  return (
    <Section title="Archived">
      <p className="muted">Archived {archivedWithLine(p)}.</p>
      <Options>
        {w ? (
          <Option
            label={`Bring back the ${w.name} ${w.kind}`}
            variant="primary"
            disabled={run.pending}
            hint={`It was archived with the ${w.kind}, so it comes back with it.`}
            onClick={() =>
              void run.go(() =>
                actions.api.bringBack(w.kind === "department" ? "department" : "project", w.id),
              )
            }
          />
        ) : (
          <Option
            label="Bring back"
            variant="primary"
            disabled={run.pending}
            hint="Puts it back where it was, with all its settings. A full-time position gets a new agent."
            onClick={() => void run.go(() => actions.api.bringBack("position", p.id))}
          />
        )}
        <Option
          label="Save to my Workforce"
          disabled={run.pending}
          hint="Keeps it with its settings, experience, and lessons, to hire again into any team."
          onClick={() => void run.go(() => actions.api.saveToWorkforce(p.id))}
        />
        <Option
          label="Delete for good…"
          variant="danger"
          hint="Asks first. It cannot be undone; a short record keeps its name on older work."
          onClick={() => actions.deleteForGood("position", p.id)}
        />
      </Options>
      <Refusal error={run.error} />
    </Section>
  );
}
