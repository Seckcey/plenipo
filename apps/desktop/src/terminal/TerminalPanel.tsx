import { useEffect, useRef, useState } from "react";
import type { LedgerEvent, ServersSnapshot, TerminalSettings } from "@plenipo/types";
import {
  Button,
  CountBadge,
  EmptyState,
  IconButton,
  MenuButton,
  ResizeHandle,
  StatusDot,
  StatusPill,
  Tabs,
  cx,
  type MenuItem,
  type ThemeName,
} from "@plenipo/ui";

import { getServers, getTerminalSettings } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";
import { OwnerTerminal } from "./OwnerTerminal";
import { PANEL_MIN, TERMINAL_BUTTON_ID, type TerminalTab } from "./panel";
import { useTerminal } from "./useTerminal";
import { watchTitle } from "./watch";
import { WatchView } from "./WatchView";

const HERE = "this-pc";

/** Settings changes that change the New terminal list: servers, the switches, the shell. */
function changesTheList(e: LedgerEvent): boolean {
  return (
    e.eventType.startsWith("guard.server_") ||
    e.eventType === "guard.switches_changed" ||
    e.eventType === "org.settings_changed"
  );
}

function tabLabel(tab: TerminalTab) {
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

function closeLabel(tab: TerminalTab): string {
  return tab.kind === "owner"
    ? `Close the terminal on ${tab.place.kind === "thisPc" ? "this PC" : tab.title}`
    : `Close ${watchTitle(tab.watch)}`;
}

/**
 * The terminal panel (Phase 12, ADR-031): at the bottom of the window, or on the right; shown
 * and hidden with the Terminal button or Ctrl+`; its edge dragged to resize it. The owner's
 * terminals and the workers' watch tabs, each with a close button. A hidden panel keeps its
 * terminals running.
 */
export function TerminalPanel({ theme }: { theme: ThemeName }) {
  const t = useTerminal();
  const [servers, setServers] = useState<ServersSnapshot | null>(null);
  const [settings, setSettings] = useState<TerminalSettings | null>(null);
  const [focusToken, setFocusToken] = useState(0);
  // A tab chosen with the mouse puts the keyboard in its terminal; one chosen with the arrow
  // keys keeps it in the list of tabs.
  const pointer = useRef(false);
  const side = t.panel.side;
  const open = t.panel.open;
  /** F6 in a terminal: back to its tab in the list. */
  const toTabs = () => {
    if (t.active) document.getElementById(`terminal-tab-${t.active}`)?.focus();
  };
  // When the last tab closes, the keyboard goes to the panel's way to open one, not to the top
  // of the window.
  const section = useRef<HTMLElement>(null);
  const hadTabs = useRef(t.tabs.length > 0);
  useEffect(() => {
    const has = t.tabs.length > 0;
    const lost = document.activeElement === null || document.activeElement === document.body;
    if (hadTabs.current && !has && open && lost) {
      section.current?.querySelector<HTMLButtonElement>(".terminal-panel__empty button")?.focus();
    }
    hadTabs.current = has;
  }, [t.tabs.length, open]);
  /** Hidden with its button: the keyboard goes back to the Terminal button in the top bar. */
  const hide = () => {
    t.hide();
    document.getElementById(TERMINAL_BUTTON_ID)?.focus();
  };

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
    };
    void subscribeLedgerEvents((e) => {
      if (!live || timer || !changesTheList(e)) return;
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
  ];

  const pick = (id: string) => {
    setFocusToken((n) => n + 1);
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
      className={cx("terminal-panel", `terminal-panel--${side}`)}
      hidden={!open}
      aria-label="Terminal"
    >
      <ResizeHandle
        label="Resize the terminal panel"
        value={t.size}
        min={PANEL_MIN}
        max={t.maxSize}
        edge={side === "bottom" ? "top" : "left"}
        onChange={t.setSize}
      />
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
                tabs={t.tabs.map((tab) => ({
                  value: tab.id,
                  label: tabLabel(tab),
                  onClose: () => t.close(tab.id),
                  closeLabel: closeLabel(tab),
                }))}
              />
            </div>
          ) : (
            <span className="terminal-panel__title">Terminal</span>
          )}
          <div className="terminal-panel__actions">
            <MenuButton
              label="New terminal"
              icon="plus"
              variant="quiet"
              align="end"
              placement={side === "bottom" ? "above" : "below"}
              items={items}
              onSelect={pick}
            />
            <IconButton
              icon={side === "bottom" ? "panelOpen" : "panelClose"}
              label={
                side === "bottom"
                  ? "Move the terminal to the right"
                  : "Move the terminal to the bottom"
              }
              onClick={() => t.setSide(side === "bottom" ? "right" : "bottom")}
            />
            <IconButton icon="close" label="Hide the terminal (Ctrl+`)" onClick={hide} />
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
      aria-pressed={t.panel.open}
      aria-keyshortcuts="Control+`"
      title={t.panel.open ? "Hide the terminal (Ctrl+`)" : "Show the terminal (Ctrl+`)"}
      onClick={t.toggle}
    >
      Terminal
      {t.unseen > 0 && <CountBadge count={t.unseen} label="new watch tabs" tone="pending" />}
    </Button>
  );
}
