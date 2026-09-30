import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { AppInfo, OrgSnapshot } from "@plenipo/types";
import {
  AppShell,
  Banner,
  BannerSlot,
  Button,
  NotificationBell,
  ThemeToggle,
  TopBar,
  useStoredState,
  useTheme,
} from "@plenipo/ui";

import {
  frontendReady,
  getAppInfo,
  getLedgerStatus,
  type PlenipoCommandError,
} from "./api/commands";
import { AgentsProvider } from "./agents/AgentsProvider";
import { subscribeDrops } from "./api/events";
import { isRunning } from "./agents/store";
import { useAgents } from "./agents/useAgents";
import { ControlBanner } from "./components/ControlBanner";
import { ALL_SCOPE, type ScopeId } from "./components/scope";
import { ScopePicker } from "./components/ScopePicker";
import { Sidebar } from "./components/Sidebar";
import {
  ALL_VIEWS,
  SECTION_OF,
  VIEW_TITLES,
  isPageKind,
  type Go,
  type Place,
  type ViewId,
} from "./components/views";
import { RuntimeProvider } from "./runtime/RuntimeProvider";
import { isActive } from "./runtime/store";
import { useRuntime } from "./runtime/useRuntime";
import { useControl } from "./control/useControl";
import { sessionWords } from "./control/words";
import { useApprovals } from "./guard/usePermissions";
import { useLearning } from "./learning/useLearning";
import { rankName, titlesOf } from "./org/titles";
import { useOrganizationNames } from "./org/useOrganizationNames";
import { OrganizationMenu } from "./orgs/OrganizationMenu";
import { OwnerButton } from "./owner/OwnerButton";
import { OwnerProvider } from "./owner/OwnerProvider";
import { DepartmentPage } from "./pages/DepartmentPage";
import { HomePage } from "./pages/HomePage";
import { ProjectPage } from "./pages/ProjectPage";
import { TaskPage } from "./pages/TaskPage";
import { WorkerPage } from "./pages/WorkerPage";
import { EditorPage } from "./files/EditorPage";
import { FilesButton } from "./files/FilesButton";
import { useFileExplorerDrops } from "./files/useObjectiveFiles";
import { nameOf, parseFileKey } from "./files/refs";
import { FilesPanel } from "./files/FilesPanel";
import { TerminalButton, TerminalPanel } from "./terminal/TerminalPanel";
import { TerminalProvider } from "./terminal/TerminalProvider";
import { useWorkspace, useWorkspaceArea } from "./workspace/context";
import { Dock, DropMarks, PanelPortals } from "./workspace/Dock";
import { WorkspaceProvider } from "./workspace/WorkspaceProvider";
import { ActivityView } from "./views/ActivityView";
import { ApprovalsView } from "./views/ApprovalsView";
import { DiagnosticsView } from "./views/DiagnosticsView";
import { GalleryView } from "./views/GalleryView";
import { OrganizationView } from "./views/OrganizationView";
import { ProjectsView } from "./views/ProjectsView";
import { RuntimesView } from "./views/RuntimesView";
import { SettingsView } from "./views/SettingsView";
import { WorkersView } from "./views/WorkersView";
import { RecoveryBanners } from "./upkeep/RecoveryBanners";
import { UpdateMark } from "./upkeep/UpdateSettings";
import { useWindowHeartbeat } from "./upkeep/useWindowHeartbeat";

type CoreState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo }
  | { status: "error"; error: PlenipoCommandError };

// Where you are (the page, and the department, project, worker, or task it is about) comes back
// after a restart (Phase 12). Selections inside a section survive a webview reload.
const PLACE_KEY = "plenipo.place";
const SELECTED_KEY = "plenipo.selectedExecution";
const SELECTED_TASK_KEY = "plenipo.selectedTask";
const SELECTED_SESSION_KEY = "plenipo.selectedSession";
const HOME: Place = { view: "home", id: null };
/** How many pages Back remembers. */
const MAX_TRAIL = 50;

function isPlace(v: unknown): v is Place {
  if (typeof v !== "object" || v === null) return false;
  const { view, id } = v as Record<string, unknown>;
  if (!ALL_VIEWS.some((x) => x === view)) return false;
  if (isPageKind(view as ViewId)) return typeof id === "string" && id.length > 0;
  return id === null || typeof id === "string";
}

const samePlace = (a: Place, b: Place) => a.view === b.view && a.id === b.id;

/** The top bar's words for a page: "Department · Operations", "Manager · Development Manager". */
function placeTitle(place: Place, org: OrgSnapshot | null): string {
  if (!isPageKind(place.view) || !place.id || !org) return VIEW_TITLES[place.view];
  switch (place.view) {
    case "department": {
      const d = org.departments.find((x) => x.id === place.id);
      return d ? `Department · ${d.name}` : "Department";
    }
    case "project": {
      const p = org.projects.find((x) => x.id === place.id);
      return p ? `Project · ${p.name}` : "Project";
    }
    case "worker": {
      const p = org.positions.find((x) => x.id === place.id);
      return p ? `${rankName(titlesOf(org), p.kind)} · ${p.title}` : "Worker";
    }
    case "file": {
      const file = parseFileKey(place.id);
      return file ? `File · ${nameOf(file.path)}` : "File";
    }
    default:
      return VIEW_TITLES[place.view];
  }
}

/** Notices that mean data may be at risk get alert styling; others are informational. */
function isSevere(notice: string): boolean {
  return /integrity|damaged|temporary|could not|not opened|newer/i.test(notice);
}

function readSession(key: string): string | null {
  try {
    return sessionStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeSession(key: string, value: string | null) {
  try {
    if (value === null) sessionStorage.removeItem(key);
    else sessionStorage.setItem(key, value);
  } catch {
    // Storage unavailable: navigation simply isn't restored after a reload.
  }
}

export function App() {
  const [core, setCore] = useState<CoreState>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    getAppInfo()
      .then((info) => {
        if (cancelled) return;
        setCore({ status: "ready", info });
        // Best effort: only meaningful in smoke-test mode.
        frontendReady().catch(() => undefined);
      })
      .catch((error: PlenipoCommandError) => {
        if (!cancelled) setCore({ status: "error", error });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <RuntimeProvider>
      <AgentsProvider>
        <WorkspaceProvider>
          <TerminalProvider>
            <OwnerProvider>
              <Shell core={core} />
            </OwnerProvider>
          </TerminalProvider>
        </WorkspaceProvider>
      </AgentsProvider>
    </RuntimeProvider>
  );
}

function Shell({ core }: { core: CoreState }) {
  const { state } = useRuntime();
  const agents = useAgents();
  const [place, setPlace] = useStoredState<Place>(PLACE_KEY, HOME, isPlace);
  /** The pages before this one, for Back (this window only). */
  const [trail, setTrail] = useState<Place[]>([]);
  const view = place.view;
  const [selectedSession, setSelectedSession] = useState<string | null>(() =>
    readSession(SELECTED_SESSION_KEY),
  );
  const [selected, setSelected] = useState<string | null>(() => readSession(SELECTED_KEY));
  const [selectedTask, setSelectedTask] = useState<string | null>(() =>
    readSession(SELECTED_TASK_KEY),
  );
  const [orgFocus, setOrgFocus] = useState<string | null>(null);
  const [projectFocus, setProjectFocus] = useState<string | null>(null);
  const [theme, setTheme] = useTheme();
  const organization = useOrganizationNames();
  const [ledgerNotices, setLedgerNotices] = useState<string[]>([]);
  const [noticesDismissed, setNoticesDismissed] = useState(false);
  const main = useRef<HTMLElement>(null);
  const workspace = useWorkspace();
  const workArea = useWorkspaceArea();
  // Plenipo brings the window back if its page stops answering (Phase 13).
  useWindowHeartbeat();
  // Files dropped from File Explorer go to the objective box they land on (Phase 21).
  useFileExplorerDrops(subscribeDrops);

  // Every view shares one scroll area: open each page at its top, not where the last one was.
  useLayoutEffect(() => {
    if (main.current) main.current.scrollTop = 0;
  }, [view, place.id]);

  useEffect(() => {
    getLedgerStatus()
      .then((status) => setLedgerNotices(status.notices))
      .catch(() => undefined);
  }, []);
  const approvals = useApprovals();
  const learning = useLearning();
  const control = useControl();
  const waiting = approvals.queue?.pending ?? [];
  const controlling = (control.status?.sessions ?? []).filter((s) => s.state === "active");
  const activeCount = Object.values(state.executions).filter(isActive).length;
  const workingCount = Object.values(agents.state.sessions).filter(isRunning).length;
  const info = core.status === "ready" ? core.info : null;

  /**
   * Go somewhere: a section, or the page of one department, project, worker, or task. For a
   * section, `id` is what to show inside it (a position on the map, a project, a task in the
   * trail, a conversation, a Settings section). Back returns to the page before.
   */
  const go: Go = (next) => {
    if (next.id !== null) {
      if (next.view === "organization") setOrgFocus(next.id);
      else if (next.view === "projects") setProjectFocus(next.id);
      else if (next.view === "activity") selectTask(next.id);
      else if (next.view === "workers") selectSession(next.id);
    }
    // A page, a Settings section, and an AI tool's card keep their ID in the place.
    const target: Place =
      isPageKind(next.view) || next.view === "settings" || next.view === "runtimes"
        ? next
        : { view: next.view, id: null };
    if (samePlace(target, place)) return;
    setTrail((t) => [...t, place].slice(-MAX_TRAIL));
    setPlace(target);
  };
  const navigate = (next: ViewId) => go({ view: next, id: null });
  /** Back to the page before, or (after a restart) to the section the page belongs to. */
  const back = () => {
    const previous = trail[trail.length - 1];
    setTrail((t) => t.slice(0, -1));
    setPlace(previous ?? (isPageKind(view) ? { view: SECTION_OF[view], id: null } : HOME));
  };
  const select = (id: string | null) => {
    setSelected(id);
    writeSession(SELECTED_KEY, id);
  };
  const selectTask = (id: string | null) => {
    setSelectedTask(id);
    writeSession(SELECTED_TASK_KEY, id);
  };
  const selectSession = (id: string | null) => {
    setSelectedSession(id);
    writeSession(SELECTED_SESSION_KEY, id);
  };
  const showExecution = (id: string) => {
    select(id);
    navigate("runtimes");
  };
  const openSession = (id: string) => go({ view: "workers", id });
  const openTask = (id: string) => go({ view: "activity", id });
  const openPosition = (id: string) => go({ view: "organization", id });
  const clearOrgFocus = useCallback(() => setOrgFocus(null), []);
  const clearProjectFocus = useCallback(() => setProjectFocus(null), []);
  const severe = ledgerNotices.some(isSevere);
  const approvalCount = waiting.length + (learning.snapshot?.waiting.length ?? 0);

  /** Leaving for another section (the strip, the bell) starts a new trail. */
  const leave = (next: ViewId) => {
    setTrail([]);
    setPlace({ view: next, id: null });
  };

  /** Where you are, for the "Showing" picker: a department's or a project's page, or all. */
  const scope: ScopeId =
    view === "department" || view === "project" ? `${view}:${place.id ?? ""}` : ALL_SCOPE;
  /** Choosing where you are opens that part's page; everything opens Home. */
  const chooseScope = (next: ScopeId) => {
    const [kind, id] = next.split(":");
    if ((kind === "department" || kind === "project") && id) go({ view: kind, id });
    else go(HOME);
  };
  const pageBack = trail.length > 0 || isPageKind(view) ? back : undefined;

  return (
    <AppShell
      className="shell"
      rail={
        <Sidebar
          current={isPageKind(view) ? SECTION_OF[view] : view}
          onNavigate={leave}
          activeCount={activeCount}
          workingCount={workingCount}
          approvalCount={approvalCount}
        />
      }
      topBar={
        <TopBar
          start={
            <>
              <OrganizationMenu go={go} />
              <ScopePicker org={organization.snapshot} value={scope} onChange={chooseScope} />
            </>
          }
          title={placeTitle(place, organization.snapshot)}
          end={
            <>
              <UpdateMark go={go} />
              {info && (
                <span className="shell__version" aria-label="Application version">
                  v{info.version}
                </span>
              )}
              <OwnerButton />
              <FilesButton />
              <TerminalButton />
              <ThemeToggle theme={theme} onChange={setTheme} />
              <NotificationBell
                count={approvalCount}
                label="waiting for your approval"
                onOpen={() => leave("approvals")}
              />
            </>
          }
        />
      }
      footer={
        <footer className="shell__footer">
          {controlling.length > 0 ? (
            <span className="shell__footer-control">
              {controlling.length === 1 && controlling[0]
                ? sessionWords(controlling[0]).title
                : `${controlling.length} workers are using the browser, the desktop, or servers`}
            </span>
          ) : activeCount > 0 ? (
            `${activeCount} program${activeCount === 1 ? "" : "s"} running`
          ) : (
            "Ready · uses your own signed-in AI tools and never asks for passwords"
          )}
        </footer>
      }
    >
      <div ref={workArea} className="shell__work">
        <Dock side="left" />
        <main
          className={`shell__main${view === "organization" ? " shell__main--flush" : ""}`}
          ref={main}
        >
          <BannerSlot>
            <ControlBanner control={control} />
            <RecoveryBanners go={go} />
            {ledgerNotices.length > 0 && !noticesDismissed && (
              <Banner
                tone={severe ? "error" : "info"}
                role={severe ? "alert" : "status"}
                className={severe ? "banner--severe" : "banner--notice"}
                title={`Ledger notice${ledgerNotices.length > 1 ? "s" : ""}`}
                onDismiss={() => setNoticesDismissed(true)}
              >
                <ul className="banner__list">
                  {ledgerNotices.map((n) => (
                    <li key={n}>{n}</li>
                  ))}
                </ul>
              </Banner>
            )}
            {waiting.length > 0 && view !== "approvals" && (
              <Banner
                tone="pending"
                role="status"
                className="banner--approval"
                title={
                  waiting.length === 1
                    ? `${waiting[0]?.worker ?? "A worker"} is waiting for your approval`
                    : `${waiting.length} requests are waiting for your approval`
                }
                action={
                  <Button size="sm" variant="primary" onClick={() => navigate("approvals")}>
                    Review
                  </Button>
                }
              >
                {waiting.length === 1 && <div className="muted">{waiting[0]?.summary}</div>}
              </Banner>
            )}
          </BannerSlot>
          {core.status === "error" && (
            <p className="status status--error" role="alert">
              Plenipo Core is unavailable: {core.error.message}
            </p>
          )}
          {view === "home" && <HomePage go={go} approvals={approvals} learning={learning} />}
          {view === "department" && place.id && (
            <DepartmentPage key={place.id} id={place.id} go={go} onBack={pageBack} />
          )}
          {view === "project" && place.id && (
            <ProjectPage key={place.id} id={place.id} go={go} onBack={pageBack} />
          )}
          {view === "worker" && place.id && (
            <WorkerPage
              key={place.id}
              id={place.id}
              go={go}
              onBack={pageBack}
              onOpenSession={openSession}
            />
          )}
          {view === "task" && place.id && (
            <TaskPage
              key={place.id}
              id={place.id}
              go={go}
              onBack={pageBack}
              onOpenSession={openSession}
            />
          )}
          {view === "file" && place.id && <EditorPage id={place.id} go={go} onBack={pageBack} />}
          {view === "organization" && (
            <OrganizationView
              onOpenSession={openSession}
              onOpenTask={openTask}
              onOpenPage={go}
              focusId={orgFocus}
              onFocusHandled={clearOrgFocus}
            />
          )}
          {view === "projects" && (
            <ProjectsView
              onOpenTask={openTask}
              onOpenApprovals={() => navigate("approvals")}
              onOpenPage={go}
              focusId={projectFocus}
              onFocusHandled={clearProjectFocus}
            />
          )}
          {view === "workers" && (
            <WorkersView
              selectedSessionId={selectedSession}
              onSelectSession={selectSession}
              onShowExecution={showExecution}
              onOpenRuntimes={(id) => go({ view: "runtimes", id: id ?? null })}
              onOpenPosition={openPosition}
              onOpenPage={go}
            />
          )}
          {view === "approvals" && (
            <ApprovalsView onOpenTask={openTask} approvals={approvals} learning={learning} />
          )}
          {view === "runtimes" && (
            <RuntimesView selectedId={selected} onSelect={select} toolId={place.id} />
          )}
          {view === "activity" && (
            <ActivityView selectedTaskId={selectedTask} onSelectTask={selectTask} onOpenPage={go} />
          )}
          {view === "settings" && (
            <SettingsView
              go={go}
              info={info}
              section={place.id}
              // Choosing a section changes where you are, without a step for Back.
              onSection={(section) => setPlace({ view: "settings", id: section })}
            />
          )}
          {view === "diagnostics" && (
            <DiagnosticsView
              info={info}
              onTaskCreated={selectTask}
              onOpenGallery={() => navigate("gallery")}
            />
          )}
          {view === "gallery" && <GalleryView theme={theme} />}
        </main>
        <Dock side="right" />
        <Dock side="bottom" />
        <DropMarks />
        {workspace.popOutProblem && (
          <div className="shell__notice-popout">
            <Banner tone="error" role="alert" title={workspace.popOutProblem} />
          </div>
        )}
        <PanelPortals
          render={(panel) =>
            panel === "terminal" ? <TerminalPanel theme={theme} /> : <FilesPanel go={go} />
          }
        />
      </div>
    </AppShell>
  );
}
