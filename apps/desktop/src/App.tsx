import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import type { AppInfo } from "@plenipo/types";
import {
  AppShell,
  Banner,
  BannerSlot,
  Button,
  NotificationBell,
  ThemeToggle,
  TopBar,
  useTheme,
} from "@plenipo/ui";

import {
  frontendReady,
  getAppInfo,
  getLedgerStatus,
  type PlenipoCommandError,
} from "./api/commands";
import { AgentsProvider } from "./agents/AgentsProvider";
import { isRunning } from "./agents/store";
import { useAgents } from "./agents/useAgents";
import { ControlBanner } from "./components/ControlBanner";
import { ALL_SCOPE, type ScopeId } from "./components/scope";
import { ScopePicker } from "./components/ScopePicker";
import { Sidebar } from "./components/Sidebar";
import { ALL_VIEWS, VIEW_TITLES, type ViewId } from "./components/views";
import { RuntimeProvider } from "./runtime/RuntimeProvider";
import { isActive } from "./runtime/store";
import { useRuntime } from "./runtime/useRuntime";
import { useControl } from "./control/useControl";
import { sessionWords } from "./control/words";
import { useApprovals } from "./guard/usePermissions";
import { useLearning } from "./learning/useLearning";
import { useOrganizationNames } from "./org/useOrganizationNames";
import { TerminalButton, TerminalPanel } from "./terminal/TerminalPanel";
import { TerminalProvider } from "./terminal/TerminalProvider";
import { useTerminal } from "./terminal/useTerminal";
import { ActivityView } from "./views/ActivityView";
import { ApprovalsView } from "./views/ApprovalsView";
import { DiagnosticsView } from "./views/DiagnosticsView";
import { GalleryView } from "./views/GalleryView";
import { OrganizationView } from "./views/OrganizationView";
import { ProjectsView } from "./views/ProjectsView";
import { RuntimesView } from "./views/RuntimesView";
import { SettingsView } from "./views/SettingsView";
import { WorkersView } from "./views/WorkersView";

type CoreState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo }
  | { status: "error"; error: PlenipoCommandError };

// UI position survives a webview reload (the backend owns everything else).
const VIEW_KEY = "plenipo.view";
const SELECTED_KEY = "plenipo.selectedExecution";
const SELECTED_TASK_KEY = "plenipo.selectedTask";
const SELECTED_SESSION_KEY = "plenipo.selectedSession";
// Where you are (the top bar's "Showing" picker), for this window's life. Until Phase 12's pages
// filter by it, it names the place a pick opened, and goes back to everything when you leave.
const SCOPE_KEY = "plenipo.scope";

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

function initialView(): ViewId {
  const saved = readSession(VIEW_KEY);
  return ALL_VIEWS.find((v) => v === saved) ?? "organization";
}

function readScope(): ScopeId {
  return readSession(SCOPE_KEY) ?? ALL_SCOPE;
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
        <TerminalProvider>
          <Shell core={core} />
        </TerminalProvider>
      </AgentsProvider>
    </RuntimeProvider>
  );
}

function Shell({ core }: { core: CoreState }) {
  const { state } = useRuntime();
  const agents = useAgents();
  const [view, setView] = useState<ViewId>(initialView);
  const [selectedSession, setSelectedSession] = useState<string | null>(() =>
    readSession(SELECTED_SESSION_KEY),
  );
  const [selected, setSelected] = useState<string | null>(() => readSession(SELECTED_KEY));
  const [selectedTask, setSelectedTask] = useState<string | null>(() =>
    readSession(SELECTED_TASK_KEY),
  );
  const [orgFocus, setOrgFocus] = useState<string | null>(null);
  const [projectFocus, setProjectFocus] = useState<string | null>(null);
  const [scope, setScope] = useState<ScopeId>(readScope);
  const [theme, setTheme] = useTheme();
  const organization = useOrganizationNames();
  const [ledgerNotices, setLedgerNotices] = useState<string[]>([]);
  const [noticesDismissed, setNoticesDismissed] = useState(false);
  const main = useRef<HTMLElement>(null);
  const { measure: measureWork, panel: terminalPanel, size: terminalSize } = useTerminal();

  // Every view shares one scroll area: open each view at its top, not where the last one was.
  useLayoutEffect(() => {
    if (main.current) main.current.scrollTop = 0;
  }, [view]);

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

  const navigate = (next: ViewId) => {
    setView(next);
    writeSession(VIEW_KEY, next);
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
  const openSession = (id: string) => {
    selectSession(id);
    navigate("workers");
  };
  const openTask = (id: string) => {
    selectTask(id);
    navigate("activity");
  };
  const openPosition = (id: string) => {
    setOrgFocus(id);
    navigate("organization");
  };
  const clearOrgFocus = useCallback(() => setOrgFocus(null), []);
  const clearProjectFocus = useCallback(() => setProjectFocus(null), []);
  const severe = ledgerNotices.some(isSevere);
  const approvalCount = waiting.length + (learning.snapshot?.waiting.length ?? 0);

  /** Leaving for another section (the strip, the bell): back to the whole organization. */
  const leave = (next: ViewId) => {
    setScope(ALL_SCOPE);
    writeSession(SCOPE_KEY, null);
    navigate(next);
  };

  /** Choosing where you are opens that part: a department on the map, a project's page. */
  const chooseScope = (next: ScopeId) => {
    setScope(next);
    writeSession(SCOPE_KEY, next);
    const [kind, id] = next.split(":");
    const org = organization.snapshot;
    if (kind === "department" && id) {
      const head = org?.departments.find((d) => d.id === id)?.headPositionId;
      if (head) openPosition(head);
      else navigate("organization");
    } else if (kind === "project" && id) {
      setProjectFocus(id);
      navigate("projects");
    }
  };

  return (
    <AppShell
      className="shell"
      rail={
        <Sidebar
          current={view}
          onNavigate={leave}
          activeCount={activeCount}
          workingCount={workingCount}
          approvalCount={approvalCount}
        />
      }
      topBar={
        <TopBar
          start={<ScopePicker org={organization.snapshot} value={scope} onChange={chooseScope} />}
          title={VIEW_TITLES[view]}
          end={
            <>
              {info && (
                <span className="shell__version" aria-label="Application version">
                  v{info.version}
                </span>
              )}
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
      <div
        ref={measureWork}
        className={`shell__work shell__work--${terminalPanel.side}`}
        style={{ "--terminal-size": `${terminalSize}px` } as CSSProperties}
      >
        <main
          className={`shell__main${view === "organization" ? " shell__main--flush" : ""}`}
          ref={main}
        >
          <BannerSlot>
            <ControlBanner control={control} />
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
          {view === "organization" && (
            <OrganizationView
              onOpenSession={openSession}
              onOpenTask={openTask}
              focusId={orgFocus}
              onFocusHandled={clearOrgFocus}
            />
          )}
          {view === "projects" && (
            <ProjectsView
              onOpenTask={openTask}
              onOpenApprovals={() => navigate("approvals")}
              focusId={projectFocus}
              onFocusHandled={clearProjectFocus}
            />
          )}
          {view === "workers" && (
            <WorkersView
              selectedSessionId={selectedSession}
              onSelectSession={selectSession}
              onShowExecution={showExecution}
              onOpenRuntimes={() => navigate("runtimes")}
              onOpenPosition={openPosition}
            />
          )}
          {view === "approvals" && (
            <ApprovalsView onOpenTask={openTask} approvals={approvals} learning={learning} />
          )}
          {view === "runtimes" && <RuntimesView selectedId={selected} onSelect={select} />}
          {view === "activity" && (
            <ActivityView selectedTaskId={selectedTask} onSelectTask={selectTask} />
          )}
          {view === "settings" && <SettingsView />}
          {view === "diagnostics" && (
            <DiagnosticsView
              info={info}
              onTaskCreated={selectTask}
              onOpenGallery={() => navigate("gallery")}
            />
          )}
          {view === "gallery" && <GalleryView theme={theme} />}
        </main>
        <TerminalPanel theme={theme} />
      </div>
    </AppShell>
  );
}
