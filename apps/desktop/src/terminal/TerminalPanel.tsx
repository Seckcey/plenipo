import { useCallback, useEffect, useRef, useState } from "react";
import type { LedgerEvent, OrgSnapshot, ServersSnapshot, TerminalSettings } from "@plenipo/types";
import {
  Button,
  CountBadge,
  EmptyState,
  MenuButton,
  StatusDot,
  StatusPill,
  Tabs,
  cx,
  type MenuItem,
  type ThemeName,
} from "@plenipo/ui";

import { getLiveView, getOrganization, getServers, getTerminalSettings } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";
import { CodeWatchView } from "./CodeWatchView";
import { OwnerTerminal } from "./OwnerTerminal";
import { useWorkspaceIfAny } from "../workspace/context";
import { TERMINAL_BUTTON_ID, type TerminalTab } from "./panel";
import { useTerminal } from "./useTerminal";
import { watchTitle } from "./watch";
import { WatchView } from "./WatchView";

const HERE = "this-pc";
/** "Watch a worker" choices in the New menu (ADR-055 §1): `watch:<positionId>`. */
const WATCH_PREFIX = "watch:";
const NOBODY = "watch-nobody";

/** An agent working now, for "Watch a worker" (or one with a Watch tab open). */
interface Working {
  positionId: string;
  title: string;
}

/** Each position's team, in words: "Website Supervisor's team", or "reports to you". */
function teamsOf(org: OrgSnapshot): Map<string, string> {
  const titles = new Map(org.positions.map((p) => [p.id, p.title]));
  const teams = new Map<string, string>();
  for (const p of org.positions) {
    const lead = p.reportsTo === null ? undefined : titles.get(p.reportsTo);
    teams.set(p.id, p.reportsTo === null ? "reports to you" : lead ? `${lead}'s team` : "");
  }
  return teams;
}

/**
 * The name to show for each agent. A title is unique only within a team, so when two agents
 * share one, each gets its team: "Senior Developer (Website Supervisor's team)"; when that is not
 * known or not enough, a number. `missing`: the agents whose team is needed and not known yet.
 */
function nameAgents(
  agents: readonly Working[],
  teams: ReadonlyMap<string, string>,
): { names: Map<string, string>; missing: string[] } {
  const byTitle = new Map<string, string[]>();
  for (const a of agents) {
    const ids = byTitle.get(a.title) ?? [];
    if (!ids.includes(a.positionId)) ids.push(a.positionId);
    byTitle.set(a.title, ids);
  }
  const names = new Map<string, string>();
  const missing: string[] = [];
  for (const [title, ids] of byTitle) {
    if (ids.length === 1) {
      names.set(ids[0] ?? "", title);
      continue;
    }
    const withTeam = ids.map((id) => {
      const team = teams.get(id);
      if (team === undefined) missing.push(id);
      return team ? `${title} (${team})` : title;
    });
    const distinct = new Set(withTeam).size === withTeam.length;
    ids.forEach((id, i) => {
      const name = withTeam[i] ?? title;
      names.set(id, distinct ? name : `${name} (${i + 1})`);
    });
  }
  return { names, missing };
}

/** Settings changes that change the New terminal list: servers, the switches, the shell. */
function changesTheList(e: LedgerEvent): boolean {
  return (
    e.eventType.startsWith("guard.server_") ||
    e.eventType === "guard.switches_changed" ||
    e.eventType === "org.settings_changed"
  );
}

/** Events after which who is working may have changed. */
function changesWhoWorks(e: LedgerEvent): boolean {
  return e.eventType === "task.state_changed" || e.eventType.startsWith("guard.grant");
}

/** `name`: a Watch tab's agent, as it is named in the panel. */
function tabLabel(tab: TerminalTab, writing: boolean, name: string) {
  if (tab.kind === "code") {
    return (
      <>
        Watch · {name}
        {writing && <StatusDot status="pending" label="being written" />}
      </>
    );
  }
  if (tab.kind === "owner") {
    return (
      <>
        {tab.title}
        {tab.environment === "production" && <StatusPill status="error" label="PRODUCTION" />}
      </>
    );
  }
  const w = tab.watch;
  return (
    <>
      <StatusDot
        status={w.connected ? "ok" : "offline"}
        label={w.connected ? "Connected" : "Disconnected"}
      />
      {watchTitle(w)}
      {w.environment === "production" && <StatusPill status="error" label="PRODUCTION" />}
    </>
  );
}

function closeLabel(tab: TerminalTab, name: string): string {
  if (tab.kind === "code") return `Close Watch for ${name}`;
  if (tab.kind === "owner" && tab.place.kind === "aiTool") return `Close ${tab.title}`;
  return tab.kind === "owner"
    ? `Close the terminal on ${tab.place.kind === "thisPc" ? "this PC" : tab.title}`
    : `Close ${watchTitle(tab.watch)}`;
}

/**
 * The terminal panel (Phase 12, ADR-031): the owner's terminals and the workers' watch tabs,
 * each with a close button, shown and hidden with the Terminal button or Ctrl+`. Where it is — a
 * dock at the bottom, left, or right, or its own window — and its size are the window's layout
 * (Phase 21, ADR-092). A hidden panel keeps its terminals running, and so does one that moves.
 */
export function TerminalPanel({ theme }: { theme: ThemeName }) {
  const t = useTerminal();
  const workspace = useWorkspaceIfAny();
  const place = workspace?.layout.panels.terminal;
  // A side dock is narrow: the Watch tab puts its file list above the file.
  const narrow = place !== undefined && !place.popped && place.dock !== "bottom";
  const menuAbove = place !== undefined && !place.popped && place.dock === "bottom";
  const [servers, setServers] = useState<ServersSnapshot | null>(null);
  const [settings, setSettings] = useState<TerminalSettings | null>(null);
  const [working, setWorking] = useState<Working[]>([]);
  // Each position's team, read from the organization only when two agents share a title.
  const [teams, setTeams] = useState<ReadonlyMap<string, string>>(() => new Map());
  const [focusToken, setFocusToken] = useState(0);
  // The Watch tabs for code where a worker is writing a change now (a mark on the tab).
  const [writing, setWriting] = useState<ReadonlySet<string>>(() => new Set());
  const onWriting = useCallback((id: string, now: boolean) => {
    setWriting((all) => {
      if (all.has(id) === now) return all;
      const next = new Set(all);
      if (now) next.add(id);
      else next.delete(id);
      return next;
    });
  }, []);
  // A tab chosen with the mouse puts the keyboard in its terminal; one chosen with the arrow
  // keys keeps it in the list of tabs.
  const pointer = useRef(false);
  const open = t.open;
  /** F6 in a terminal: back to its tab in the list (in the window the panel is in). */
  const toTabs = () => {
    const doc = section.current?.ownerDocument ?? document;
    if (t.active) doc.getElementById(`terminal-tab-${t.active}`)?.focus();
  };
  // When the last tab closes, the keyboard goes to the panel's way to open one, not to the top
  // of the window.
  const section = useRef<HTMLElement>(null);
  const hadTabs = useRef(t.tabs.length > 0);
  useEffect(() => {
    const has = t.tabs.length > 0;
    const doc = section.current?.ownerDocument ?? document;
    const lost = doc.activeElement === null || doc.activeElement === doc.body;
    if (hadTabs.current && !has && open && lost) {
      section.current?.querySelector<HTMLButtonElement>(".terminal-panel__empty button")?.focus();
    }
    hadTabs.current = has;
  }, [t.tabs.length, open]);
  // Fresh servers and shell each time the panel is shown, and while it is open, whenever
  // Settings changes them (a server added, Remote computers (SSH) switched on, another shell).
  useEffect(() => {
    if (!open) return;
    let live = true;
    let stop: (() => void) | null = null;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const refresh = () => {
      getServers()
        .then((s) => live && setServers(s))
        .catch(() => undefined);
      getTerminalSettings()
        .then((s) => live && setSettings(s))
        .catch(() => undefined);
      // The agents working now, for "Watch a worker" (from the live view: nothing new is read).
      getLiveView()
        .then((v) => {
          if (!live) return;
          const seen = new Map<string, Working>();
          for (const w of v.workers) {
            if (w.positionId && !seen.has(w.positionId)) {
              seen.set(w.positionId, { positionId: w.positionId, title: w.worker });
            }
          }
          setWorking([...seen.values()].sort((a, b) => a.title.localeCompare(b.title)));
        })
        .catch(() => undefined);
    };
    void subscribeLedgerEvents((e) => {
      if (!live || timer || !(changesTheList(e) || changesWhoWorks(e))) return;
      timer = setTimeout(() => {
        timer = null;
        if (live) refresh();
      }, 300);
    })
      .then((s) => {
        if (live) stop = s;
        else s();
      })
      .catch(() => undefined)
      // Listen first, so a change between the answer and the listening is not missed.
      .finally(() => {
        if (live) refresh();
      });
    return () => {
      live = false;
      stop?.();
      if (timer) clearTimeout(timer);
    };
  }, [open]);

  // The agents in "Watch a worker" and in the Watch tabs, named so that no two look the same.
  const { names, missing } = nameAgents(
    [
      ...working,
      ...t.tabs.flatMap((tab) =>
        tab.kind === "code" ? [{ positionId: tab.positionId, title: tab.title }] : [],
      ),
    ],
    teams,
  );
  const nameOf = (positionId: string, title: string) => names.get(positionId) ?? title;
  const missingKey = [...missing].sort().join(" ");
  useEffect(() => {
    if (!missingKey) return;
    let live = true;
    getOrganization()
      .then((org) => {
        if (live) setTeams(teamsOf(org));
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [missingKey]);

  const watchItems: MenuItem[] =
    working.length === 0
      ? [
          {
            id: NOBODY,
            label: "Watch a worker (nobody is working now)",
            icon: "file",
            disabled: true,
          },
        ]
      : working.map((w) => ({
          id: `${WATCH_PREFIX}${w.positionId}`,
          label: `Watch ${nameOf(w.positionId, w.title)}`,
          icon: "file",
          hint: "Read-only",
        }));
  const items: MenuItem[] = [
    {
      id: HERE,
      label: "This PC",
      icon: "terminal",
      hint: settings ? (settings.otherShell ?? shellLabel(settings)) : undefined,
    },
    ...(servers?.servers ?? []).map((v): MenuItem => {
      const why = !servers?.switchedOn
        ? "Remote computers (SSH) is off"
        : !v.server.hostKey
          ? "its server ID is not pinned"
          : null;
      return {
        id: v.server.id,
        label: why ? `${v.server.name} (${why})` : v.server.name,
        icon: "server",
        disabled: why !== null,
        hint: v.server.environment === "production" ? "PRODUCTION" : undefined,
      };
    }),
    ...watchItems,
  ];

  const pick = (id: string) => {
    setFocusToken((n) => n + 1);
    if (id.startsWith(WATCH_PREFIX)) {
      const w = working.find((x) => `${WATCH_PREFIX}${x.positionId}` === id);
      if (w) t.openWatch(w.positionId, w.title);
      return;
    }
    if (id === HERE) {
      t.openHere();
      return;
    }
    const v = servers?.servers.find((s) => s.server.id === id);
    if (v) t.openServer(v.server.id, v.server.name, v.server.environment);
  };

  return (
    <section
      ref={section}
      className={cx("terminal-panel", `terminal-panel--${narrow ? "right" : "bottom"}`)}
      // In a window's layout, its dock shows and hides it; shown on its own, it does.
      hidden={!workspace && !open}
      aria-label="Terminal"
    >
      <div className="terminal-panel__main">
        <div className="terminal-panel__bar">
          {t.tabs.length > 0 && t.active !== null ? (
            <div
              className="terminal-panel__tabs"
              onPointerDown={() => {
                pointer.current = true;
              }}
              onKeyDown={() => {
                pointer.current = false;
              }}
            >
              <Tabs
                label="Terminals"
                value={t.active}
                onChange={(id) => {
                  t.setActive(id);
                  if (pointer.current) setFocusToken((n) => n + 1);
                }}
                idPrefix="terminal"
                tabs={t.tabs.map((tab) => {
                  const name = tab.kind === "code" ? nameOf(tab.positionId, tab.title) : "";
                  return {
                    value: tab.id,
                    label: tabLabel(tab, writing.has(tab.id), name),
                    onClose: () => t.close(tab.id),
                    closeLabel: closeLabel(tab, name),
                  };
                })}
              />
            </div>
          ) : (
            // In a dock, the dock's tab already names it.
            <span className="terminal-panel__title">{workspace ? "" : "Terminal"}</span>
          )}
          <div className="terminal-panel__actions">
            <MenuButton
              label="New terminal"
              icon="plus"
              variant="quiet"
              align="end"
              placement={menuAbove ? "above" : "below"}
              items={items}
              onSelect={pick}
            />
          </div>
        </div>
        <div className="terminal-panel__body">
          {t.tabs.length === 0 ? (
            <div className="terminal-panel__empty">
              <EmptyState
                pip="coding"
                title="No terminal open"
                action={
                  <Button
                    size="sm"
                    variant="primary"
                    icon="terminal"
                    onClick={() => {
                      setFocusToken((n) => n + 1);
                      t.openHere();
                    }}
                  >
                    Open a terminal on this PC
                  </Button>
                }
              >
                Type freely on this PC, or on one of your servers (New terminal). When a worker uses
                a server, its commands show here as they run.
              </EmptyState>
            </div>
          ) : (
            t.tabs.map((tab) => (
              <div
                key={tab.id}
                role="tabpanel"
                id={`terminal-panel-${tab.id}`}
                aria-labelledby={`terminal-tab-${tab.id}`}
                className="terminal-panel__tab"
                hidden={tab.id !== t.active}
              >
                {tab.kind === "owner" ? (
                  <OwnerTerminal
                    tab={tab}
                    active={tab.id === t.active}
                    visible={open}
                    theme={theme}
                    focusToken={focusToken}
                    onLeave={toTabs}
                    onOpened={() => t.tabOpened(tab)}
                    onEnded={() => t.tabEnded(tab)}
                    onFailed={(message) => t.tabFailed(tab, message)}
                  />
                ) : tab.kind === "code" ? (
                  <CodeWatchView
                    tab={{ ...tab, title: nameOf(tab.positionId, tab.title) }}
                    onWriting={onWriting}
                  />
                ) : (
                  <WatchView tab={tab.watch} />
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </section>
  );
}

function shellLabel(settings: TerminalSettings): string | undefined {
  return settings.shells.find((s) => s.shell === settings.shell)?.label;
}

/** The Terminal button in the top bar, with the count of new watch tabs. */
export function TerminalButton() {
  const t = useTerminal();
  return (
    <Button
      id={TERMINAL_BUTTON_ID}
      size="sm"
      variant="quiet"
      icon="terminal"
      aria-pressed={t.open}
      aria-keyshortcuts="Control+`"
      title={t.open ? "Hide the terminal (Ctrl+`)" : "Show the terminal (Ctrl+`)"}
      onClick={t.toggle}
    >
      Terminal
      {t.unseen > 0 && <CountBadge count={t.unseen} label="new watch tabs" tone="pending" />}
    </Button>
  );
}
