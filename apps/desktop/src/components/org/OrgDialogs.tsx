import { useState, type FormEvent, type ReactNode } from "react";
import type {
  DepartmentInfo,
  DepartmentInput,
  DevelopmentInput,
  HireInput,
  LeadInput,
  OrgSnapshot,
  PositionInfo,
  PositionKind,
  ProjectInfo,
  ProjectInput,
  RoleInfo,
  RoleInput,
  RoleJob,
  RoleUpdate,
  Staffing,
} from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { STAFFING_LABEL } from "../../org/format";
import {
  defaultRuntime,
  hireRefusal,
  hireableRoles,
  positionMap,
  supervisorChoices,
} from "../../org/rules";
import { RANKS, rankName, roleLabel, titlesOf, withArticle, type TitleSet } from "../../org/titles";
import { usePermissionSets } from "../../guard/usePermissions";
import { useRoutingOnce } from "../../routing/useRouting";
import { ModelPicker } from "../models/ModelPicker";
import { PILL_TONE } from "../tones";
import {
  EMPTY_JOB,
  JOB_FIELDS,
  MAX_OBJECTIVE_FIELD,
  OWNER_VALUE,
  jobLines,
  positionChoiceLabel,
  useSubmit,
} from "./dialogHelpers";
import { Modal } from "./Modal";
import { RuntimeOptions } from "./RuntimeOptions";
import { reusableWorkers } from "./teamReuse";
import { runtimeChoiceLabel, subscriptionInstead } from "./runtimeChoices";

/** Resolves with the refusal to show, or `null` once done. */
export type Submit<T> = (input: T) => Promise<string | null>;

function projectOf(snapshot: OrgSnapshot, positionId: string | null): ProjectInfo | null {
  if (!positionId) return null;
  const projectId = positionMap(snapshot).get(positionId)?.projectId ?? null;
  return snapshot.projects.find((p) => p.id === projectId) ?? null;
}

/** "Full-time: one agent holds it and keeps its conversation." */
const STAFFING_HINT: Record<Staffing, string> = {
  persistent: "Full-time: one agent holds it and keeps its conversation.",
  onDemand: "On call: a new worker is brought in for each task and leaves when it is done.",
};

function capitalized(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

export function FormError({ error }: { error: string | null }) {
  return error ? (
    <p className="form-error" role="alert">
      {error}
    </p>
  ) : null;
}

export function Footer({
  pending,
  label,
  disabled = false,
  onCancel,
}: {
  pending: boolean;
  label: string;
  disabled?: boolean;
  onCancel: () => void;
}) {
  return (
    <footer className="modal__footer">
      <Button variant="quiet" onClick={onCancel}>
        Cancel
      </Button>
      <Button type="submit" variant="primary" disabled={pending || disabled}>
        {pending ? "Working…" : label}
      </Button>
    </footer>
  );
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
      {hint && <small className="field__hint">{hint}</small>}
    </label>
  );
}

/** The AI tool choice: "" (automatic: the role's model choices) or a fixed AI tool. */
function RuntimeField({
  snapshot,
  value,
  onChange,
  project,
}: {
  snapshot: OrgSnapshot;
  value: string;
  onChange: (id: string) => void;
  project: ProjectInfo | null;
}) {
  const refused = value !== "" && project !== null && !project.allowedRuntimes.includes(value);
  const instead = value === "" ? null : subscriptionInstead(snapshot, value);
  return (
    <Field
      label="AI tool"
      hint={
        instead ? (
          <span className="field__warn">{instead}</span>
        ) : refused ? (
          <span className="field__warn">
            {project.name} does not allow this AI tool
            {project.allowedRuntimes.length > 0
              ? ` (allowed: ${project.allowedRuntimes.map((r) => runtimeChoiceLabel(snapshot, r)).join(", ")})`
              : "; it allows none yet"}
            .
          </span>
        ) : value === "" ? (
          "Plenipo picks the AI tool and model for each worker from the role's model choices (Settings → AI models) and says why."
        ) : undefined
      }
    >
      <select value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">Automatic (the role&apos;s model choices)</option>
        <RuntimeOptions snapshot={snapshot} current={value} />
      </select>
    </Field>
  );
}

/** A fixed AI tool and its model (if named); automatic when `runtimeId` is "". */
function withRuntime<T extends object>(
  input: T,
  runtimeId: string,
  model: string,
): T & { runtimeId?: string; model?: string } {
  if (runtimeId === "") return input;
  const m = model.trim();
  return m ? { ...input, runtimeId, model: m } : { ...input, runtimeId };
}

// ---- Hire -----------------------------------------------------------------------------------

export function HireDialog({
  snapshot,
  roleId: initialRole,
  reportsTo: initialSupervisor,
  onCancel,
  onSubmit,
  onHireSaved,
}: {
  snapshot: OrgSnapshot;
  roleId: string | null;
  /** `undefined`: choose a sensible supervisor; `null`: the owner. */
  reportsTo?: string | null;
  onCancel: () => void;
  onSubmit: Submit<HireInput>;
  /** Hire an agent from your Workforce instead (ADR-045). */
  onHireSaved?: (
    savedId: string,
    reportsTo: string | null,
    title: string,
  ) => Promise<string | null>;
}) {
  const roles = hireableRoles(snapshot);
  const byId = positionMap(snapshot);
  const t = titlesOf(snapshot);
  const firstRole =
    roles.find((r) => r.id === initialRole) ??
    roles.find((r) => r.kind === "worker") ??
    roles[0] ??
    null;
  const pickSupervisor = (role: RoleInfo | null, wanted: string | null | undefined) => {
    if (!role) return null;
    if (wanted !== undefined && hireRefusal(snapshot, role, wanted) === null) return wanted;
    return supervisorChoices(snapshot, role)[0] ?? null;
  };
  const [roleId, setRoleId] = useState(firstRole?.id ?? "");
  const role = roles.find((r) => r.id === roleId) ?? null;
  const [title, setTitle] = useState(firstRole?.name ?? "");
  const [titleEdited, setTitleEdited] = useState(false);
  const [reportsTo, setReportsTo] = useState<string | null>(() =>
    pickSupervisor(firstRole, initialSupervisor),
  );
  const project = projectOf(snapshot, reportsTo);
  const [runtimeId, setRuntimeId] = useState("");
  const [model, setModel] = useState("");
  const [vacant, setVacant] = useState(false);
  const [specialtyId, setSpecialtyId] = useState("");
  const [savedId, setSavedId] = useState("");
  const { pending, error, run } = useSubmit();
  const routing = useRoutingOnce();
  const hireable = new Set(roles.map((r) => r.id));
  const savedAgents = snapshot.workforce.filter((w) => hireable.has(w.roleId));
  const saved = savedAgents.find((w) => w.id === savedId) ?? null;
  const specialties = role?.specialties ?? [];

  const choices = role ? supervisorChoices(snapshot, role) : [];
  const wantedRefusal =
    role && initialSupervisor !== undefined && initialSupervisor !== reportsTo
      ? hireRefusal(snapshot, role, initialSupervisor)
      : null;

  const changeRole = (id: string) => {
    const next = roles.find((r) => r.id === id) ?? null;
    setRoleId(id);
    setSpecialtyId("");
    if (!titleEdited) setTitle(next?.name ?? "");
    const supervisor = pickSupervisor(next, reportsTo);
    changeSupervisor(supervisor);
    if (next?.staffing !== "persistent") setVacant(false);
  };
  const changeSpecialty = (id: string) => {
    setSpecialtyId(id);
    const chosen = specialties.find((x) => x.id === id);
    if (!titleEdited) setTitle(chosen?.title || role?.name || "");
  };
  const changeSaved = (id: string) => {
    setSavedId(id);
    const next = savedAgents.find((w) => w.id === id);
    if (next) {
      changeRole(next.roleId);
      setTitle(next.title);
      setTitleEdited(false);
    } else if (!titleEdited) {
      setTitle(role?.name ?? "");
    }
  };
  const changeSupervisor = (id: string | null) => {
    setReportsTo(id);
    const allowed = projectOf(snapshot, id)?.allowedRuntimes ?? null;
    if (runtimeId !== "" && allowed && !allowed.includes(runtimeId)) {
      setRuntimeId(defaultRuntime(snapshot, allowed));
      setModel("");
    }
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!role) return;
    if (saved && onHireSaved) {
      void run(() => onHireSaved(saved.id, reportsTo, title.trim()));
      return;
    }
    const input: HireInput = withRuntime(
      { roleId: role.id, title: title.trim(), reportsTo },
      runtimeId,
      model,
    );
    const withSpecialty = specialtyId ? { ...input, specialtyId } : input;
    void run(() => onSubmit(vacant ? { ...withSpecialty, vacant: true } : withSpecialty));
  };

  return (
    <Modal title="Hire" onClose={onCancel} tour="hire-dialog">
      <form className="modal__body" aria-label="Hire" onSubmit={submit}>
        {onHireSaved && savedAgents.length > 0 && (
          <Field
            label="Who"
            hint="An agent from your Workforce comes with its settings, experience, and lessons."
          >
            <select value={savedId} onChange={(e) => changeSaved(e.target.value)}>
              <option value="">A new agent</option>
              <optgroup label="From my Workforce">
                {savedAgents.map((w) => (
                  <option key={w.id} value={w.id}>
                    {w.title} — {w.roleName}, experience {w.experience.score}
                  </option>
                ))}
              </optgroup>
            </select>
          </Field>
        )}
        <Field
          label="Role"
          hint={role ? `${role.description} ${STAFFING_HINT[role.staffing]}` : undefined}
        >
          <select
            value={roleId}
            onChange={(e) => changeRole(e.target.value)}
            disabled={saved !== null}
            required
          >
            <optgroup label="Leadership">
              {roles
                .filter((r) => r.kind === "superintendent")
                .map((r) => (
                  <option key={r.id} value={r.id}>
                    {roleLabel(t, r)}
                  </option>
                ))}
            </optgroup>
            <optgroup label="Team members">
              {roles
                .filter((r) => r.kind === "worker")
                .map((r) => (
                  <option key={r.id} value={r.id}>
                    {r.name}
                    {r.staffing === "persistent" ? " (full-time)" : ""}
                  </option>
                ))}
            </optgroup>
          </select>
        </Field>
        {!saved && specialties.length > 0 && (
          <Field
            label="Specialty"
            hint="Adds lines for one area of work to its instructions, and suggests a title."
          >
            <select value={specialtyId} onChange={(e) => changeSpecialty(e.target.value)}>
              <option value="">None: the {role?.name} role&apos;s job alone</option>
              {specialties.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.name}
                  {x.builtIn ? "" : " (yours)"}
                </option>
              ))}
            </select>
          </Field>
        )}
        <Field
          label="Title"
          hint="Unique within its team; team members hand work to each other by title."
        >
          <input
            value={title}
            maxLength={120}
            required
            onChange={(e) => {
              setTitle(e.target.value);
              setTitleEdited(true);
            }}
          />
        </Field>
        <Field
          label="Reports to"
          hint={wantedRefusal ? <span className="field__warn">{wantedRefusal}</span> : undefined}
        >
          <select
            value={reportsTo ?? OWNER_VALUE}
            onChange={(e) =>
              changeSupervisor(e.target.value === OWNER_VALUE ? null : e.target.value)
            }
            disabled={choices.length === 0}
          >
            {choices.map((id) =>
              id === null ? (
                <option key={OWNER_VALUE} value={OWNER_VALUE}>
                  You ({rankName(t, "owner")})
                </option>
              ) : (
                <option key={id} value={id}>
                  {byId.get(id) ? positionChoiceLabel(t, byId.get(id) as PositionInfo) : id}
                </option>
              ),
            )}
          </select>
        </Field>
        {role && choices.length === 0 && (
          <p className="hint">
            {capitalized(withArticle(roleLabel(t, role)))} reports to a full-time position. Hire{" "}
            {withArticle(rankName(t, "superintendent"))} or create a department first.
          </p>
        )}
        {!saved && (
          <RuntimeField
            snapshot={snapshot}
            value={runtimeId}
            onChange={(id) => {
              setRuntimeId(id);
              setModel("");
            }}
            project={project}
          />
        )}
        {!saved && runtimeId !== "" && (
          <ModelPicker routing={routing} runtimeId={runtimeId} value={model} onChange={setModel} />
        )}
        {!saved && role?.staffing === "persistent" && (
          <label className="check">
            <input type="checkbox" checked={vacant} onChange={(e) => setVacant(e.target.checked)} />
            <span>
              Leave vacant
              <span className="check__hint">Create the position now and hire its agent later.</span>
            </span>
          </label>
        )}
        <FormError error={error} />
        <Footer
          pending={pending}
          label="Hire"
          disabled={!role || choices.length === 0 || title.trim() === ""}
          onCancel={onCancel}
        />
      </form>
    </Modal>
  );
}

// ---- Departments ----------------------------------------------------------------------------

function leadRoles(snapshot: OrgSnapshot, kind: PositionKind): RoleInfo[] {
  return snapshot.roles.filter((r) => r.kind === kind);
}

function LeadFields({
  snapshot,
  kind,
  lead,
  onChange,
  project,
  what,
}: {
  snapshot: OrgSnapshot;
  kind: PositionKind;
  lead: LeadState;
  onChange: (patch: Partial<LeadState>) => void;
  project: ProjectInfo | null;
  what: string;
}) {
  const roles = leadRoles(snapshot, kind);
  const t = titlesOf(snapshot);
  const routing = useRoutingOnce();
  const kindRoles = new Set(roles.map((r) => r.id));
  const savedAgents = snapshot.workforce.filter((w) => kindRoles.has(w.roleId));
  const saved = savedAgents.find((w) => w.id === lead.fromWorkforce) ?? null;
  return (
    <fieldset className="fieldset">
      <legend>{what}</legend>
      {savedAgents.length > 0 && (
        <Field
          label="Who"
          hint="An agent from your Workforce comes with its settings, experience, and lessons."
        >
          <select
            value={lead.fromWorkforce}
            onChange={(e) => {
              const next = savedAgents.find((w) => w.id === e.target.value);
              onChange(
                next
                  ? {
                      fromWorkforce: next.id,
                      roleId: next.roleId,
                      title: next.title,
                      titleEdited: true,
                    }
                  : { fromWorkforce: "" },
              );
            }}
          >
            <option value="">A new agent</option>
            <optgroup label="From my Workforce">
              {savedAgents.map((w) => (
                <option key={w.id} value={w.id}>
                  {w.title} — experience {w.experience.score}
                </option>
              ))}
            </optgroup>
          </select>
        </Field>
      )}
      <Field label="Role">
        <select
          value={lead.roleId}
          onChange={(e) => onChange({ roleId: e.target.value })}
          disabled={saved !== null}
          required
        >
          {roles.map((r) => (
            <option key={r.id} value={r.id}>
              {roleLabel(t, r)}
            </option>
          ))}
        </select>
      </Field>
      <Field label="Title">
        <input
          value={lead.title}
          maxLength={120}
          required
          onChange={(e) => onChange({ title: e.target.value, titleEdited: true })}
        />
      </Field>
      {!saved && (
        <RuntimeField
          snapshot={snapshot}
          value={lead.runtimeId}
          onChange={(runtimeId) => onChange({ runtimeId, model: "" })}
          project={project}
        />
      )}
      {!saved && lead.runtimeId !== "" && (
        <ModelPicker
          routing={routing}
          runtimeId={lead.runtimeId}
          value={lead.model}
          onChange={(model) => onChange({ model })}
        />
      )}
      {!saved && (
        <label className="check">
          <input
            type="checkbox"
            checked={lead.vacant}
            onChange={(e) => onChange({ vacant: e.target.checked })}
          />
          <span>
            Leave vacant
            <span className="check__hint">Create the position now and hire its agent later.</span>
          </span>
        </label>
      )}
    </fieldset>
  );
}

interface LeadState {
  roleId: string;
  title: string;
  titleEdited: boolean;
  runtimeId: string;
  model: string;
  vacant: boolean;
  /** An agent from your Workforce ("" : a new agent). */
  fromWorkforce: string;
}

function newLead(snapshot: OrgSnapshot, kind: PositionKind, fromWorkforce = ""): LeadState {
  const roles = leadRoles(snapshot, kind);
  const role = roles.find((r) => r.template) ?? roles[0];
  const saved = snapshot.workforce.find(
    (w) => w.id === fromWorkforce && roles.some((r) => r.id === w.roleId),
  );
  return {
    roleId: saved?.roleId ?? role?.id ?? "",
    title: saved?.title ?? "",
    titleEdited: saved !== undefined,
    runtimeId: "",
    model: "",
    vacant: false,
    fromWorkforce: saved?.id ?? "",
  };
}

function leadInput(lead: LeadState, fallbackTitle: string): LeadInput {
  if (lead.fromWorkforce) {
    return {
      roleId: lead.roleId,
      title: (lead.title || fallbackTitle).trim(),
      fromWorkforce: lead.fromWorkforce,
    };
  }
  const input: LeadInput = withRuntime(
    {
      roleId: lead.roleId,
      title: (lead.titleEdited ? lead.title : lead.title || fallbackTitle).trim(),
    },
    lead.runtimeId,
    lead.model,
  );
  return lead.vacant ? { ...input, vacant: true } : input;
}

export function NewDepartmentDialog({
  snapshot,
  reportsTo: initialSupervisor = null,
  fromWorkforce = "",
  onCancel,
  onSubmit,
  onTemplate,
}: {
  snapshot: OrgSnapshot;
  reportsTo?: string | null;
  /** Its manager comes from your Workforce (ADR-045). */
  fromWorkforce?: string;
  onCancel: () => void;
  onSubmit: Submit<DepartmentInput>;
  /** Add a department from a template instead (Phase 25, item 2.8). */
  onTemplate?: Submit<string> | undefined;
}) {
  // The templates for departments this organization doesn't have yet.
  const templates = snapshot.templates.departments.filter(
    (d) => !snapshot.departments.some((x) => x.active && x.name === d.name),
  );
  const [template, setTemplate] = useState(templates[0]?.id ?? "");
  const chosen = templates.find((d) => d.id === template);
  const superintendents = snapshot.positions.filter((p) => p.active && p.kind === "superintendent");
  const t = titlesOf(snapshot);
  const manager = rankName(t, "departmentManager");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [reportsTo, setReportsTo] = useState<string | null>(
    superintendents.some((p) => p.id === initialSupervisor) ? initialSupervisor : null,
  );
  const [lead, setLead] = useState(() => newLead(snapshot, "departmentManager", fromWorkforce));
  const { pending, error, run } = useSubmit();
  const fallbackTitle = `${name.trim() || "Department"} Manager`;
  const shownLead = lead.titleEdited ? lead : { ...lead, title: fallbackTitle };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const input: DepartmentInput = {
      name: name.trim(),
      description: description.trim(),
      head: leadInput(shownLead, fallbackTitle),
    };
    void run(() => onSubmit(reportsTo ? { ...input, reportsTo } : input));
  };

  return (
    <Modal title="New department" onClose={onCancel} wide tour="new-department">
      <form className="modal__body" aria-label="New department" onSubmit={submit}>
        {onTemplate && templates.length > 0 && !fromWorkforce && (
          <fieldset className="choices">
            <legend>Start from a template</legend>
            <Field label="Template" hint={chosen ? chosen.adds.join(" · ") : undefined}>
              <select value={template} onChange={(e) => setTemplate(e.target.value)}>
                {templates.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.name}: {d.description}
                  </option>
                ))}
              </select>
            </Field>
            <div className="actions">
              <Button
                size="sm"
                disabled={pending || !chosen}
                onClick={() => void run(() => onTemplate(template))}
              >
                {chosen ? `Add ${chosen.name}` : "Add it"}
              </Button>
              <span className="muted">Or make your own below.</span>
            </div>
          </fieldset>
        )}
        <Field label="Name">
          <input value={name} maxLength={120} required onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="Description">
          <textarea
            value={description}
            rows={2}
            maxLength={MAX_OBJECTIVE_FIELD}
            onChange={(e) => setDescription(e.target.value)}
          />
        </Field>
        <Field label={`Its ${manager} reports to`}>
          <select
            value={reportsTo ?? OWNER_VALUE}
            onChange={(e) => setReportsTo(e.target.value === OWNER_VALUE ? null : e.target.value)}
          >
            <option value={OWNER_VALUE}>You ({rankName(t, "owner")})</option>
            {superintendents.map((p) => (
              <option key={p.id} value={p.id}>
                {positionChoiceLabel(t, p)}
              </option>
            ))}
          </select>
        </Field>
        <LeadFields
          snapshot={snapshot}
          kind="departmentManager"
          lead={shownLead}
          onChange={(patch) => setLead((l) => ({ ...l, ...patch }))}
          project={null}
          what={`Its ${manager}`}
        />
        <FormError error={error} />
        <Footer
          pending={pending}
          label="Create department"
          disabled={name.trim() === "" || shownLead.roleId === ""}
          onCancel={onCancel}
        />
      </form>
    </Modal>
  );
}

export function EditDepartmentDialog({
  department,
  onCancel,
  onSubmit,
}: {
  department: DepartmentInfo;
  onCancel: () => void;
  onSubmit: Submit<DepartmentInput>;
}) {
  const [name, setName] = useState(department.name);
  const [description, setDescription] = useState(department.description);
  const [active, setActive] = useState(department.active);
  const { pending, error, run } = useSubmit();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(() => onSubmit({ name: name.trim(), description: description.trim(), active }));
  };
  return (
    <Modal title={`Edit ${department.name}`} onClose={onCancel}>
      <form className="modal__body" aria-label="Edit department" onSubmit={submit}>
        <Field label="Name">
          <input value={name} maxLength={120} required onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="Description">
          <textarea
            value={description}
            rows={3}
            maxLength={MAX_OBJECTIVE_FIELD}
            onChange={(e) => setDescription(e.target.value)}
          />
        </Field>
        <label className="check">
          <input type="checkbox" checked={active} onChange={(e) => setActive(e.target.checked)} />
          <span>
            Active
            <span className="check__hint">An inactive department takes no new projects.</span>
          </span>
        </label>
        <FormError error={error} />
        <Footer pending={pending} label="Save" disabled={name.trim() === ""} onCancel={onCancel} />
      </form>
    </Modal>
  );
}

// ---- Projects -------------------------------------------------------------------------------

interface ProjectSettingsState {
  name: string;
  description: string;
  repositoryUrl: string;
  localPath: string;
  allowedRuntimes: string[];
  capabilityProfile: string;
  branchPerObjective: boolean;
}

function settingsInput(s: ProjectSettingsState): ProjectInput {
  const input: ProjectInput = {
    name: s.name.trim(),
    description: s.description.trim(),
    allowedRuntimes: s.allowedRuntimes,
    branchPerObjective: s.branchPerObjective,
  };
  const repositoryUrl = s.repositoryUrl.trim();
  const localPath = s.localPath.trim();
  const capabilityProfile = s.capabilityProfile.trim();
  return {
    ...input,
    ...(repositoryUrl ? { repositoryUrl } : {}),
    ...(localPath ? { localPath } : {}),
    ...(capabilityProfile ? { capabilityProfile } : {}),
  };
}

function ProjectSettingsFields({
  snapshot,
  value,
  onChange,
}: {
  snapshot: OrgSnapshot;
  value: ProjectSettingsState;
  onChange: (patch: Partial<ProjectSettingsState>) => void;
}) {
  const sets = usePermissionSets();
  const toggle = (id: string, on: boolean) =>
    onChange({
      allowedRuntimes: on
        ? [...value.allowedRuntimes.filter((r) => r !== id), id]
        : value.allowedRuntimes.filter((r) => r !== id),
    });
  return (
    <>
      <Field label="Name">
        <input
          value={value.name}
          maxLength={120}
          required
          onChange={(e) => onChange({ name: e.target.value })}
        />
      </Field>
      <Field label="Description">
        <textarea
          value={value.description}
          rows={2}
          maxLength={MAX_OBJECTIVE_FIELD}
          onChange={(e) => onChange({ description: e.target.value })}
        />
      </Field>
      <fieldset className="choices">
        <legend>Allowed AI tools — its team may use only these (none allows none)</legend>
        {snapshot.runtimes.map((r) => (
          <label key={r.id} className="choice">
            <input
              type="checkbox"
              checked={value.allowedRuntimes.includes(r.id)}
              onChange={(e) => toggle(r.id, e.target.checked)}
            />
            <span className="choice__label">
              {r.label}
              {r.paid ? " · paid per use" : ""}
            </span>
            {!r.ready && <StatusPill status={PILL_TONE.warn} label="Not ready" />}
          </label>
        ))}
      </fieldset>
      <Field label="Repository URL (optional)">
        <input
          value={value.repositoryUrl}
          maxLength={2000}
          placeholder="https://github.com/…"
          onChange={(e) => onChange({ repositoryUrl: e.target.value })}
        />
      </Field>
      <Field
        label="Project folder (optional)"
        hint="Its workers' file, program, and git tools work only inside this folder. Without one, they work in Plenipo's own folder inside Documents."
      >
        <input
          value={value.localPath}
          maxLength={1000}
          placeholder="D:\projects\website"
          onChange={(e) => onChange({ localPath: e.target.value })}
        />
      </Field>
      <label className="choice">
        <input
          type="checkbox"
          checked={value.branchPerObjective}
          onChange={(e) => onChange({ branchPerObjective: e.target.checked })}
        />
        <span className="choice__label">
          Work on a separate branch for each objective (recommended)
        </span>
      </label>
      <p className="hint">
        When the folder is a git repository, each objective&apos;s workers work in their own copy of
        it, on a new branch — your own copy of the folder is never changed. Turn this off to let
        workers change the folder itself.
      </p>
      <Field
        label="Permission limit"
        hint="Narrows what every worker may do in this project; each role's own permissions still apply."
      >
        <select
          value={value.capabilityProfile}
          onChange={(e) => onChange({ capabilityProfile: e.target.value })}
        >
          <option value="">No limit</option>
          {(sets ?? []).map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
          {value.capabilityProfile !== "" &&
            !(sets ?? []).some((s) => s.id === value.capabilityProfile) && (
              <option value={value.capabilityProfile}>
                {sets === null
                  ? value.capabilityProfile
                  : `${value.capabilityProfile} (not a permission set: its workers get nothing)`}
              </option>
            )}
        </select>
      </Field>
    </>
  );
}

export function NewProjectDialog({
  snapshot,
  departmentId: initialDepartment = null,
  fromWorkforce = "",
  onCancel,
  onSubmit,
  onTemplate,
}: {
  snapshot: OrgSnapshot;
  departmentId?: string | null;
  /** Its supervisor comes from your Workforce (ADR-045). */
  fromWorkforce?: string;
  onCancel: () => void;
  onSubmit: Submit<ProjectInput>;
  /** Use the Software project template instead (Phase 25, item 2.8). */
  onTemplate?: (() => void) | undefined;
}) {
  const departments = snapshot.departments.filter((d) => d.active && d.headPositionId);
  const [departmentId, setDepartmentId] = useState(
    departments.find((d) => d.id === initialDepartment)?.id ?? departments[0]?.id ?? "",
  );
  const readyRuntimes = snapshot.runtimes.filter((r) => r.ready).map((r) => r.id);
  const [settings, setSettings] = useState<ProjectSettingsState>({
    name: "",
    description: "",
    repositoryUrl: "",
    localPath: "",
    allowedRuntimes: readyRuntimes.length > 0 ? readyRuntimes : snapshot.runtimes.map((r) => r.id),
    capabilityProfile: "",
    branchPerObjective: true,
  });
  const [lead, setLead] = useState(() => newLead(snapshot, "projectCoordinator", fromWorkforce));
  const { pending, error, run } = useSubmit();
  const t = titlesOf(snapshot);
  const supervisor = rankName(t, "projectCoordinator");
  const fallbackTitle = `${settings.name.trim() || "Project"} Supervisor`;
  const shownLead = lead.titleEdited ? lead : { ...lead, title: fallbackTitle };
  const draftProject: ProjectInfo = {
    id: "",
    name: settings.name.trim() || "This project",
    description: "",
    departmentId,
    repositoryUrl: null,
    localPath: null,
    allowedRuntimes: settings.allowedRuntimes,
    capabilityProfile: null,
    coordinatorPositionId: null,
    active: true,
    branchPerObjective: settings.branchPerObjective,
    createdAt: 0,
    archivedAt: null,
    archivedWith: null,
    deleted: false,
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(() =>
      onSubmit({
        ...settingsInput(settings),
        departmentId,
        coordinator: leadInput(shownLead, fallbackTitle),
      }),
    );
  };

  return (
    <Modal title="New project" onClose={onCancel} wide tour="new-project">
      <form className="modal__body" aria-label="New project" onSubmit={submit}>
        {onTemplate && !fromWorkforce && (
          <div className="actions">
            <Button size="sm" onClick={onTemplate}>
              Use the Software project template
            </Button>
            <span className="muted">
              A {rankName(titlesOf(snapshot), "projectCoordinator")} and a Development team
              (developer, reviewer, QA engineer, writer), using your department&apos;s workers
              first.
            </span>
          </div>
        )}
        {departments.length === 0 ? (
          <p className="hint">
            A project belongs to a department, and its {supervisor} reports to the department&apos;s{" "}
            {rankName(t, "departmentManager")}. Create a department first.
          </p>
        ) : (
          <Field label="Department">
            <select value={departmentId} onChange={(e) => setDepartmentId(e.target.value)} required>
              {departments.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
            </select>
          </Field>
        )}
        <ProjectSettingsFields
          snapshot={snapshot}
          value={settings}
          onChange={(patch) => setSettings((s) => ({ ...s, ...patch }))}
        />
        <LeadFields
          snapshot={snapshot}
          kind="projectCoordinator"
          lead={shownLead}
          onChange={(patch) => setLead((l) => ({ ...l, ...patch }))}
          project={draftProject}
          what={`Its ${supervisor}`}
        />
        <FormError error={error} />
        <Footer
          pending={pending}
          label="Create project"
          disabled={
            departments.length === 0 || settings.name.trim() === "" || shownLead.roleId === ""
          }
          onCancel={onCancel}
        />
      </form>
    </Modal>
  );
}

export function EditProjectDialog({
  snapshot,
  project,
  onCancel,
  onSubmit,
}: {
  snapshot: OrgSnapshot;
  project: ProjectInfo;
  onCancel: () => void;
  onSubmit: Submit<ProjectInput>;
}) {
  const [settings, setSettings] = useState<ProjectSettingsState>({
    name: project.name,
    description: project.description,
    repositoryUrl: project.repositoryUrl ?? "",
    localPath: project.localPath ?? "",
    allowedRuntimes: project.allowedRuntimes,
    capabilityProfile: project.capabilityProfile ?? "",
    branchPerObjective: project.branchPerObjective,
  });
  const { pending, error, run } = useSubmit();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(() => onSubmit(settingsInput(settings)));
  };
  return (
    <Modal title={`Edit ${project.name}`} onClose={onCancel} wide>
      <form className="modal__body" aria-label="Edit project" onSubmit={submit}>
        <ProjectSettingsFields
          snapshot={snapshot}
          value={settings}
          onChange={(patch) => setSettings((s) => ({ ...s, ...patch }))}
        />
        <FormError error={error} />
        <Footer
          pending={pending}
          label="Save"
          disabled={settings.name.trim() === ""}
          onCancel={onCancel}
        />
      </form>
    </Modal>
  );
}

/** The Development template's team (Phase 8): the roles a new Development project is staffed
 * with, all on call. */
const DEVELOPMENT_TEAM = [
  "Senior Developer",
  "Code Reviewer",
  "QA Engineer",
  "Documentation Writer",
];

/** Set up a software project from the Development template (Phase 8). */
export function SetUpDevelopmentDialog({
  snapshot,
  onCancel,
  onSubmit,
}: {
  snapshot: OrgSnapshot;
  onCancel: () => void;
  onSubmit: Submit<DevelopmentInput>;
}) {
  const readyRuntimes = snapshot.runtimes.filter((r) => r.ready).map((r) => r.id);
  const [settings, setSettings] = useState<ProjectSettingsState>({
    name: "",
    description: "",
    repositoryUrl: "",
    localPath: "",
    allowedRuntimes: readyRuntimes.length > 0 ? readyRuntimes : snapshot.runtimes.map((r) => r.id),
    capabilityProfile: "",
    branchPerObjective: true,
  });
  const [runtimeId, setRuntimeId] = useState("");
  const { pending, error, run } = useSubmit();
  const t = titlesOf(snapshot);
  const development = snapshot.departments.find(
    (d) => d.active && d.name.toLowerCase() === "development",
  );
  // The department it joins (Phase 25, item 2.7): the Development department, or one you pick.
  const [departmentId, setDepartmentId] = useState(development?.id ?? "");
  const hasDepartment = departmentId !== "";
  const department = snapshot.departments.find((d) => d.id === departmentId);
  // Each job uses a worker the department already has, unless you ask for a new one.
  const reuse = reusableWorkers(snapshot, departmentId || null, DEVELOPMENT_TEAM);
  const [hireNew, setHireNew] = useState<string[]>([]);
  const refused = runtimeId !== "" && !settings.allowedRuntimes.includes(runtimeId);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(() =>
      onSubmit({
        project: settingsInput(settings),
        ...(runtimeId ? { runtimeId } : {}),
        ...(departmentId && departmentId !== development?.id ? { departmentId } : {}),
        hireNew,
      }),
    );
  };
  return (
    <Modal title="Set up a Development project" onClose={onCancel} wide tour="software-project">
      <form className="modal__body" aria-label="Set up a Development project" onSubmit={submit}>
        <p className="muted">
          {hasDepartment
            ? `The project joins the ${department?.name ?? "Development"} department`
            : `Plenipo creates the Development department with its ${rankName(t, "superintendent")}`}
          , then the project with its {rankName(t, "projectCoordinator")} and a team on call:{" "}
          {DEVELOPMENT_TEAM.join(", ")}. Give objectives on the Projects page; each one gets its own
          branch.
        </p>
        <Field
          label="Department"
          hint="The project's team uses the department's workers first, and hires only what's missing."
        >
          <select value={departmentId} onChange={(e) => setDepartmentId(e.target.value)}>
            {!development && <option value="">A new Development department</option>}
            {snapshot.departments
              .filter((d) => d.active)
              .map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
          </select>
        </Field>
        <fieldset className="choices">
          <legend>The team</legend>
          {DEVELOPMENT_TEAM.map((job, i) => {
            const p = reuse[i];
            if (!p) {
              return (
                <p key={job} className="muted">
                  {job}: hire new (the department has none)
                </p>
              );
            }
            const model = p.route?.choice?.label ?? runtimeChoiceLabel(snapshot, p.runtimeId ?? "");
            const fresh = hireNew.includes(job);
            return (
              <Field key={job} label={job}>
                <select
                  value={fresh ? "new" : "use"}
                  onChange={(e) =>
                    setHireNew((h) =>
                      e.target.value === "new" ? [...h, job] : h.filter((x) => x !== job),
                    )
                  }
                >
                  <option value="use">
                    Use {p.title} ({p.roleName}
                    {model ? `, ${model}` : ""})
                  </option>
                  <option value="new">Hire new</option>
                </select>
              </Field>
            );
          })}
        </fieldset>
        <ProjectSettingsFields
          snapshot={snapshot}
          value={settings}
          onChange={(patch) => setSettings((s) => ({ ...s, ...patch }))}
        />
        <Field
          label={`AI tool of the ${hasDepartment ? "" : `${rankName(t, "superintendent")} and the `}${rankName(t, "projectCoordinator")}`}
          hint={
            refused ? (
              <span className="field__warn">Pick one of the project&apos;s allowed AI tools.</span>
            ) : (
              "The team is always automatic: each role's model choices pick its AI tool and model."
            )
          }
        >
          <select value={runtimeId} onChange={(e) => setRuntimeId(e.target.value)}>
            <option value="">Automatic (the role&apos;s model choices)</option>
            <RuntimeOptions snapshot={snapshot} current={runtimeId} />
          </select>
        </Field>
        <FormError error={error} />
        <Footer
          pending={pending}
          label="Set up"
          disabled={settings.name.trim() === "" || refused}
          onCancel={onCancel}
        />
      </form>
    </Modal>
  );
}

// ---- Roles ----------------------------------------------------------------------------------

/**
 * A new role, or a change to one you created (built-in roles keep their instructions). Its
 * working instructions — its job, what it hands back, its limits, and when it asks for help —
 * go into every one of its workers' instructions, in your words (ADR-019).
 */
export function RoleDialog({
  titles: t,
  role,
  onCancel,
  onSubmit,
  onUpdate,
}: {
  titles: TitleSet;
  /** The role to change; a new role when absent. */
  role?: RoleInfo;
  onCancel: () => void;
  onSubmit: Submit<RoleInput>;
  onUpdate?: Submit<RoleUpdate>;
}) {
  const [name, setName] = useState(role?.name ?? "");
  const [description, setDescription] = useState(role?.description ?? "");
  const [kind, setKind] = useState<PositionKind>(role?.kind ?? "worker");
  const [staffing, setStaffing] = useState<Staffing>(role?.staffing ?? "onDemand");
  const [job, setJob] = useState<Record<keyof RoleJob, string>>(() => {
    const j = role?.job ?? EMPTY_JOB;
    return {
      duties: j.duties.join("\n"),
      returns: j.returns.join("\n"),
      limits: j.limits.join("\n"),
      askLead: j.askLead.join("\n"),
    };
  });
  const { pending, error, run } = useSubmit();
  const fixed = kind !== "worker";
  const editing = role !== undefined;
  const title = editing ? `Edit role: ${role.name}` : "New role";
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const written: RoleJob = {
      duties: jobLines(job.duties),
      returns: jobLines(job.returns),
      limits: jobLines(job.limits),
      askLead: jobLines(job.askLead),
    };
    void run(() =>
      editing && onUpdate
        ? onUpdate({ name: name.trim(), description: description.trim(), job: written })
        : onSubmit({
            name: name.trim(),
            description: description.trim(),
            kind,
            staffing: fixed ? "persistent" : staffing,
            job: written,
          }),
    );
  };
  return (
    <Modal title={title} onClose={onCancel}>
      <form className="modal__body" aria-label={title} onSubmit={submit}>
        <Field label="Name">
          <input value={name} maxLength={80} required onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field
          label="What this role does"
          hint="A sentence or two in plain words. Becomes part of its workers' instructions."
        >
          <textarea
            value={description}
            rows={3}
            maxLength={MAX_OBJECTIVE_FIELD}
            onChange={(e) => setDescription(e.target.value)}
          />
        </Field>
        <fieldset className="choices">
          <legend>Working instructions (optional)</legend>
          <p className="muted">
            Its workers follow these, with the same rules as the built-in roles. Left empty, they
            work from “What this role does” and their lead&apos;s instructions.
          </p>
          {JOB_FIELDS.map((f) => (
            <Field key={f.key} label={f.label} hint={f.hint}>
              <textarea
                value={job[f.key]}
                rows={3}
                maxLength={MAX_OBJECTIVE_FIELD}
                onChange={(e) => setJob((j) => ({ ...j, [f.key]: e.target.value }))}
              />
            </Field>
          ))}
        </fieldset>
        {editing ? (
          <p className="muted">
            {rankName(t, role.kind)} · {STAFFING_LABEL[role.staffing]}. Rank and staffing stay as
            they are; positions holding this role keep it, and their next workers get the new
            instructions.
          </p>
        ) : (
          <>
            <Field label="Rank">
              <select value={kind} onChange={(e) => setKind(e.target.value as PositionKind)}>
                {RANKS.filter((k): k is PositionKind => k !== "owner").map((k) => (
                  <option key={k} value={k}>
                    {rankName(t, k)}
                  </option>
                ))}
              </select>
            </Field>
            <fieldset className="choices">
              <legend>Staffing</legend>
              <label className="choice">
                <input
                  type="radio"
                  name="staffing"
                  checked={fixed || staffing === "persistent"}
                  onChange={() => setStaffing("persistent")}
                />
                <span className="choice__label">{STAFFING_LABEL.persistent}</span>
                <span className="muted">
                  one agent that keeps its conversation and can lead a team
                </span>
              </label>
              <label className="choice">
                <input
                  type="radio"
                  name="staffing"
                  disabled={fixed}
                  checked={!fixed && staffing === "onDemand"}
                  onChange={() => setStaffing("onDemand")}
                />
                <span className="choice__label">{STAFFING_LABEL.onDemand}</span>
                <span className="muted">a new worker for each task, gone when it is done</span>
              </label>
            </fieldset>
          </>
        )}
        <FormError error={error} />
        <Footer
          pending={pending}
          label={editing ? "Save role" : "Create role"}
          disabled={name.trim() === ""}
          onCancel={onCancel}
        />
      </form>
    </Modal>
  );
}
