/**
 * The organization: a live topology map (like a network topology view) of departments,
 * managers, supervisors, their teams, and the workers brought in for tasks. Hire by dragging a
 * role onto a position, reorganize by dragging a position onto another, and assign reviewers,
 * QA evaluators, and security auditors the same way. Everything comes from the Workforce engine;
 * nothing is hard-coded here.
 *
 * Phase 18 (ADR-053, ADR-054): the toolbar, tiles placed by hand (Tidy up, with Undo), Move here
 * or Lend, the trash can (with Undo) and the Archived drawer, rewiring by line ends, filters, the
 * legend, the live view, and a first-time tour.
 */
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type FormEvent,
  type ReactNode,
} from "react";
import type { OrgSnapshot, OversightRole, PositionInfo, TilePlace } from "@plenipo/types";
import { Button, type MenuItem } from "@plenipo/ui";

import {
  archiveDepartment,
  archivePosition,
  archiveProject,
  assignOversight,
  bringBack,
  createDepartment,
  createProject,
  createRole,
  createSpecialty,
  deleteForGood,
  deleteSavedAgent,
  updateRole,
  endOversight,
  fillPosition,
  giveObjective,
  hireFromWorkforce,
  hirePosition,
  lendAgent,
  movePosition,
  removeSpecialty,
  renameOrganization,
  retargetOversight,
  sendHome,
  saveToWorkforce,
  toCommandError,
  updateDepartment,
  updatePosition,
  updateProject,
  updateSpecialty,
  vacatePosition,
  type ArchivedKind,
} from "../api/commands";
import {
  ArchivedDrawer,
  CanvasToolbar,
  CanvasTour,
  FiltersPanel,
  HelpPanel,
  LegendPanel,
} from "../components/org/CanvasPanels";
import { Directory, type DirectoryActions } from "../components/org/Directory";
import { DropMenu, type DropChoice } from "../components/org/DropMenu";
import { Glyph } from "../components/org/Glyph";
import { HirePalette } from "../components/org/HirePalette";
import { Inspector, type InspectorActions } from "../components/org/Inspector";
import { usePanelWidth } from "../components/org/inspector/panel";
import {
  DeleteForGoodDialog,
  HireFromWorkforceDialog,
  SpecialtyDialog,
} from "../components/org/OwnerControlDialogs";
import { ConfirmDialog, Modal } from "../components/org/Modal";
import type { Go } from "../components/views";
import {
  EditDepartmentDialog,
  EditProjectDialog,
  HireDialog,
  NewDepartmentDialog,
  NewProjectDialog,
  RoleDialog,
} from "../components/org/OrgDialogs";
import {
  TRASH,
  TopologyCanvas,
  type ArrangeMove,
  type CanvasHandle,
  type DragPayload,
  type LineEnd,
} from "../components/org/TopologyCanvas";
import { mergePlaces, movedPlaces } from "../org/arrange";
import { archivedCount, vpRoleId } from "../org/control";
import {
  NO_FILTERS,
  filtersActive,
  isCanvasFilters,
  visibleTiles,
  FILTER_KEYS,
  type CanvasFilters,
} from "../org/filters";
import { OVERSIGHT_LABEL, OVERSIGHT_NOUN, plural } from "../org/format";
import { ORG_ID, OWNER_ID, WHERE_ROW_GAP, ancestorsOf, layoutOrganization } from "../org/layout";
import { handoffMarks, whereLines } from "../org/live";
import { nodeContext } from "../org/nodes";
import {
  hireRefusal,
  lendRefusal,
  moveChoices,
  moveRefusal,
  oversightOrder,
  oversightRefusal,
  positionMap,
  trashTarget,
} from "../org/rules";
import { searchMatches } from "../org/search";
import { rankName, roleLabel, titleSet, withArticle } from "../org/titles";
import { LEGEND_KEY, TOUR_KEY, readFlag, writeFlag, type PointerMode } from "../org/tour";
import { useLiveView, useReducedMotion } from "../org/useLiveView";
import { useOrganization } from "../org/useOrganization";
import { usePlaces } from "../org/usePlaces";
import { useOwnerProfile } from "../owner/context";
import { useOpenWatch } from "../terminal/useTerminal";
import { useChatIfAny } from "../chat/context";

const SELECTED_KEY = "plenipo.orgSelected";
const MODE_KEY = "plenipo.orgMode";
const COLLAPSED_KEY = "plenipo.orgCollapsed";
const OVERSIGHT_KEY = "plenipo.orgOversight";
const PALETTE_KEY = "plenipo.orgPalette";
const POINTER_KEY = "plenipo.orgPointer";
const FILTERS_KEY = "plenipo.orgFilters";
const WHERE_KEY = "plenipo.orgWhere";
/** How long a note with Undo stays. */
const UNDO_MS = 10_000;
/** The most targets a line end's menu lists. */
const MAX_LINE_CHOICES = 40;

type Mode = "topology" | "list";

type Dialog =
  | { kind: "hire"; roleId: string | null; reportsTo?: string | null }
  | { kind: "newDepartment"; reportsTo: string | null; fromWorkforce?: string }
  | { kind: "editDepartment"; id: string }
  | { kind: "newProject"; departmentId: string | null; fromWorkforce?: string }
  | { kind: "deleteForGood"; target: ArchivedKind; id: string }
  | { kind: "hireSaved"; savedId: string }
  | { kind: "specialty"; roleId: string; specialtyId?: string }
  | { kind: "editProject"; id: string }
  | { kind: "role" }
  | { kind: "editRole"; id: string }
  | { kind: "rename" }
  | {
      kind: "confirm";
      title: string;
      message: ReactNode;
      confirmLabel: string;
      work: () => Promise<OrgSnapshot>;
      /** A note once done, with Undo (the trash can). */
      done?: { text: string; undo: () => Promise<OrgSnapshot>; undone: string };
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
  action?: { label: string; run: () => void };
}

type Panel = "filters" | "help" | "archived" | null;

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

function readFilters(): CanvasFilters {
  try {
    const v = JSON.parse(read(FILTERS_KEY) ?? "null") as unknown;
    return isCanvasFilters(v) ? v : NO_FILTERS;
  } catch {
    return NO_FILTERS;
  }
}

function readPointer(): PointerMode {
  const v = read(POINTER_KEY);
  return v === "pan" || v === "arrange" ? v : "select";
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
  onOpenPage,
  focusId = null,
  onFocusHandled,
}: {
  onOpenSession: (sessionId: string) => void;
  onOpenTask: (taskId: string) => void;
  /** Opens the page of a position, department, or project (Phase 12). */
  onOpenPage?: Go | undefined;
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
  const [paletteOpen, setPaletteOpen] = useState(() => read(PALETTE_KEY) !== "folded");
  const [query, setQuery] = useState("");
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [drop, setDrop] = useState<Drop | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastId = useRef(0);
  const [panelWidth, setPanelWidth] = usePanelWidth();
  const [pointer, setPointerState] = useState<PointerMode>(readPointer);
  const [filters, setFiltersState] = useState<CanvasFilters>(readFilters);
  const [panel, setPanel] = useState<Panel>(null);
  const [legendOpen, setLegendOpen] = useState(() => readFlag(LEGEND_KEY) === "shown");
  const [whereOn, setWhereOn] = useState(() => read(WHERE_KEY) === "on");
  const [touring, setTouring] = useState(() => readFlag(TOUR_KEY) !== "seen");
  /** Tiles following the pointer while they are arranged (not saved yet). */
  const [preview, setPreview] = useState<TilePlace[] | null>(null);
  const { profile: owner } = useOwnerProfile();
  const openWatch = useOpenWatch();
  const chat = useChatIfAny();
  const reducedMotion = useReducedMotion();

  const setMode = (next: Mode) => {
    setModeState(next);
    write(MODE_KEY, next);
  };
  const setSelected = setSelectedState;
  const updateCollapsed = setCollapsed;
  // Remember the view for this session (restored after navigating away and back).
  useEffect(() => write(SELECTED_KEY, selectedId), [selectedId]);
  useEffect(() => write(COLLAPSED_KEY, JSON.stringify([...collapsed])), [collapsed]);

  const toast = useCallback(
    (text: string, tone: Toast["tone"] = "ok", action?: Toast["action"]) => {
      const id = ++toastId.current;
      setToasts((list) => [...list.slice(-2), { id, text, tone, ...(action ? { action } : {}) }]);
      setTimeout(
        () => setToasts((list) => list.filter((t) => t.id !== id)),
        action ? UNDO_MS : tone === "error" ? 9000 : 4000,
      );
    },
    [],
  );
  const toastError = useCallback((message: string) => toast(message, "error"), [toast]);
  const { places, place, tidy } = usePlaces(snapshot, toastError);

  const setPointer = (next: PointerMode) => {
    setPointerState(next);
    write(POINTER_KEY, next);
  };
  const setFilters = useCallback((next: CanvasFilters) => {
    setFiltersState(next);
    write(FILTERS_KEY, filtersActive(next) ? JSON.stringify(next) : null);
  }, []);
  const toggleLegend = () => {
    setLegendOpen((open) => {
      writeFlag(LEGEND_KEY, open ? "hidden" : "shown");
      return !open;
    });
  };
  const toggleWhere = () => {
    setWhereOn((on) => {
      write(WHERE_KEY, on ? null : "on");
      return !on;
    });
  };
  const endTour = () => {
    writeFlag(TOUR_KEY, "seen");
    setTouring(false);
  };

  const filtered = useMemo(
    () => (snapshot ? visibleTiles(snapshot, filters) : null),
    [snapshot, filters],
  );
  const layoutOptions = useMemo(
    () => ({
      places,
      shown: filtered?.shown ?? null,
      ...(whereOn ? { rowGap: WHERE_ROW_GAP } : {}),
    }),
    [places, filtered, whereOn],
  );
  /** The canvas as saved, before any tile following the pointer. */
  const settled = useMemo(
    () => (snapshot ? layoutOrganization(snapshot, collapsed, layoutOptions) : null),
    [snapshot, collapsed, layoutOptions],
  );
  const layout = useMemo(
    () =>
      snapshot && preview
        ? layoutOrganization(snapshot, collapsed, {
            ...layoutOptions,
            places: mergePlaces(places, preview),
          })
        : settled,
    [snapshot, collapsed, layoutOptions, places, preview, settled],
  );
  const ctx = useMemo(() => (snapshot ? nodeContext(snapshot, owner) : null), [snapshot, owner]);
  const liveView = useLiveView(mode === "topology" && snapshot !== null);
  const canvasLive = useMemo(
    () =>
      snapshot && layout && liveView
        ? {
            where: whereOn ? whereLines(snapshot, liveView) : null,
            handoffs: handoffMarks(layout, liveView),
            reducedMotion,
          }
        : null,
    [snapshot, layout, liveView, whereOn, reducedMotion],
  );
  const titles = titleSet(snapshot?.titles);
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
      // An agent the canvas's filters hide: clear them, so the tile chosen is on the canvas.
      if (
        mode === "topology" &&
        filtered &&
        !filtered.shown.has(id) &&
        snapshot.positions.some((p) => p.id === id && p.active)
      ) {
        setFilters(NO_FILTERS);
      }
      setSelected(id);
    },
    [snapshot, collapsed, updateCollapsed, setSelected, mode, filtered, setFilters],
  );

  // Arriving from another view with a position to show.
  const [arrived, setArrived] = useState<string | null>(null);
  // Once the request is handled (cleared), the same place can be asked for again.
  if (!focusId && arrived !== null) setArrived(null);
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

  /** Run a change that answers with something other than the organization; then reload it. */
  const change = useCallback(
    async (work: () => Promise<unknown>): Promise<string | null> => {
      try {
        await work();
        // Wait for the organization as it is now, so the panel never shows the old value.
        await reload();
        return null;
      } catch (reason) {
        return toCommandError(reason).message;
      }
    },
    [reload],
  );

  const actions: InspectorActions = useMemo(
    () => ({
      run,
      change,
      giveObjective: async (positionId, objective, files) => {
        try {
          await (files
            ? giveObjective(positionId, objective, files.projectId, files.files)
            : giveObjective(positionId, objective));
          void reload();
          return null;
        } catch (reason) {
          return toCommandError(reason).message;
        }
      },
      hire: (reportsTo, roleId = null) => setDialog({ kind: "hire", roleId, reportsTo }),
      newDepartment: (reportsTo) => setDialog({ kind: "newDepartment", reportsTo }),
      newProject: (departmentId) => setDialog({ kind: "newProject", departmentId }),
      newRole: () => setDialog({ kind: "role" }),
      editRole: (id) => setDialog({ kind: "editRole", id }),
      editDepartment: (id) => setDialog({ kind: "editDepartment", id }),
      editProject: (id) => setDialog({ kind: "editProject", id }),
      newSpecialty: (roleId) => setDialog({ kind: "specialty", roleId }),
      editSpecialty: (specialtyId) => {
        const role = snapshot?.roles.find((r) => r.specialties.some((x) => x.id === specialtyId));
        if (role) setDialog({ kind: "specialty", roleId: role.id, specialtyId });
      },
      rename: () => setDialog({ kind: "rename" }),
      confirm: (request) => setDialog({ kind: "confirm", ...request }),
      deleteForGood: (target, id) => setDialog({ kind: "deleteForGood", target, id }),
      openSession: onOpenSession,
      openTask: onOpenTask,
      openPage: onOpenPage,
      api: {
        fill: fillPosition,
        vacate: vacatePosition,
        update: updatePosition,
        move: movePosition,
        archive: archivePosition,
        assign: assignOversight,
        endOversight,
        archiveDepartment,
        archiveProject,
        bringBack,
        saveToWorkforce,
        sendHome,
        lend: lendAgent,
      },
    }),
    [run, change, reload, onOpenSession, onOpenTask, onOpenPage, snapshot],
  );

  const directoryActions: DirectoryActions = useMemo(
    () => ({
      bringBack: (kind, id, name) =>
        void runWithToast(() => bringBack(kind, id), `Brought back ${name}.`),
      deleteForGood: (target, id) => setDialog({ kind: "deleteForGood", target, id }),
      saveToWorkforce: (id, title) =>
        void runWithToast(() => saveToWorkforce(id), `Saved ${title} to your Workforce.`),
      hireSaved: (saved) => setDialog({ kind: "hireSaved", savedId: saved.id }),
      deleteSaved: (saved) =>
        setDialog({
          kind: "confirm",
          title: `Delete ${saved.title} for good?`,
          message: (
            <p>
              It leaves your Workforce and cannot be hired again. This cannot be undone. The lessons
              its role keeps stay with the role.
            </p>
          ),
          confirmLabel: "Delete for good",
          work: () => deleteSavedAgent(saved.id),
        }),
    }),
    [runWithToast],
  );

  // ---- Drag and drop -------------------------------------------------------------------------

  const byId = useMemo(
    () => (snapshot ? positionMap(snapshot) : new Map<string, PositionInfo>()),
    [snapshot],
  );

  /** Why a line end cannot go to `target`; `null` when it can. */
  const lineRefusal = useCallback(
    (line: LineEnd, target: string): string | null => {
      if (!snapshot) return "Loading…";
      if (line.kind === "reports") {
        const moving = byId.get(line.positionId);
        if (!moving) return "That agent no longer exists.";
        const to = supervisorOf(byId, target);
        if (to === undefined) return "Drop the line's end on an agent, or on you.";
        return moveRefusal(snapshot, moving, to);
      }
      const o = snapshot.oversight.find((x) => x.id === line.oversightId);
      if (!o) return "That line is gone.";
      const next = byId.get(target);
      if (!next) return "Drop the line's end on an agent.";
      const overseer = line.end === "overseer" ? next : byId.get(o.overseerId);
      const team = line.end === "target" ? next : byId.get(o.targetId);
      if (!overseer || !team) return "That line is gone.";
      if (overseer.id === o.overseerId && team.id === o.targetId) return "It is already there.";
      return oversightRefusal(snapshot, overseer, team, o.role);
    },
    [snapshot, byId],
  );

  /** What the trash can would do with `payload`, or why it cannot. */
  const trashRefusal = useCallback(
    (payload: DragPayload): string | null => {
      if (!snapshot) return "Loading…";
      if (payload.kind !== "position") return "Only an agent can go in the trash can.";
      const p = byId.get(payload.positionId);
      if (!p) return "That agent no longer exists.";
      const t = trashTarget(snapshot, p);
      return "refused" in t ? t.refused : null;
    },
    [snapshot, byId],
  );

  const dropRefusal = useCallback(
    (payload: DragPayload, target: string): string | null => {
      if (!snapshot) return "Loading…";
      if (target === TRASH) return trashRefusal(payload);
      if (payload.kind === "line") return lineRefusal(payload.line, target);
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
      if (target_ && lendRefusal(snapshot, moving, target_) === null) return null;
      const oversee =
        target_ &&
        (["review", "qa", "security"] as OversightRole[]).some(
          (r) => oversightRefusal(snapshot, moving, target_, r) === null,
        );
      return oversee ? null : move;
    },
    [snapshot, byId, lineRefusal, trashRefusal],
  );

  /** Archive with the trash can (ADR-053 §9–§10): at once, or after a question for a lead. */
  const trash = useCallback(
    (positionId: string) => {
      if (!snapshot) return;
      const p = byId.get(positionId);
      if (!p) return;
      const t = trashTarget(snapshot, p);
      if ("refused" in t) {
        toast(t.refused, "error");
        return;
      }
      if (t.kind === "position") {
        void (async () => {
          const failure = await run(() => archivePosition(p.id));
          if (failure) {
            toast(failure, "error");
            return;
          }
          if (selectedId === p.id) setSelected(null);
          const again =
            p.staffing === "persistent" ? " Bringing it back starts a new conversation." : "";
          toast(`Archived ${p.title}.${again}`, "ok", {
            label: "Undo",
            run: () =>
              void runWithToast(() => bringBack("position", p.id), `Brought back ${p.title}.`),
          });
        })();
        return;
      }
      const project = t.kind === "project";
      setDialog({
        kind: "confirm",
        title: project
          ? `Archive the ${t.name} project and its team?`
          : `Archive the ${t.name} department and everything in it?`,
        message: (
          <p>
            {project
              ? `${p.title} and its team are archived with the project.`
              : `Its projects, their teams, and ${p.title} are archived with it.`}{" "}
            Nothing is deleted: Undo, or the Archived drawer, brings it all back.
          </p>
        ),
        confirmLabel: "Archive",
        work: () => (project ? archiveProject(t.id) : archiveDepartment(t.id)),
        done: {
          text: `Archived ${t.name}.`,
          undo: () => bringBack(t.kind, t.id),
          undone: `Brought back ${t.name}.`,
        },
      });
    },
    [snapshot, byId, run, runWithToast, toast, selectedId, setSelected],
  );

  /** Move a line's end (ADR-053 §8): the same move, or the oversight moved in one step. */
  const rewire = useCallback(
    (line: LineEnd, target: string) => {
      if (!snapshot) return;
      if (line.kind === "reports") {
        const moving = byId.get(line.positionId);
        const to = supervisorOf(byId, target);
        if (!moving || to === undefined) return;
        const lead = to ? byId.get(to)?.title : "you";
        void runWithToast(
          () => movePosition(moving.id, to),
          `${moving.title} now reports to ${lead ?? "you"}.`,
        );
        return;
      }
      const o = snapshot.oversight.find((x) => x.id === line.oversightId);
      const next = byId.get(target);
      if (!o || !next) return;
      const to = line.end === "overseer" ? { overseerId: next.id } : { targetId: next.id };
      const overseer = line.end === "overseer" ? next.title : ctx?.title(o.overseerId);
      const team = line.end === "target" ? next.title : ctx?.title(o.targetId);
      void runWithToast(
        () => retargetOversight(o.id, to),
        `${overseer ?? "It"} is now ${team ?? "that"}'s ${OVERSIGHT_NOUN[o.role]}.`,
      );
    },
    [snapshot, byId, runWithToast, ctx],
  );

  const onDrop = useCallback(
    (payload: DragPayload, target: string, at: { x: number; y: number }) => {
      if (!snapshot) return;
      if (target === TRASH) {
        if (payload.kind === "position") trash(payload.positionId);
        return;
      }
      if (payload.kind === "line") {
        rewire(payload.line, target);
        return;
      }
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
          label: "Move here",
          detail: `Reports to ${lead ? lead.title : `you (${rankName(titles, "owner")})`} for good${
            moving.staffing === "persistent" ? "; its team moves with it" : ""
          }`,
          run: () =>
            void runWithToast(
              () => movePosition(moving.id, to),
              `${moving.title} now reports to ${leadName}.`,
            ),
        });
      }
      if (lead && lendRefusal(snapshot, moving, lead) === null) {
        choices.push(
          {
            id: "lend-objective",
            label: "Lend for one objective",
            detail: `Helps ${lead.title}'s team with its next objective, then comes home`,
            run: () =>
              void runWithToast(
                () => lendAgent(moving.id, lead.id, "objective"),
                `${moving.title} is lent to ${lead.title}'s team for one objective.`,
              ),
          },
          {
            id: "lend-returned",
            label: "Lend until I send it home",
            detail: `Helps ${lead.title}'s team until you choose Send home`,
            run: () =>
              void runWithToast(
                () => lendAgent(moving.id, lead.id, "returned"),
                `${moving.title} is lent to ${lead.title}'s team until you send it home.`,
              ),
          },
        );
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
                `${moving.title} is now ${lead.title}'s ${OVERSIGHT_NOUN[role]}.`,
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
    [snapshot, byId, runWithToast, titles, trash, rewire],
  );

  /** A line end chosen with the keyboard (or a click): every agent it can go to. */
  const lineMenu = useCallback(
    (line: LineEnd, at: { x: number; y: number }) => {
      if (!snapshot) return;
      const targets: { id: string; label: string }[] = [];
      if (line.kind === "reports") {
        const moving = byId.get(line.positionId);
        if (!moving) return;
        for (const to of moveChoices(snapshot, moving)) {
          targets.push({
            id: to ?? ORG_ID,
            label: `Report to ${to ? (byId.get(to)?.title ?? "") : `you (${rankName(titles, "owner")})`}`,
          });
        }
      } else {
        for (const p of snapshot.positions) {
          if (p.active && lineRefusal(line, p.id) === null) {
            targets.push({
              id: p.id,
              label: line.end === "overseer" ? `Hand it to ${p.title}` : `Check ${p.title}'s team`,
            });
          }
        }
      }
      const name =
        line.kind === "reports"
          ? `Who ${byId.get(line.positionId)?.title ?? "it"} reports to`
          : "Move this oversight line";
      setDrop({
        title: name,
        x: at.x,
        y: at.y,
        choices:
          targets.length === 0
            ? [
                {
                  id: "none",
                  label: "Nowhere else it can go now",
                  run: () => undefined,
                },
              ]
            : targets.slice(0, MAX_LINE_CHOICES).map((t) => ({
                id: t.id,
                label: t.label,
                run: () => rewire(line, t.id),
              })),
      });
    },
    [snapshot, byId, titles, lineRefusal, rewire],
  );

  /** A lent agent's badge: Send home, or show the team it helps. */
  const lentMenu = useCallback(
    (positionId: string, at: { x: number; y: number }) => {
      const p = byId.get(positionId);
      const loan = p?.loan;
      if (!p || !loan) return;
      setDrop({
        title: `${p.title}: lent to ${loan.to}'s team`,
        x: at.x,
        y: at.y,
        choices: [
          {
            id: "home",
            label: "Send home",
            detail: loan.goingHome
              ? "Already going home after this task"
              : "If it is working, it finishes this task first",
            run: () =>
              void (async () => {
                try {
                  const next = await sendHome(p.id);
                  apply(next);
                  const still = next.positions.find((x) => x.id === p.id)?.loan;
                  toast(
                    still ? `${p.title} goes home after the task it is on.` : `${p.title} is home.`,
                  );
                } catch (reason) {
                  toast(toCommandError(reason).message, "error");
                }
              })(),
          },
          {
            id: "show",
            label: `Show ${loan.to}`,
            run: () => reveal(loan.toLeadId),
          },
        ],
      });
    },
    [byId, apply, toast, reveal],
  );

  /** A tile moved by hand: follow the pointer, then save where it was dropped. */
  const arrange = useCallback(
    (move: ArrangeMove | null, final: boolean) => {
      if (!settled || !move) {
        setPreview(null);
        return;
      }
      const moved = movedPlaces(settled, move.tileId, move.dx, move.dy, move.alone);
      if (final) {
        setPreview(null);
        if (Math.abs(move.dx) >= 1 || Math.abs(move.dy) >= 1) void place(moved);
      } else {
        setPreview(moved);
      }
    },
    [settled, place],
  );

  const tidyUp = useCallback(() => {
    void (async () => {
      const removed = await tidy();
      if (removed && removed.length > 0) {
        toast("Tidied up: every tile is back in neat rows.", "ok", {
          label: "Undo",
          run: () => void place(removed),
        });
      }
    })();
  }, [tidy, place, toast]);

  const describeDrag = useCallback(
    (payload: DragPayload, over: string | null) => {
      if (payload.kind === "role") {
        const role = snapshot?.roles.find((r) => r.id === payload.roleId);
        return {
          title: `Hire ${role ? roleLabel(titles, role) : "a role"}`,
          glyph: role?.glyph ?? "worker",
          hint: "Release to hire into this team",
        };
      }
      if (payload.kind === "line") {
        return {
          title: payload.line.kind === "reports" ? "Reports to…" : "Oversight line",
          glyph: "link",
          hint: "Release to move the line here",
        };
      }
      const p = byId.get(payload.positionId);
      let hint = "Release to choose: move here, lend, or oversee this team";
      if (over === TRASH && snapshot && p) {
        // What the trash can does with it (a refusal is shown instead, by the canvas).
        const t = trashTarget(snapshot, p);
        if (!("refused" in t)) {
          hint =
            t.kind === "position"
              ? "Release to archive it (with Undo)"
              : `Release to archive its ${t.kind} (asks first)`;
        }
      }
      return {
        title: p?.title ?? "Position",
        glyph: snapshot?.roles.find((r) => r.id === p?.roleId)?.glyph ?? "worker",
        hint,
      };
    },
    [snapshot, byId, titles],
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

  const togglePalette = () => {
    setPaletteOpen((open) => {
      write(PALETTE_KEY, open ? "folded" : "open");
      return !open;
    });
  };

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

  const addItems: MenuItem[] = [
    { id: "department", label: "A department", icon: "department" },
    { id: "project", label: "A project", icon: "projects" },
    { id: "role", label: "A new role", icon: "settings" },
    { id: "hire", label: "Hire an agent", icon: "user" },
  ];
  const add = (id: string) => {
    if (id === "department") setDialog({ kind: "newDepartment", reportsTo: null });
    else if (id === "project") setDialog({ kind: "newProject", departmentId: null });
    else if (id === "role") setDialog({ kind: "role" });
    else if (id === "hire")
      setDialog({
        kind: "hire",
        roleId: null,
        ...(selectedId && byId.has(selectedId) ? { reportsTo: selectedId } : {}),
      });
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
            <Button variant="primary" onClick={() => void org.reload()}>
              Try again
            </Button>
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
      // A position deleted for good (or moved to the Workforce) leaves the panel too.
      snapshot.positions.some((p) => p.id === selectedId && !p.deleted) ||
      selectedId.startsWith("worker:"));
  const empty = snapshot.positions.every((p) => !p.active);
  const editingDepartment =
    dialog?.kind === "editDepartment"
      ? snapshot.departments.find((d) => d.id === dialog.id)
      : undefined;
  const editingProject =
    dialog?.kind === "editProject" ? snapshot.projects.find((p) => p.id === dialog.id) : undefined;
  const editingRole =
    dialog?.kind === "editRole" ? snapshot.roles.find((r) => r.id === dialog.id) : undefined;
  const savedForHire =
    dialog?.kind === "hireSaved"
      ? snapshot.workforce.find((w) => w.id === dialog.savedId)
      : undefined;
  const editingSpecialty =
    dialog?.kind === "specialty" && dialog.specialtyId
      ? snapshot.roles.flatMap((r) => r.specialties).find((x) => x.id === dialog.specialtyId)
      : undefined;

  return (
    <section className="org" aria-labelledby="org-title">
      <header className="org__bar">
        <div className="org__top">
          <div className="org__name">
            <h1 id="org-title">{snapshot.name}</h1>
            <button type="button" className="link" onClick={() => setDialog({ kind: "rename" })}>
              Rename
            </button>
          </div>
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
      </header>

      {snapshot.notices.length > 0 && (
        <div className="org__notices" role="status">
          {snapshot.notices.map((n) => (
            <span key={n}>{n}</span>
          ))}
        </div>
      )}

      <div
        className={`org__body org__body--${mode}${selectedExists ? " has-inspector" : ""}`}
        style={{ "--inspector-width": `${panelWidth}px` } as CSSProperties}
      >
        {mode === "topology" && (
          <HirePalette
            snapshot={snapshot}
            open={paletteOpen}
            onToggle={togglePalette}
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
            faded={filtered?.faded ?? null}
            showOversight={showOversight}
            mode={pointer}
            onMode={setPointer}
            onArrange={arrange}
            onLineMenu={lineMenu}
            onLent={lentMenu}
            onWatch={
              openWatch ? (id: string) => openWatch(id, byId.get(id)?.title ?? "Agent") : null
            }
            onChat={
              chat
                ? (n) => {
                    if (n.kind === "position") {
                      chat.open({
                        positionId: n.position.id,
                        sessionId: n.position.agent?.sessionId ?? null,
                        title: n.position.title,
                      });
                    } else if (n.kind === "worker" && n.worker.sessionId) {
                      chat.open({
                        sessionId: n.worker.sessionId,
                        title: `${byId.get(n.positionId)?.title ?? "Worker"} (on call)`,
                      });
                    }
                  }
                : null
            }
            live={canvasLive}
            onSelect={setSelected}
            onToggle={toggle}
            dropRefusal={dropRefusal}
            onDrop={onDrop}
            describeDrag={describeDrag}
            insetRight={selectedExists ? panelWidth : 0}
            toolbar={
              <CanvasToolbar
                mode={pointer}
                onMode={setPointer}
                onTidy={tidyUp}
                canTidy={places.length > 0}
                filtersOpen={panel === "filters"}
                filterCount={FILTER_KEYS.filter((k) => filters[k] !== null).length}
                onFilters={() => setPanel(panel === "filters" ? null : "filters")}
                legendOpen={legendOpen}
                onLegend={toggleLegend}
                whereOn={whereOn}
                onWhere={toggleWhere}
                oversightOn={showOversight}
                onOversight={toggleOversight}
                addItems={addItems}
                onAdd={add}
                trashOpen={panel === "archived"}
                archived={archivedCount(snapshot)}
                onTrash={() => setPanel(panel === "archived" ? null : "archived")}
                helpOpen={panel === "help"}
                onHelp={() => setPanel(panel === "help" ? null : "help")}
              />
            }
          >
            {filtered && panel !== "filters" && (
              <div className="canvas-filter-note" data-canvas-ui role="status">
                Showing {filtered.matched} of {filtered.total} agents
                <button type="button" className="link" onClick={() => setFilters(NO_FILTERS)}>
                  Clear filters
                </button>
              </div>
            )}
            {panel === "filters" && (
              <FiltersPanel
                snapshot={snapshot}
                filters={filters}
                onChange={setFilters}
                matched={filtered?.matched ?? null}
                total={snapshot.positions.filter((p) => p.active).length}
                onClose={() => setPanel(null)}
              />
            )}
            {panel === "help" && (
              <HelpPanel
                onTour={() => {
                  setPanel(null);
                  setTouring(true);
                }}
                onClose={() => setPanel(null)}
              />
            )}
            {panel === "archived" && (
              <ArchivedDrawer
                snapshot={snapshot}
                actions={directoryActions}
                onSelect={reveal}
                onClose={() => setPanel(null)}
              />
            )}
            {legendOpen && <LegendPanel onClose={toggleLegend} />}
            {touring && !empty && <CanvasTour onDone={endTour} />}
            {empty && (
              <div className="topology__empty" data-canvas-ui>
                <h2>Build your organization</h2>
                <ol>
                  <li>
                    Create a department — it comes with its {rankName(titles, "departmentManager")}.
                  </li>
                  <li>
                    Create a project in it — it comes with its{" "}
                    {rankName(titles, "projectCoordinator")}.
                  </li>
                  <li>
                    Drag roles from the palette onto the {rankName(titles, "projectCoordinator")} to
                    build the team.
                  </li>
                  <li>
                    Select the {rankName(titles, "projectCoordinator")} and give it an objective.
                  </li>
                </ol>
                <div className="actions">
                  <Button
                    variant="primary"
                    onClick={() => setDialog({ kind: "newDepartment", reportsTo: null })}
                  >
                    Create a department
                  </Button>
                  <Button
                    variant="quiet"
                    onClick={() =>
                      setDialog({ kind: "hire", roleId: vpRoleId(snapshot), reportsTo: null })
                    }
                  >
                    Hire {withArticle(rankName(titles, "superintendent"))}
                  </Button>
                </div>
              </div>
            )}
          </TopologyCanvas>
        ) : (
          <Directory
            snapshot={snapshot}
            query={query}
            selectedId={selectedId}
            onSelect={reveal}
            actions={directoryActions}
          />
        )}
        {selectedExists && selectedId && (
          <Inspector
            snapshot={snapshot}
            selectedId={selectedId}
            revision={org.revision}
            actions={actions}
            onSelect={reveal}
            onClose={() => setSelected(null)}
            width={panelWidth}
            onWidth={setPanelWidth}
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
            {t.action && (
              <button
                type="button"
                className="toast__action"
                onClick={() => {
                  setToasts((list) => list.filter((x) => x.id !== t.id));
                  t.action?.run();
                }}
              >
                {t.action.label}
              </button>
            )}
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
          onHireSaved={(savedId, reportsTo, title) =>
            submit(() => hireFromWorkforce(savedId, reportsTo, title), `Hired ${title}.`)
          }
        />
      )}
      {dialog?.kind === "newDepartment" && (
        <NewDepartmentDialog
          snapshot={snapshot}
          reportsTo={dialog.reportsTo}
          {...(dialog.fromWorkforce ? { fromWorkforce: dialog.fromWorkforce } : {})}
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
          {...(dialog.fromWorkforce ? { fromWorkforce: dialog.fromWorkforce } : {})}
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
          titles={titles}
          onCancel={closeDialog}
          onSubmit={(input) => submit(() => createRole(input), `Added the ${input.name} role.`)}
        />
      )}
      {editingRole && (
        <RoleDialog
          titles={titles}
          role={editingRole}
          onCancel={closeDialog}
          onSubmit={() => Promise.resolve(null)}
          onUpdate={(input) =>
            submit(() => updateRole(editingRole.id, input), `Saved the ${input.name} role.`)
          }
        />
      )}
      {dialog?.kind === "rename" && (
        <RenameDialog
          name={snapshot.name}
          onCancel={closeDialog}
          onSubmit={(name) => submit(() => renameOrganization(name))}
        />
      )}
      {dialog?.kind === "deleteForGood" && (
        <DeleteForGoodDialog
          kind={dialog.target}
          id={dialog.id}
          onCancel={closeDialog}
          onDelete={(save) =>
            submit(
              () => deleteForGood(dialog.target, dialog.id, save),
              save.length > 0
                ? `Deleted for good; ${save.length} saved to your Workforce.`
                : "Deleted for good.",
            )
          }
        />
      )}
      {savedForHire && (
        <HireFromWorkforceDialog
          snapshot={snapshot}
          saved={savedForHire}
          onCancel={closeDialog}
          onSubmit={(reportsTo, title) =>
            submit(() => hireFromWorkforce(savedForHire.id, reportsTo, title), `Hired ${title}.`)
          }
          onNewDepartment={(fromWorkforce) =>
            setDialog({ kind: "newDepartment", reportsTo: null, fromWorkforce })
          }
          onNewProject={(fromWorkforce) =>
            setDialog({ kind: "newProject", departmentId: null, fromWorkforce })
          }
        />
      )}
      {dialog?.kind === "specialty" && (
        <SpecialtyDialog
          snapshot={snapshot}
          roleId={dialog.roleId}
          specialty={editingSpecialty}
          onCancel={closeDialog}
          onSubmit={(input) =>
            editingSpecialty
              ? submit(() => updateSpecialty(editingSpecialty.id, input), `Saved ${input.name}.`)
              : submit(() => createSpecialty(input), `Added the ${input.name} specialty.`)
          }
          onRemove={
            editingSpecialty
              ? () =>
                  submit(
                    () => removeSpecialty(editingSpecialty.id),
                    `Removed the ${editingSpecialty.name} specialty.`,
                  )
              : undefined
          }
        />
      )}
      {dialog?.kind === "confirm" && (
        <ConfirmDialog
          title={dialog.title}
          message={dialog.message}
          confirmLabel={dialog.confirmLabel}
          danger
          onCancel={closeDialog}
          onConfirm={async () => {
            const failure = await submit(dialog.work);
            const done = dialog.done;
            if (failure === null && done) {
              toast(done.text, "ok", {
                label: "Undo",
                run: () => void runWithToast(done.undo, done.undone),
              });
            }
            return failure;
          }}
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
          <Button variant="quiet" onClick={onCancel}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" disabled={pending || name.trim() === ""}>
            Save
          </Button>
        </footer>
      </form>
    </Modal>
  );
}
