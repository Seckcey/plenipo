import { useEffect, useState } from "react";
import type { AppInfo } from "@plenipo/types";

import {
  frontendReady,
  getAppInfo,
  getLedgerStatus,
  type PlenipoCommandError,
} from "./api/commands";
import { BrandMark } from "./components/BrandMark";
import { Sidebar } from "./components/Sidebar";
import { VIEWS, type ViewId } from "./components/views";
import { RuntimeProvider } from "./runtime/RuntimeProvider";
import { isActive } from "./runtime/store";
import { useRuntime } from "./runtime/useRuntime";
import { ActivityView } from "./views/ActivityView";
import { DiagnosticsView } from "./views/DiagnosticsView";
import { OrganizationView } from "./views/OrganizationView";
import { RuntimesView } from "./views/RuntimesView";
import { SettingsView } from "./views/SettingsView";

type CoreState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo }
  | { status: "error"; error: PlenipoCommandError };

// UI position survives a webview reload (the backend owns everything else).
const VIEW_KEY = "plenipo.view";
const SELECTED_KEY = "plenipo.selectedExecution";
const SELECTED_TASK_KEY = "plenipo.selectedTask";

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
  return VIEWS.find((v) => v.id === saved)?.id ?? "organization";
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
      <Shell core={core} />
    </RuntimeProvider>
  );
}

function Shell({ core }: { core: CoreState }) {
  const { state } = useRuntime();
  const [view, setView] = useState<ViewId>(initialView);
  const [selected, setSelected] = useState<string | null>(() => readSession(SELECTED_KEY));
  const [selectedTask, setSelectedTask] = useState<string | null>(() =>
    readSession(SELECTED_TASK_KEY),
  );
  const [ledgerNotices, setLedgerNotices] = useState<string[]>([]);
  const [noticesDismissed, setNoticesDismissed] = useState(false);

  useEffect(() => {
    getLedgerStatus()
      .then((status) => setLedgerNotices(status.notices))
      .catch(() => undefined);
  }, []);
  const activeCount = Object.values(state.executions).filter(isActive).length;
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
  const severe = ledgerNotices.some(isSevere);

  return (
    <div className="shell">
      <header className="shell__header">
        <BrandMark />
        <span className="shell__wordmark">Plenipo</span>
        {info && (
          <span className="shell__version" aria-label="Application version">
            v{info.version}
          </span>
        )}
      </header>

      <div className="shell__body">
        <Sidebar current={view} onNavigate={navigate} activeCount={activeCount} />
        <main className="shell__main">
          {ledgerNotices.length > 0 && !noticesDismissed && (
            <div
              className={`banner${severe ? " banner--severe" : ""}`}
              role={severe ? "alert" : "status"}
            >
              <div>
                <strong>Ledger notice{ledgerNotices.length > 1 ? "s" : ""}</strong>
                <ul>
                  {ledgerNotices.map((n) => (
                    <li key={n}>{n}</li>
                  ))}
                </ul>
              </div>
              <button type="button" className="link" onClick={() => setNoticesDismissed(true)}>
                Dismiss
              </button>
            </div>
          )}
          {core.status === "error" && (
            <p className="status status--error" role="alert">
              Plenipo Core is unavailable: {core.error.message}
            </p>
          )}
          {view === "organization" && <OrganizationView />}
          {view === "runtimes" && <RuntimesView selectedId={selected} onSelect={select} />}
          {view === "activity" && (
            <ActivityView selectedTaskId={selectedTask} onSelectTask={selectTask} />
          )}
          {view === "settings" && <SettingsView />}
          {view === "diagnostics" && <DiagnosticsView info={info} onTaskCreated={selectTask} />}
        </main>
      </div>

      <footer className="shell__footer">
        {activeCount > 0
          ? `${activeCount} process${activeCount === 1 ? "" : "es"} running`
          : "No providers or credentials required"}
      </footer>
    </div>
  );
}
