/**
 * The organization: a live topology map (like a network topology view) of departments,
 * managers, project coordinators, their teams, and the workers they spawn. Hire by dragging a
 * role onto a position, reorganize by dragging a position onto another, and assign reviewers,
 * QA evaluators, and security auditors the same way. Everything comes from the Workforce engine;
 * nothing is hard-coded here.
 */
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import type { OrgSnapshot, OversightRole, PositionInfo } from "@plenipo/types";

import {
  archivePosition,
  archiveProject,
  assignOversight,
  createDepartment,
  createProject,
  createRole,
  endOversight,
  fillPosition,
  giveObjective,
  hirePosition,
  movePosition,
  removeDepartment,
  renameOrganization,
  toCommandError,
  updateDepartment,
  updatePosition,
  updateProject,
  vacatePosition,
} from "../api/commands";
import { Directory } from "../components/org/Directory";
import { DropMenu, type DropChoice } from "../components/org/DropMenu";
import { Glyph } from "../components/org/Glyph";
import { HirePalette } from "../components/org/HirePalette";
import { Inspector, type InspectorActions } from "../components/org/Inspector";
import { ConfirmDialog, Modal } from "../components/org/Modal";
import {
  EditDepartmentDialog,
  EditProjectDialog,
  HireDialog,
  NewDepartmentDialog,
  NewProjectDialog,
  RoleDialog,
} from "../components/org/OrgDialogs";
import {
  TopologyCanvas,
  type CanvasHandle,
  type DragPayload,
} from "../components/org/TopologyCanvas";
import { OVERSIGHT_LABEL, plural } from "../org/format";
import { ORG_ID, OWNER_ID, ancestorsOf, layoutOrganization } from "../org/layout";
import { nodeContext } from "../org/nodes";
import {
  hireRefusal,
  moveRefusal,
  oversightOrder,
  oversightRefusal,
  positionMap,
} from "../org/rules";
import { searchMatches } from "../org/search";
import { useOrganization } from "../org/useOrganization";

const SELECTED_KEY = "plenipo.orgSelected";
const MODE_KEY = "plenipo.orgMode";
const COLLAPSED_KEY = "plenipo.orgCollapsed";
const OVERSIGHT_KEY = "plenipo.orgOversight";

type Mode = "topology" | "list";

type Dialog =
  | { kind: "hire"; roleId: string | null; reportsTo?: string | null }
  | { kind: "newDepartment"; reportsTo: string | null }
  | { kind: "editDepartment"; id: string }
  | { kind: "newProject"; departmentId: string | null }
  | { kind: "editProject"; id: string }
  | { kind: "role" }
  | { kind: "rename" }
  | {
      kind: "confirm";
      title: string;
      message: ReactNode;
      confirmLabel: string;
      work: () => Promise<OrgSnapshot>;
    };

interface Drop {
  title: string;
  x: number;
  y: number;
  choices: DropChoice[];
}

interface Toast {
  id: number;
  text: string;
  tone: "ok" | "error";
}

function read(key: string): string | null {
  try {
    return sessionStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string | null) {
  try {
    if (value === null) sessionStorage.removeItem(key);
    else sessionStorage.setItem(key, value);
  } catch {
    // Storage unavailable: the view simply isn't restored after a reload.
  }
}

/** `collapsed` without the nodes that hide `id`; `null` when nothing hides it. */
function expanded(snapshot: OrgSnapshot, collapsed: Set<string>, id: string): Set<string> | null {
  const hidden = ancestorsOf(snapshot, id).filter((a) => collapsed.has(a));
  if (hidden.length === 0) return null;
  const next = new Set(collapsed);
  hidden.forEach((a) => next.delete(a));
  return next;
}

/** A drop target as a supervisor: You and the organization both mean the owner. */
function supervisorOf(byId: Map<string, PositionInfo>, target: string): string | null | undefined {
  if (target === OWNER_ID || target === ORG_ID) return null;
  return byId.has(target) ? target : undefined;
}

function readCollapsed(): Set<string> {
  try {
    const list = JSON.parse(read(COLLAPSED_KEY) ?? "[]") as unknown;
    return new Set(
      Array.isArray(list) ? list.filter((x): x is string => typeof x === "string") : [],
    );
  } catch {
    return new Set();
  }
}

export function OrganizationView({
  onOpenSession,
  onOpenTask,
  focusId = null,
  onFocusHandled,
}: {
  onOpenSession: (sessionId: string) => void;
  onOpenTask: (taskId: string) => void;
  /** A position to show on arrival (from another view). */
  focusId?: string | null;
  onFocusHandled?: () => void;
}) {
  const org = useOrganization();
  const { snapshot, apply, reload } = org;
  const canvas = useRef<CanvasHandle>(null);
  const [mode, setModeState] = useState<Mode>(() =>
    read(MODE_KEY) === "list" ? "list" : "topology",
  );
  const [selectedId, setSelectedState] = useState<string | null>(() => read(SELECTED_KEY));
  const [collapsed, setCollapsed] = useState<Set<string>>(readCollapsed);
  const [showOversight, setShowOversight] = useState(() => read(OVERSIGHT_KEY) !== "off");
  const [query, setQuery] = useState("");
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [drop, setDrop] = useState<Drop | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastId = useRef(0);

  const setMode = (next: Mode) => {
    setModeState(next);
    write(MODE_KEY, next);
  };
  const setSelected = setSelectedState;
  const updateCollapsed = setCollapsed;
  // Remember the view for this session (restored after navigating away and back).
  useEffect(() => write(SELECTED_KEY, selectedId), [selectedId]);
  useEffect(() => write(COLLAPSED_KEY, JSON.stringify([...collapsed])), [collapsed]);

  const toast = useCallback((text: string, tone: Toast["tone"] = "ok") => {
    const id = ++toastId.current;
    setToasts((list) => [...list.slice(-2), { id, text, tone }]);
    setTimeout(
      () => setToasts((list) => list.filter((t) => t.id !== id)),
      tone === "error" ? 9000 : 4000,
    );
  }, []);

  const layout = useMemo(
    () => (snapshot ? layoutOrganization(snapshot, collapsed) : null),
    [snapshot, collapsed],
  );
  const ctx = useMemo(() => (snapshot ? nodeContext(snapshot) : null), [snapshot]);
  const matchList = useMemo(
    () => (snapshot ? searchMatches(snapshot, query) : null),
    [snapshot, query],
  );
  const matches = useMemo(() => (matchList ? new Set(matchList) : null), [matchList]);

  /** Select a node and make sure it is shown (the canvas brings it into view). */
  const reveal = useCallback(
    (id: string) => {
      if (!snapshot) return;
      const next = expanded(snapshot, collapsed, id);
      if (next) updateCollapsed(next);
      setSelected(id);
    },
    [snapshot, collapsed, updateCollapsed, setSelected],
  );

  // Arriving from another view with a position to show.
  const [arrived, setArrived] = useState<string | null>(null);
  if (focusId && snapshot && arrived !== focusId) {
    setArrived(focusId);
    if (snapshot.positions.some((p) => p.id === focusId)) {
      const next = expanded(snapshot, collapsed, focusId);
      if (next) setCollapsed(next);
      setSelectedState(focusId);
    }
  }
  useEffect(() => {
    if (focusId && arrived === focusId) onFocusHandled?.();
  }, [focusId, arrived, onFocusHandled]);

  /** Run a change; the snapshot it returns is applied. Resolves with the refusal, if any. */
  const run = useCallback(
    async (work: () => Promise<OrgSnapshot>): Promise<string | null> => {
      try {
        apply(await work());
        return null;
      } catch (reason) {
        return toCommandError(reason).message;
      }
    },
    [apply],
  );

  /** Run a change started from the canvas; report the outcome as a toast. */
  const runWithToast = useCallback(
    async (work: () => Promise<OrgSnapshot>, done: string) => {
      const failure = await run(work);
      toast(failure ?? done, failure ? "error" : "ok");
    },
    [run, toast],
  );

  const closeDialog = useCallback(() => setDialog(null), []);
  /** Submit a dialog: close it once the change is applied, keep it open with the refusal. */
  const submit = useCallback(
    async (work: () => Promise<OrgSnapshot>, done?: string) => {
      const failure = await run(work);
      if (failure === null) {
        setDialog(null);
        if (done) toast(done);
      }
      return failure;
    },
    [run, toast],
  );

  const actions: InspectorActions = useMemo(
    () => ({
      run,
      giveObjective: async (positionId, objective) => {
        try {
          await giveObjective(positionId, objective);
          void reload();
          return null;
        } catch (reason) {
          return toCommandError(reason).message;
        }
      },
      hire: (reportsTo) => setDialog({ kind: "hire", roleId: null, reportsTo }),
      newDepartment: (reportsTo) => setDialog({ kind: "newDepartment", reportsTo }),
      newProject: (departmentId) => setDialog({ kind: "newProject", departmentId }),
      newRole: () => setDialog({ kind: "role" }),
      editDepartment: (id) => setDialog({ kind: "editDepartment", id }),
      editProject: (id) => setDialog({ kind: "editProject", id }),
      rename: () => setDialog({ kind: "rename" }),
      confirm: (request) => setDialog({ kind: "confirm", ...request }),
      openSession: onOpenSession,
      openTask: onOpenTask,
      api: {
        fill: fillPosition,
        vacate: vacatePosition,
        update: updatePosition,
        move: movePosition,
        archive: archivePosition,
        assign: assignOversight,
        endOversight,
        removeDepartment,
        archiveProject,
      },
    }),
    [run, reload, onOpenSession, onOpenTask],
  );

  // ---- Drag and drop -------------------------------------------------------------------------

  const byId = useMemo(
    () => (snapshot ? positionMap(snapshot) : new Map<string, PositionInfo>()),
    [snapshot],
  );

  const dropRefusal = useCallback(
    (payload: DragPayload, target: string): string | null => {
      if (!snapshot) return "Loading…";
      const to = supervisorOf(byId, target);
      if (to === undefined) return "Workers come and go with their tasks; drop on a position.";
      if (payload.kind === "role") {
        const role = snapshot.roles.find((r) => r.id === payload.roleId);
        return role ? hireRefusal(snapshot, role, to) : "That role no longer exists.";
      }
      const moving = byId.get(payload.positionId);
      if (!moving) return "That position no longer exists.";
      if (to === moving.id) return "Drop it on another position.";
      const move = moveRefusal(snapshot, moving, to);
      if (move === null) return null;
      const target_ = to ? byId.get(to) : undefined;
      const oversee =
        target_ &&
        (["review", "qa", "security"] as OversightRole[]).some(
          (r) => oversightRefusal(snapshot, moving, target_, r) === null,
        );
      return oversee ? null : move;
    },
    [snapshot, byId],
  );

  const onDrop = useCallback(
    (payload: DragPayload, target: string, at: { x: number; y: number }) => {
      if (!snapshot) return;
      const to = supervisorOf(byId, target);
      if (to === undefined) return;
      if (payload.kind === "role") {
        setDialog({ kind: "hire", roleId: payload.roleId, reportsTo: to });
        return;
      }
      const moving = byId.get(payload.positionId);
      if (!moving) return;
      const lead = to ? byId.get(to) : undefined;
      const leadName = lead ? lead.title : "you";
      const choices: DropChoice[] = [];
      if (moveRefusal(snapshot, moving, to) === null) {
        choices.push({
          id: "move",
          label: `Report to ${lead ? lead.title : "you (owner)"}`,
          detail: moving.staffing === "persistent" ? "Its team moves with it" : "Joins this team",
          run: () =>
            void runWithToast(
              () => movePosition(moving.id, to),
              `${moving.title} now reports to ${leadName}.`,
            ),
        });
      }
      if (lead) {
        const glyph = snapshot.roles.find((r) => r.id === moving.roleId)?.glyph ?? "";
        for (const role of oversightOrder(glyph)) {
          if (oversightRefusal(snapshot, moving, lead, role) !== null) continue;
          choices.push({
            id: role,
            label: `${OVERSIGHT_LABEL[role]} for ${lead.title}'s team`,
            detail: "Stays where it is; joins that team's hand-off list",
            run: () =>
              void runWithToast(
                () => assignOversight(moving.id, lead.id, role),
                `${moving.title} is now ${lead.title}'s ${OVERSIGHT_LABEL[role].toLowerCase()}.`,
              ),
          });
        }
      }
      if (choices.length > 0)
        setDrop({
          title: `${moving.title} → ${lead ? lead.title : "You"}`,
          x: at.x,
          y: at.y,
          choices,
        });
    },
    [snapshot, byId, runWithToast],
  );

  const describeDrag = useCallback(
    (payload: DragPayload) => {
      if (payload.kind === "role") {
        const role = snapshot?.roles.find((r) => r.id === payload.roleId);
        return {
          title: `Hire ${role?.name ?? "a role"}`,
          glyph: role?.glyph ?? "worker",
          hint: "Release to hire into this team",
        };
      }
      const p = byId.get(payload.positionId);
      return {
        title: p?.title ?? "Position",
        glyph: snapshot?.roles.find((r) => r.id === p?.roleId)?.glyph ?? "worker",
        hint: "Release to choose: report here, or oversee this team",
      };
    },
    [snapshot, byId],
  );

  const toggle = useCallback(
    (id: string) => {
      const next = new Set(collapsed);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      updateCollapsed(next);
    },
    [collapsed, updateCollapsed],
  );

  const toggleOversight = () => {
    setShowOversight((on) => {
      write(OVERSIGHT_KEY, on ? "off" : "on");
      return !on;
    });
  };

  const findFirst = (e: FormEvent) => {
    e.preventDefault();
    const first = matchList?.[0];
    if (first) reveal(first);
  };

  // ---- Render --------------------------------------------------------------------------------

  if (!snapshot || !layout || !ctx) {
    return (
      <section className="view" aria-labelledby="org-title">
        <h1 id="org-title">Organization</h1>
        {org.status === "error" ? (
          <div className="empty">
            <h2>The organization could not be loaded</h2>
            <p className="status status--error">{org.error}</p>
            <button type="button" className="button" onClick={() => void org.reload()}>
              Try again
            </button>
          </div>
        ) : (
          <p className="muted">Loading the organization…</p>
        )}
      </section>
    );
  }

  const s = snapshot.stats;
  const selectedExists =
    selectedId !== null &&
    (selectedId === OWNER_ID ||
      selectedId === ORG_ID ||
      snapshot.positions.some((p) => p.id === selectedId) ||
      selectedId.startsWith("worker:"));
  const empty = snapshot.positions.every((p) => !p.active);
  const editingDepartment =
    dialog?.kind === "editDepartment"
      ? snapshot.departments.find((d) => d.id === dialog.id)
      : undefined;
  const editingProject =
    dialog?.kind === "editProject" ? snapshot.projects.find((p) => p.id === dialog.id) : undefined;

  return (
    <section className="org" aria-labelledby="org-title">
      <header className="org__bar">
        <div className="org__name">
          <h1 id="org-title">{snapshot.name}</h1>
          <button type="button" className="link" onClick={() => setDialog({ kind: "rename" })}>
            Rename
          </button>
        </div>
        <dl className="org__kpis" aria-label="At a glance">
          <Kpi label="Departments" value={s.departments} />
          <Kpi label="Projects" value={s.projects} />
          <Kpi
            label="Positions"
            value={s.positions}
            detail={s.vacant > 0 ? `${s.vacant} vacant` : undefined}
          />
          <Kpi label="Live workers" value={s.activeWorkers} />
          <Kpi
            label="Work"
            value={`${s.working} working`}
            detail={`${s.waiting} waiting · ${s.queued} queued`}
            tone={s.working > 0 ? "working" : undefined}
          />
          <Kpi
            label="Last 24 h"
            value={`${s.completed24h} done`}
            detail={s.failed24h > 0 ? `${s.failed24h} failed` : undefined}
            tone={s.failed24h > 0 ? "bad" : undefined}
          />
        </dl>
        <div className="org__tools">
          <form className="org__search" role="search" onSubmit={findFirst}>
            <Glyph name="search" size={16} />
            <input
              type="search"
              value={query}
              aria-label="Find in the organization"
              placeholder="Find in organization…"
              onChange={(e) => setQuery(e.target.value)}
            />
            {matchList && (
              <span className="org__matches" role="status">
                {plural(matchList.length, "match", "matches")}
              </span>
            )}
          </form>
          <div className="segmented" role="group" aria-label="Layout">
            <button
              type="button"
              aria-pressed={mode === "topology"}
              onClick={() => setMode("topology")}
            >
              Topology
            </button>
            <button type="button" aria-pressed={mode === "list"} onClick={() => setMode("list")}>
              List
            </button>
          </div>
        </div>
      </header>

      {snapshot.notices.length > 0 && (
        <div className="org__notices" role="status">
          {snapshot.notices.map((n) => (
            <span key={n}>{n}</span>
          ))}
        </div>
      )}

      <div className={`org__body org__body--${mode}${selectedExists ? " has-inspector" : ""}`}>
        {mode === "topology" && (
          <HirePalette
            snapshot={snapshot}
            onStartDrag={(payload, event) => canvas.current?.startDrag(payload, event)}
            onPick={(roleId) =>
              setDialog({
                kind: "hire",
                roleId,
                ...(selectedId &&
                selectedId !== ORG_ID &&
                selectedId !== OWNER_ID &&
                byId.has(selectedId)
                  ? { reportsTo: selectedId }
                  : {}),
              })
            }
            onNewDepartment={() => setDialog({ kind: "newDepartment", reportsTo: null })}
            onNewProject={() => setDialog({ kind: "newProject", departmentId: null })}
            onNewRole={() => setDialog({ kind: "role" })}
          />
        )}
        {mode === "topology" ? (
          <TopologyCanvas
            ref={canvas}
            layout={layout}
            ctx={ctx}
            selectedId={selectedId}
            matches={matches}
            showOversight={showOversight}
            onToggleOversight={toggleOversight}
            onSelect={setSelected}
            onToggle={toggle}
            dropRefusal={dropRefusal}
            onDrop={onDrop}
            describeDrag={describeDrag}
          >
            {empty && (
              <div className="topology__empty" data-canvas-ui>
                <h2>Build your organization</h2>
                <ol>
                  <li>Create a department — it comes with its manager.</li>
                  <li>Create a project in it — it comes with its coordinator.</li>
                  <li>Drag roles from the palette onto the coordinator to build its team.</li>
                  <li>Select the coordinator and give it an objective.</li>
                </ol>
                <div className="actions">
                  <button
                    type="button"
                    className="button"
                    onClick={() => setDialog({ kind: "newDepartment", reportsTo: null })}
                  >
                    Create a department
                  </button>
                  <button
                    type="button"
                    className="button button--quiet"
                    onClick={() => setDialog({ kind: "hire", roleId: null, reportsTo: null })}
                  >
                    Hire a superintendent
                  </button>
                </div>
              </div>
            )}
          </TopologyCanvas>
        ) : (
          <Directory snapshot={snapshot} query={query} selectedId={selectedId} onSelect={reveal} />
        )}
        {selectedExists && selectedId && (
          <Inspector
            snapshot={snapshot}
            selectedId={selectedId}
            revision={org.revision}
            actions={actions}
            onSelect={reveal}
            onClose={() => setSelected(null)}
          />
        )}
      </div>

      <div className="toasts" aria-live="polite">
        {toasts.map((t) => (
          <div
            key={t.id}
            className={`toast toast--${t.tone}`}
            role={t.tone === "error" ? "alert" : "status"}
          >
            {t.text}
          </div>
        ))}
      </div>

      {drop && (
        <DropMenu
          title={drop.title}
          x={drop.x}
          y={drop.y}
          choices={drop.choices}
          onClose={() => setDrop(null)}
        />
      )}

      {dialog?.kind === "hire" && (
        <HireDialog
          snapshot={snapshot}
          roleId={dialog.roleId}
          {...(dialog.reportsTo !== undefined ? { reportsTo: dialog.reportsTo } : {})}
          onCancel={closeDialog}
          onSubmit={(input) => submit(() => hirePosition(input), `Hired ${input.title}.`)}
        />
      )}
      {dialog?.kind === "newDepartment" && (
        <NewDepartmentDialog
          snapshot={snapshot}
          reportsTo={dialog.reportsTo}
          onCancel={closeDialog}
          onSubmit={(input) => submit(() => createDepartment(input), `Created ${input.name}.`)}
        />
      )}
      {editingDepartment && (
        <EditDepartmentDialog
          department={editingDepartment}
          onCancel={closeDialog}
          onSubmit={(input) =>
            submit(() => updateDepartment(editingDepartment.id, input), "Saved.")
          }
        />
      )}
      {dialog?.kind === "newProject" && (
        <NewProjectDialog
          snapshot={snapshot}
          departmentId={dialog.departmentId}
          onCancel={closeDialog}
          onSubmit={(input) => submit(() => createProject(input), `Created ${input.name}.`)}
        />
      )}
      {editingProject && (
        <EditProjectDialog
          snapshot={snapshot}
          project={editingProject}
          onCancel={closeDialog}
          onSubmit={(input) => submit(() => updateProject(editingProject.id, input), "Saved.")}
        />
      )}
      {dialog?.kind === "role" && (
        <RoleDialog
          onCancel={closeDialog}
          onSubmit={(input) => submit(() => createRole(input), `Added the ${input.name} role.`)}
        />
      )}
      {dialog?.kind === "rename" && (
        <RenameDialog
          name={snapshot.name}
          onCancel={closeDialog}
          onSubmit={(name) => submit(() => renameOrganization(name))}
        />
      )}
      {dialog?.kind === "confirm" && (
        <ConfirmDialog
          title={dialog.title}
          message={dialog.message}
          confirmLabel={dialog.confirmLabel}
          danger
          onCancel={closeDialog}
          onConfirm={() => submit(dialog.work)}
        />
      )}
    </section>
  );
}

function Kpi({
  label,
  value,
  detail,
  tone,
}: {
  label: string;
  value: number | string;
  detail?: string | undefined;
  tone?: "working" | "waiting" | "bad" | undefined;
}) {
  return (
    <div className="kpi" data-tone={tone}>
      <dt>{label}</dt>
      <dd>
        {value}
        {detail && <span className="kpi__detail">{detail}</span>}
      </dd>
    </div>
  );
}

function RenameDialog({
  name: current,
  onCancel,
  onSubmit,
}: {
  name: string;
  onCancel: () => void;
  onSubmit: (name: string) => Promise<string | null>;
}) {
  const [name, setName] = useState(current);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const save = async (e: FormEvent) => {
    e.preventDefault();
    setPending(true);
    const failure = await onSubmit(name.trim());
    setPending(false);
    setError(failure);
  };
  return (
    <Modal title="Rename organization" onClose={onCancel}>
      <form className="modal__body" aria-label="Rename organization" onSubmit={(e) => void save(e)}>
        <label className="field">
          <span>Name</span>
          <input value={name} maxLength={120} required onChange={(e) => setName(e.target.value)} />
        </label>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        <footer className="modal__footer">
          <button type="button" className="button button--quiet" onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" className="button" disabled={pending || name.trim() === ""}>
            Save
          </button>
        </footer>
      </form>
    </Modal>
  );
}
