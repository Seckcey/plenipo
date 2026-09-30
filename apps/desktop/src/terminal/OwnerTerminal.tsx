import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import type { TerminalInfo } from "@plenipo/types";
import { ErrorState, terminalTheme, TERMINAL_FONT, type ThemeName } from "@plenipo/ui";

import {
  closeTerminal,
  openTerminal,
  resizeTerminal,
  toCommandError,
  writeTerminal,
} from "../api/commands";
import { usePanelWindow } from "../workspace/context";
import type { OwnerTab } from "./panel";
import { endedLine, fromBase64, pieces, screenReaderWanted } from "./words";

type Status =
  | { kind: "opening" }
  | { kind: "open"; info: TerminalInfo }
  | { kind: "ended"; why: string }
  | { kind: "failed"; message: string };

/**
 * One of the owner's terminals (ADR-031): xterm.js on screen, a shell on this PC or on a server
 * behind it, or an AI tool's own sign-in program (ADR-058). Typing goes to the program as it is
 * typed (a large paste in pieces), and nothing else ever does; nothing is recorded. Its colors
 * come from the terminal tokens, and follow the theme.
 */
export function OwnerTerminal({
  tab,
  active,
  visible,
  theme,
  focusToken,
  onLeave,
  onOpened,
  onEnded,
  onFailed,
}: {
  tab: OwnerTab;
  active: boolean;
  /** The panel is shown (a hidden terminal keeps running). */
  visible: boolean;
  theme: ThemeName;
  /** Changes when the panel is shown with the keyboard: the active terminal takes the focus. */
  focusToken: number;
  /** F6 in the terminal: the keyboard goes back to the panel's tabs (Tab itself is the shell's). */
  onLeave?: (() => void) | undefined;
  /** It opened (the first time, or again after Try again). */
  onOpened?: (() => void) | undefined;
  /** Its program ended (the tab stays, and says how). */
  onEnded?: (() => void) | undefined;
  /** It could not open, with Plenipo's reason. */
  onFailed?: ((message: string) => void) | undefined;
}) {
  const box = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const themeNow = useRef(theme);
  const [status, setStatus] = useState<Status>({ kind: "opening" });
  const [attempt, setAttempt] = useState(0);
  const leave = useRef(onLeave);
  const openedNow = useRef(onOpened);
  const endedNow = useRef(onEnded);
  const failedNow = useRef(onFailed);
  useLayoutEffect(() => {
    leave.current = onLeave;
    openedNow.current = onOpened;
    endedNow.current = onEnded;
    failedNow.current = onFailed;
  });

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    let disposed = false;
    let id: string | null = null;
    let ended = false;
    let typedEarly: string[] = [];
    const xterm = new Terminal({
      fontFamily: TERMINAL_FONT.family,
      fontSize: TERMINAL_FONT.size,
      theme: terminalTheme(themeNow.current),
      cursorBlink: true,
      scrollback: 5000,
      // Text on a colored background stays readable (programs color backgrounds too).
      minimumContrastRatio: 4.5,
      // Settings → Terminal: let a screen reader read what the terminal shows.
      screenReaderMode: screenReaderWanted(),
    });
    const fitter = new FitAddon();
    xterm.loadAddon(fitter);
    xterm.open(el);
    term.current = xterm;
    fit.current = fitter;
    const refit = () => {
      if (el.offsetParent === null) return;
      try {
        fitter.fit();
      } catch {
        // Not laid out yet.
      }
    };
    refit();
    // Ctrl+` belongs to Plenipo (it shows and hides the panel), not to the shell; so does F6,
    // which takes the keyboard back to the panel's tabs.
    xterm.attachCustomKeyEventHandler((e) => {
      if (e.ctrlKey && (e.code === "Backquote" || e.key === "`")) return false;
      if (e.key === "F6" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
        if (e.type === "keydown") {
          e.preventDefault();
          leave.current?.();
        }
        return false;
      }
      return true;
    });
    // What the owner types or pastes goes to the shell in order, in pieces Plenipo takes. If
    // Plenipo refuses a piece, the rest of that paste is not sent (a script with a hole in it
    // could do harm), and the terminal says why (once, until typing reaches the shell again).
    let sending: Promise<void> = Promise.resolve();
    let refused: string | null = null;
    const send = (to: string, data: string) => {
      const parts = pieces(data);
      sending = sending.then(async () => {
        for (const piece of parts) {
          if (disposed || ended) return;
          try {
            await writeTerminal(to, piece);
            refused = null;
          } catch (reason) {
            const message = toCommandError(reason).message;
            if (!disposed && !ended && message !== refused) {
              refused = message;
              xterm.write(`\r\n\x1b[2m[${message}]\x1b[0m\r\n`);
            }
            return;
          }
        }
      });
    };
    const typing = xterm.onData((data) => {
      if (ended) return;
      if (id) send(id, data);
      else typedEarly.push(data);
    });
    const sizing = xterm.onResize(({ cols, rows }) => {
      if (id && !ended) void resizeTerminal(id, cols, rows).catch(() => undefined);
    });
    const opened = { cols: xterm.cols, rows: xterm.rows };
    openTerminal(tab.place, opened.cols, opened.rows, (event) => {
      if (disposed) return;
      if (event.kind === "output") {
        xterm.write(fromBase64(event.data));
      } else {
        ended = true;
        xterm.write(`\r\n\x1b[2m[${endedLine(event.why, event.code)}]\x1b[0m\r\n`);
        setStatus({ kind: "ended", why: event.why });
        endedNow.current?.();
      }
    })
      .then((info) => {
        if (disposed) {
          void closeTerminal(info.id).catch(() => undefined);
          return;
        }
        id = info.id;
        if (!ended) {
          setStatus({ kind: "open", info });
          openedNow.current?.();
        }
        // The panel may have changed size while a server's terminal was connecting.
        if (!ended && (xterm.cols !== opened.cols || xterm.rows !== opened.rows)) {
          void resizeTerminal(info.id, xterm.cols, xterm.rows).catch(() => undefined);
        }
        for (const data of typedEarly) send(info.id, data);
        typedEarly = [];
      })
      .catch((reason: unknown) => {
        if (disposed) return;
        const message = toCommandError(reason).message;
        setStatus({ kind: "failed", message });
        failedNow.current?.(message);
      });
    return () => {
      disposed = true;
      typing.dispose();
      sizing.dispose();
      if (id && !ended) void closeTerminal(id).catch(() => undefined);
      xterm.dispose();
      term.current = null;
      fit.current = null;
    };
  }, [tab.place, attempt]);

  // The panel is in this window now (Plenipo's own, or its pop-out, ADR-092): the terminal
  // follows it, keeping what it shows, and fits the panel's size there.
  const panelWindow = usePanelWindow();
  useEffect(() => {
    const el = box.current;
    const xterm = term.current;
    if (!el || !xterm) return;
    xterm.open(el);
    const refit = () => {
      if (el.offsetParent === null) return;
      try {
        fit.current?.fit();
      } catch {
        // Not laid out yet.
      }
    };
    refit();
    // The observer of the window the panel is in now.
    const Observer =
      (panelWindow as Window & { ResizeObserver?: typeof ResizeObserver }).ResizeObserver ??
      (typeof ResizeObserver === "undefined" ? undefined : ResizeObserver);
    if (!Observer) return;
    const observer = new Observer(refit);
    observer.observe(el);
    return () => observer.disconnect();
  }, [panelWindow, attempt]);

  useEffect(() => {
    themeNow.current = theme;
    if (term.current) term.current.options.theme = terminalTheme(theme);
  }, [theme]);

  // Shown: fit the panel's size.
  useEffect(() => {
    if (!active || !visible) return;
    try {
      fit.current?.fit();
    } catch {
      // Not laid out yet.
    }
  }, [active, visible]);

  // The keyboard goes into the terminal when it opens, when the panel is shown, and when the
  // panel asks (its tab clicked, a new terminal); a tab reached with the arrow keys leaves the
  // keyboard in the list of tabs. A tab that opened by itself leaves the keyboard where it is
  // until the panel asks.
  const activeNow = useRef(active);
  useLayoutEffect(() => {
    activeNow.current = active;
  });
  const quietUntil = useRef(tab.quiet === true ? { visible, focusToken } : null);
  useEffect(() => {
    const quiet = quietUntil.current;
    if (quiet && quiet.visible === visible && quiet.focusToken === focusToken) return;
    quietUntil.current = null;
    if (activeNow.current && visible) term.current?.focus();
  }, [visible, focusToken]);

  const where = tab.place.kind === "thisPc" ? "this PC" : tab.title;
  // An AI tool's sign-in tab is named by what it runs: "Sign in · Codex".
  const signIn = tab.place.kind === "aiTool";
  return (
    <div className="terminal-view">
      {status.kind === "failed" && (
        <ErrorState
          compact
          urgent
          title={signIn ? `${tab.title} could not open` : `The terminal on ${where} could not open`}
          message={status.message}
          onRetry={() => {
            setStatus({ kind: "opening" });
            setAttempt((n) => n + 1);
          }}
        />
      )}
      <div
        ref={box}
        className="terminal-view__screen"
        aria-label={signIn ? tab.title : `Terminal on ${where}`}
        role="group"
        hidden={status.kind === "failed"}
      />
    </div>
  );
}
