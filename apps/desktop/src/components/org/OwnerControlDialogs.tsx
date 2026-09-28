/**
 * Dialogs for the owner's control over workers (Phase 17): delete for good, with the offer to
 * save experienced agents to your Workforce (ADR-043, ADR-045); hire an agent from your
 * Workforce; and your own specialties (ADR-042).
 */
import { useEffect, useState, type FormEvent } from "react";
import type {
  Capability,
  DeletionPreview,
  ModelFeature,
  OrgSnapshot,
  RoleJob,
  SavedAgentInfo,
  SpecialtyInfo,
  SpecialtyInput,
} from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { previewDeleteForGood, toCommandError, type ArchivedKind } from "../../api/commands";
import { CAPABILITIES, CAPABILITY_LABEL } from "../../guard/format";
import { experienceLine } from "../../org/control";
import { plural } from "../../org/format";
import { hireRefusal, positionMap, supervisorChoices } from "../../org/rules";
import { rankName, titlesOf } from "../../org/titles";
import { FEATURES, FEATURE_LABEL } from "../../routing/format";
import { Modal } from "./Modal";
import {
  EMPTY_JOB,
  JOB_FIELDS,
  MAX_OBJECTIVE_FIELD,
  OWNER_VALUE,
  jobLines,
  positionChoiceLabel,
  useSubmit,
} from "./dialogHelpers";
import { Field, Footer, FormError } from "./OrgDialogs";

const KIND_WORD: Record<ArchivedKind, string> = {
  position: "agent",
  project: "project",
  department: "department",
};

// ---- Delete for good ------------------------------------------------------------------------

/**
 * Ask before deleting an archived agent, project, or department for good. Lists everything that
 * goes; the agents whose experience is above your organization's average start out checked to be
 * saved to your Workforce. Unchecked agents are deleted for good.
 */
export function DeleteForGoodDialog({
  kind,
  id,
  onCancel,
  onDelete,
}: {
  kind: ArchivedKind;
  id: string;
  onCancel: () => void;
  onDelete: (save: string[]) => Promise<string | null>;
}) {
  const [preview, setPreview] = useState<DeletionPreview | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [save, setSave] = useState<Set<string>>(new Set());
  const { pending, error, run } = useSubmit();

  useEffect(() => {
    let live = true;
    previewDeleteForGood(kind, id).then(
      (p) => {
        if (!live) return;
        setPreview(p);
        setSave(new Set(p.agents.filter((a) => a.experience.experienced).map((a) => a.positionId)));
      },
      (reason: unknown) => {
        if (live) setLoadError(toCommandError(reason).message);
      },
    );
    return () => {
      live = false;
    };
  }, [kind, id]);

  const toggle = (positionId: string, on: boolean) =>
    setSave((s) => {
      const next = new Set(s);
      if (on) next.add(positionId);
      else next.delete(positionId);
      return next;
    });

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!preview) return;
    // Parents first, as the preview lists them.
    void run(() => onDelete(preview.agents.map((a) => a.positionId).filter((a) => save.has(a))));
  };

  const title = preview ? `Delete ${preview.name} for good?` : "Delete for good?";
  const deleted = preview ? preview.agents.length - save.size : 0;
  return (
    <Modal title={title} onClose={onCancel} wide>
      <form className="modal__body" aria-label="Delete for good" onSubmit={submit}>
        {loadError ? (
          <p className="form-error" role="alert">
            {loadError}
          </p>
        ) : !preview ? (
          <p className="muted">Finding everything that goes with it…</p>
        ) : (
          <>
            <p>
              This cannot be undone. The {KIND_WORD[kind]} leaves every list; a short record stays
              in the Ledger so older work still shows its name. Nothing on your PC is deleted.
            </p>
            {(preview.projects.length > 0 || preview.departments.length > 0) && (
              <p>
                Going with it:{" "}
                {[
                  ...preview.departments.map((d) => `the ${d} department`),
                  ...preview.projects.map((x) => `the ${x} project`),
                ].join(", ")}
                .
              </p>
            )}
            {preview.agents.length === 0 ? (
              <p className="muted">No agents go with it.</p>
            ) : (
              <fieldset className="fieldset">
                <legend>Save to my Workforce</legend>
                <p className="muted">
                  Agents you check move to your Workforce, to hire again later with their settings,
                  experience, and lessons. The rest are deleted for good. Those above your
                  organization&apos;s average experience ({preview.averageExperience}) are marked
                  experienced and start out checked.
                </p>
                <ul className="delete-agents">
                  {preview.agents.map((a) => (
                    <li key={a.positionId}>
                      <label className="check">
                        <input
                          type="checkbox"
                          checked={save.has(a.positionId)}
                          onChange={(e) => toggle(a.positionId, e.target.checked)}
                        />
                        <span>
                          {a.title}
                          <span className="check__hint">
                            {a.roleName} · experience {experienceLine(a.experience)}
                            {a.experience.experienced ? " · experienced" : ""}
                          </span>
                        </span>
                      </label>
                    </li>
                  ))}
                </ul>
              </fieldset>
            )}
            <p className="muted" role="status">
              {plural(save.size, "agent")} saved to your Workforce; {plural(deleted, "agent")}{" "}
              deleted for good.
            </p>
          </>
        )}
        <FormError error={error} />
        <footer className="modal__footer">
          <Button variant="quiet" onClick={onCancel}>
            Cancel
          </Button>
          <Button type="submit" variant="danger" disabled={pending || !preview}>
            {pending ? "Deleting…" : "Delete for good"}
          </Button>
        </footer>
      </form>
    </Modal>
  );
}

// ---- Hire from your Workforce ---------------------------------------------------------------

/** Hire an agent from your Workforce into a team: choose who it reports to, and its title. */
export function HireFromWorkforceDialog({
  snapshot,
  saved,
  onCancel,
  onSubmit,
  onNewDepartment,
  onNewProject,
}: {
  snapshot: OrgSnapshot;
  saved: SavedAgentInfo;
  onCancel: () => void;
  onSubmit: (reportsTo: string | null, title: string) => Promise<string | null>;
  /** A Manager comes with a department: start one led by this agent. */
  onNewDepartment: (savedId: string) => void;
  /** A Supervisor comes with a project: start one led by this agent. */
  onNewProject: (savedId: string) => void;
}) {
  const role = snapshot.roles.find((r) => r.id === saved.roleId) ?? null;
  const t = titlesOf(snapshot);
  const byId = positionMap(snapshot);
  const choices = role ? supervisorChoices(snapshot, role) : [];
  const [reportsTo, setReportsTo] = useState<string | null>(choices[0] ?? null);
  const [title, setTitle] = useState(saved.title);
  const { pending, error, run } = useSubmit();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(() => onSubmit(reportsTo, title.trim()));
  };
  const leadsUnit = role?.kind === "departmentManager" || role?.kind === "projectCoordinator";
  return (
    <Modal title={`Hire ${saved.title}`} onClose={onCancel}>
      <form className="modal__body" aria-label="Hire from my Workforce" onSubmit={submit}>
        <p className="muted">
          {saved.roleName}
          {saved.specialty ? ` (${saved.specialty})` : ""} · experience{" "}
          {experienceLine(saved.experience)}. It comes back with its settings and lessons.
        </p>
        {!role ? (
          <p className="hint">Its role no longer exists, so it cannot be hired.</p>
        ) : leadsUnit ? (
          <>
            <p className="hint">
              {role.kind === "departmentManager"
                ? `A ${rankName(t, "departmentManager")} comes with a department: create one and it leads it.`
                : `A ${rankName(t, "projectCoordinator")} comes with a project: create one and it leads it.`}
            </p>
            <footer className="modal__footer">
              <Button variant="quiet" onClick={onCancel}>
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={() =>
                  role.kind === "departmentManager"
                    ? onNewDepartment(saved.id)
                    : onNewProject(saved.id)
                }
              >
                {role.kind === "departmentManager" ? "New department" : "New project"}
              </Button>
            </footer>
          </>
        ) : (
          <>
            <Field
              label="Reports to"
              hint={
                choices.length === 0
                  ? (hireRefusal(snapshot, role, null) ?? undefined)
                  : "The team it joins."
              }
            >
              <select
                value={reportsTo ?? OWNER_VALUE}
                disabled={choices.length === 0}
                onChange={(e) =>
                  setReportsTo(e.target.value === OWNER_VALUE ? null : e.target.value)
                }
              >
                {choices.map((id) =>
                  id === null ? (
                    <option key={OWNER_VALUE} value={OWNER_VALUE}>
                      You ({rankName(t, "owner")})
                    </option>
                  ) : (
                    <option key={id} value={id}>
                      {byId.get(id) ? positionChoiceLabel(t, byId.get(id)!) : id}
                    </option>
                  ),
                )}
              </select>
            </Field>
            <Field
              label="Title"
              hint="Unique within its team; teammates hand work to each other by title."
            >
              <input
                value={title}
                maxLength={120}
                required
                onChange={(e) => setTitle(e.target.value)}
              />
            </Field>
            <FormError error={error} />
            <Footer
              pending={pending}
              label="Hire"
              disabled={choices.length === 0 || title.trim() === ""}
              onCancel={onCancel}
            />
          </>
        )}
      </form>
    </Modal>
  );
}

// ---- Your own specialties -------------------------------------------------------------------

/**
 * A specialty of your own for a role, or a change to one (ADR-042). Its lines are added to its
 * role's working instructions for the agents that have it; its suggestions never change anything
 * by themselves.
 */
export function SpecialtyDialog({
  snapshot,
  roleId,
  specialty,
  onCancel,
  onSubmit,
  onRemove,
}: {
  snapshot: OrgSnapshot;
  /** The role it belongs to (a new specialty). */
  roleId: string;
  /** The specialty to change; a new one when absent. */
  specialty?: SpecialtyInfo | undefined;
  onCancel: () => void;
  onSubmit: (input: SpecialtyInput) => Promise<string | null>;
  onRemove?: (() => Promise<string | null>) | undefined;
}) {
  const role = snapshot.roles.find((r) => r.id === roleId);
  const [name, setName] = useState(specialty?.name ?? "");
  const [title, setTitle] = useState(specialty?.title ?? "");
  const [job, setJob] = useState<Record<keyof RoleJob, string>>(() => {
    const j = specialty?.job ?? EMPTY_JOB;
    return {
      duties: j.duties.join("\n"),
      returns: j.returns.join("\n"),
      limits: j.limits.join("\n"),
      askLead: j.askLead.join("\n"),
    };
  });
  const [needs, setNeeds] = useState<ModelFeature[]>(specialty?.suggest.needs ?? []);
  const [permissions, setPermissions] = useState<string[]>(specialty?.suggest.permissions ?? []);
  const { pending, error, run } = useSubmit();
  const editing = specialty !== undefined;
  const heading = editing
    ? `Edit specialty: ${specialty.name}`
    : `New specialty for ${role?.name ?? "a role"}`;
  const toggle = <T,>(list: T[], item: T, on: boolean) =>
    on ? [...list.filter((x) => x !== item), item] : list.filter((x) => x !== item);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const input: SpecialtyInput = {
      name: name.trim(),
      title: title.trim(),
      job: {
        duties: jobLines(job.duties),
        returns: jobLines(job.returns),
        limits: jobLines(job.limits),
        askLead: jobLines(job.askLead),
      },
      suggest: {
        needs,
        minContextTokens: specialty?.suggest.minContextTokens ?? null,
        models: specialty?.suggest.models ?? [],
        permissions,
      },
    };
    void run(() => onSubmit(editing ? input : { ...input, roleId }));
  };

  return (
    <Modal title={heading} onClose={onCancel} wide>
      <form className="modal__body" aria-label={heading} onSubmit={submit}>
        <Field label="Name" hint="The area of work, e.g. “Payments” or “Mobile”.">
          <input value={name} maxLength={80} required onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field
          label="Suggested title (optional)"
          hint={`Offered as the title when you hire ${role?.name ?? "the role"} with it.`}
        >
          <input value={title} maxLength={80} onChange={(e) => setTitle(e.target.value)} />
        </Field>
        <fieldset className="choices">
          <legend>What it adds to the {role?.name ?? ""} role&apos;s instructions</legend>
          {JOB_FIELDS.map((f) => (
            <Field key={f.key} label={f.label} hint={f.hint}>
              <textarea
                value={job[f.key]}
                rows={2}
                maxLength={MAX_OBJECTIVE_FIELD}
                onChange={(e) => setJob((j) => ({ ...j, [f.key]: e.target.value }))}
              />
            </Field>
          ))}
        </fieldset>
        <fieldset className="choices">
          <legend>Suggestions (shown on the agent&apos;s panel; never applied on their own)</legend>
          {FEATURES.map((f) => (
            <label key={f} className="check">
              <input
                type="checkbox"
                checked={needs.includes(f)}
                onChange={(e) => setNeeds(toggle(needs, f, e.target.checked))}
              />
              <span>A model that {FEATURE_LABEL[f].toLowerCase()}</span>
            </label>
          ))}
          {CAPABILITIES.map((c: Capability) => (
            <label key={c} className="check">
              <input
                type="checkbox"
                checked={permissions.includes(c)}
                onChange={(e) => setPermissions(toggle(permissions, c, e.target.checked))}
              />
              <span>Permission: {CAPABILITY_LABEL[c]}</span>
            </label>
          ))}
        </fieldset>
        <FormError error={error} />
        <footer className="modal__footer">
          {editing && onRemove && (
            <Button variant="danger" disabled={pending} onClick={() => void run(onRemove)}>
              Remove specialty
            </Button>
          )}
          <Button variant="quiet" onClick={onCancel}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" disabled={pending || name.trim() === ""}>
            {pending ? "Working…" : editing ? "Save specialty" : "Add specialty"}
          </Button>
        </footer>
      </form>
    </Modal>
  );
}
