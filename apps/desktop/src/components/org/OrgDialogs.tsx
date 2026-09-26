import { useState, type FormEvent, type ReactNode } from "react";
import type {
  DepartmentInfo,
  DepartmentInput,
  HireInput,
  LeadInput,
  OrgSnapshot,
  PositionInfo,
  PositionKind,
  ProjectInfo,
  ProjectInput,
  RoleInfo,
  RoleInput,
  Staffing,
} from "@plenipo/types";

import { STAFFING_LABEL } from "../../org/format";
import {
  defaultRuntime,
  hireRefusal,
  hireableRoles,
  positionMap,
  supervisorChoices,
} from "../../org/rules";
import { RANKS, rankName, roleLabel, titlesOf, withArticle, type TitleSet } from "../../org/titles";
import { Modal } from "./Modal";

/** Resolves with the refusal to show, or `null` once done. */
type Submit<T> = (input: T) => Promise<string | null>;

const OWNER_VALUE = "__owner__";
const MAX_OBJECTIVE_FIELD = 4000;

function runtimeChoiceLabel(snapshot: OrgSnapshot, id: string): string {
  const r = snapshot.runtimes.find((x) => x.id === id);
  if (!r) return id;
  return r.ready ? r.label : `${r.label} (not ready)`;
}

function projectOf(snapshot: OrgSnapshot, positionId: string | null): ProjectInfo | null {
  if (!positionId) return null;
  const projectId = positionMap(snapshot).get(positionId)?.projectId ?? null;
  return snapshot.projects.find((p) => p.id === projectId) ?? null;
}

/** "Website Supervisor", or "Engineering Lead — Manager" when the title does not say its rank
 * (a worker's role, for workers). */
function positionChoiceLabel(t: TitleSet, p: PositionInfo): string {
  const label = p.kind === "worker" ? p.roleName : rankName(t, p.kind);
  return p.title === label || p.title.endsWith(` ${label}`) ? p.title : `${p.title} — ${label}`;
}

/** "Full-time: one agent holds it and keeps its conversation." */
const STAFFING_HINT: Record<Staffing, string> = {
  persistent: "Full-time: one agent holds it and keeps its conversation.",
  onDemand: "On call: a new worker is brought in for each task and leaves when it is done.",
};

function capitalized(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function useSubmit() {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (work: () => Promise<string | null>) => {
    setPending(true);
    setError(null);
    const failure = await work();
    setPending(false);
    setError(failure);
  };
  return { pending, error, run };
}

function FormError({ error }: { error: string | null }) {
  return error ? (
    <p className="form-error" role="alert">
      {error}
    </p>
  ) : null;
}

function Footer({
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
      <button type="button" className="button button--quiet" onClick={onCancel}>
        Cancel
      </button>
      <button type="submit" className="button" disabled={pending || disabled}>
        {pending ? "Working…" : label}
      </button>
    </footer>
  );
}

function Field({
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
  const refused = project !== null && !project.allowedRuntimes.includes(value);
  return (
    <Field
      label="AI tool"
      hint={
        refused ? (
          <span className="field__warn">
            {project.name} does not allow this AI tool
            {project.allowedRuntimes.length > 0
              ? ` (allowed: ${project.allowedRuntimes.map((r) => runtimeChoiceLabel(snapshot, r)).join(", ")})`
              : "; it allows none yet"}
            .
          </span>
        ) : undefined
      }
    >
      <select value={value} onChange={(e) => onChange(e.target.value)} required>
        {snapshot.runtimes.map((r) => (
          <option key={r.id} value={r.id}>
            {runtimeChoiceLabel(snapshot, r.id)}
          </option>
        ))}
      </select>
    </Field>
  );
}

function withModel<T extends object>(input: T, model: string): T & { model?: string } {
  const m = model.trim();
  return m ? { ...input, model: m } : input;
}

// ---- Hire -----------------------------------------------------------------------------------

export function HireDialog({
  snapshot,
  roleId: initialRole,
  reportsTo: initialSupervisor,
  onCancel,
  onSubmit,
}: {
  snapshot: OrgSnapshot;
  roleId: string | null;
  /** `undefined`: choose a sensible supervisor; `null`: the owner. */
  reportsTo?: string | null;
  onCancel: () => void;
  onSubmit: Submit<HireInput>;
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
  const [runtimeId, setRuntimeId] = useState(() =>
    defaultRuntime(snapshot, project?.allowedRuntimes ?? null),
  );
  const [model, setModel] = useState("");
  const [vacant, setVacant] = useState(false);
  const { pending, error, run } = useSubmit();

  const choices = role ? supervisorChoices(snapshot, role) : [];
  const wantedRefusal =
    role && initialSupervisor !== undefined && initialSupervisor !== reportsTo
      ? hireRefusal(snapshot, role, initialSupervisor)
      : null;

  const changeRole = (id: string) => {
    const next = roles.find((r) => r.id === id) ?? null;
    setRoleId(id);
    if (!titleEdited) setTitle(next?.name ?? "");
    const supervisor = pickSupervisor(next, reportsTo);
    changeSupervisor(supervisor);
    if (next?.staffing !== "persistent") setVacant(false);
  };
  const changeSupervisor = (id: string | null) => {
    setReportsTo(id);
    const allowed = projectOf(snapshot, id)?.allowedRuntimes ?? null;
    if (allowed && !allowed.includes(runtimeId)) setRuntimeId(defaultRuntime(snapshot, allowed));
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!role) return;
    const input: HireInput = withModel(
      { roleId: role.id, title: title.trim(), reportsTo, runtimeId },
      model,
    );
    void run(() => onSubmit(vacant ? { ...input, vacant: true } : input));
  };

  return (
    <Modal title="Hire" onClose={onCancel}>
      <form className="modal__body" aria-label="Hire" onSubmit={submit}>
        <Field
          label="Role"
          hint={role ? `${role.description} ${STAFFING_HINT[role.staffing]}` : undefined}
        >
          <select value={roleId} onChange={(e) => changeRole(e.target.value)} required>
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
        <RuntimeField
          snapshot={snapshot}
          value={runtimeId}
          onChange={setRuntimeId}
          project={project}
        />
        <Field label="Model (optional)" hint="Leave blank for the AI tool's default model.">
          <input value={model} maxLength={100} onChange={(e) => setModel(e.target.value)} />
        </Field>
        {role?.staffing === "persistent" && (
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
  return (
    <fieldset className="fieldset">
      <legend>{what}</legend>
      <Field label="Role">
        <select value={lead.roleId} onChange={(e) => onChange({ roleId: e.target.value })} required>
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
      <RuntimeField
        snapshot={snapshot}
        value={lead.runtimeId}
        onChange={(runtimeId) => onChange({ runtimeId })}
        project={project}
      />
      <Field label="Model (optional)">
        <input
          value={lead.model}
          maxLength={100}
          onChange={(e) => onChange({ model: e.target.value })}
        />
      </Field>
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
}

function newLead(snapshot: OrgSnapshot, kind: PositionKind, allowed: string[] | null): LeadState {
  const roles = leadRoles(snapshot, kind);
  const role = roles.find((r) => r.template) ?? roles[0];
  return {
    roleId: role?.id ?? "",
    title: "",
    titleEdited: false,
    runtimeId: defaultRuntime(snapshot, allowed),
    model: "",
    vacant: false,
  };
}

function leadInput(lead: LeadState, fallbackTitle: string): LeadInput {
  const input: LeadInput = withModel(
    {
      roleId: lead.roleId,
      title: (lead.titleEdited ? lead.title : lead.title || fallbackTitle).trim(),
      runtimeId: lead.runtimeId,
    },
    lead.model,
  );
  return lead.vacant ? { ...input, vacant: true } : input;
}

export function NewDepartmentDialog({
  snapshot,
  reportsTo: initialSupervisor = null,
  onCancel,
  onSubmit,
}: {
  snapshot: OrgSnapshot;
  reportsTo?: string | null;
  onCancel: () => void;
  onSubmit: Submit<DepartmentInput>;
}) {
  const superintendents = snapshot.positions.filter((p) => p.active && p.kind === "superintendent");
  const t = titlesOf(snapshot);
  const manager = rankName(t, "departmentManager");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [reportsTo, setReportsTo] = useState<string | null>(
    superintendents.some((p) => p.id === initialSupervisor) ? initialSupervisor : null,
  );
  const [lead, setLead] = useState(() => newLead(snapshot, "departmentManager", null));
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
    <Modal title="New department" onClose={onCancel} wide>
      <form className="modal__body" aria-label="New department" onSubmit={submit}>
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
}

function settingsInput(s: ProjectSettingsState): ProjectInput {
  const input: ProjectInput = {
    name: s.name.trim(),
    description: s.description.trim(),
    allowedRuntimes: s.allowedRuntimes,
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
            <span className="choice__label">{r.label}</span>
            {!r.ready && <span className="pill pill--warn">Not ready</span>}
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
        label="Local folder (optional)"
        hint="Recorded only: workers get no folder access until Guard arrives (Phase 7)."
      >
        <input
          value={value.localPath}
          maxLength={1000}
          onChange={(e) => onChange({ localPath: e.target.value })}
        />
      </Field>
      <Field
        label="Capability profile (optional)"
        hint="Recorded only: Guard grants capabilities from Phase 7."
      >
        <input
          value={value.capabilityProfile}
          maxLength={64}
          onChange={(e) => onChange({ capabilityProfile: e.target.value })}
        />
      </Field>
    </>
  );
}

export function NewProjectDialog({
  snapshot,
  departmentId: initialDepartment = null,
  onCancel,
  onSubmit,
}: {
  snapshot: OrgSnapshot;
  departmentId?: string | null;
  onCancel: () => void;
  onSubmit: Submit<ProjectInput>;
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
  });
  const [lead, setLead] = useState(() =>
    newLead(snapshot, "projectCoordinator", settings.allowedRuntimes),
  );
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
    createdAt: 0,
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
    <Modal title="New project" onClose={onCancel} wide>
      <form className="modal__body" aria-label="New project" onSubmit={submit}>
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

// ---- Roles ----------------------------------------------------------------------------------

export function RoleDialog({
  titles: t,
  onCancel,
  onSubmit,
}: {
  titles: TitleSet;
  onCancel: () => void;
  onSubmit: Submit<RoleInput>;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [kind, setKind] = useState<PositionKind>("worker");
  const [staffing, setStaffing] = useState<Staffing>("onDemand");
  const { pending, error, run } = useSubmit();
  const fixed = kind !== "worker";
  const submit = (e: FormEvent) => {
    e.preventDefault();
    void run(() =>
      onSubmit({
        name: name.trim(),
        description: description.trim(),
        kind,
        staffing: fixed ? "persistent" : staffing,
      }),
    );
  };
  return (
    <Modal title="New role" onClose={onCancel}>
      <form className="modal__body" aria-label="New role" onSubmit={submit}>
        <Field label="Name">
          <input value={name} maxLength={80} required onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="What it does" hint="Becomes part of its workers' instructions.">
          <textarea
            value={description}
            rows={3}
            maxLength={MAX_OBJECTIVE_FIELD}
            onChange={(e) => setDescription(e.target.value)}
          />
        </Field>
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
            <span className="muted">one agent that keeps its conversation and can lead a team</span>
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
        <FormError error={error} />
        <Footer
          pending={pending}
          label="Create role"
          disabled={name.trim() === ""}
          onCancel={onCancel}
        />
      </form>
    </Modal>
  );
}
