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
import type { OwnerTab } from "./panel";
import { endedLine, fromBase64, pieces, screenReaderWanted } from "./words";

type Status =
  | { kind: "opening" }
  | { kind: "open"; info: TerminalInfo }
  | { kind: "ended"; why: string }
  | { kind: "failed"; message: string };

/**
 * One of the owner's terminals (ADR-031): xterm.js on screen, a shell on this PC or on a server
 * behind it. Typing goes to the shell as it is typed (a large paste in pieces); nothing is
 * recorded. Its colors come from the terminal tokens, and follow the theme.
 */
export function OwnerTerminal({
  tab,
  active,
  visible,
  theme,
  focusToken,
  onLeave,
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
}) {
  const box = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const themeNow = useRef(theme);
  const [status, setStatus] = useState<Status>({ kind: "opening" });
  const [attempt, setAttempt] = useState(0);
  const leave = useRef(onLeave);
  useLayoutEffect(() => {
    leave.current = onLeave;
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
      }
    })
      .then((info) => {
        if (disposed) {
          void closeTerminal(info.id).catch(() => undefined);
          return;
        }
        id = info.id;
        if (!ended) setStatus({ kind: "open", info });
        // The panel may have changed size while a server's terminal was connecting.
        if (!ended && (xterm.cols !== opened.cols || xterm.rows !== opened.rows)) {
          void resizeTerminal(info.id, xterm.cols, xterm.rows).catch(() => undefined);
        }
        for (const data of typedEarly) send(info.id, data);
        typedEarly = [];
      })
      .catch((reason: unknown) => {
        if (!disposed) setStatus({ kind: "failed", message: toCommandError(reason).message });
      });
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(refit);
    observer?.observe(el);
    return () => {
      disposed = true;
      observer?.disconnect();
      typing.dispose();
      sizing.dispose();
      if (id && !ended) void closeTerminal(id).catch(() => undefined);
      xterm.dispose();
      term.current = null;
      fit.current = null;
    };
  }, [tab.place, attempt]);

  useEffect(() => {
    themeNow.current = theme;
    if (term.current) term.current.options.theme = terminalTheme(theme);
  }, [theme]);

  useEffect(() => {
    if (!active || !visible) return;
    try {
      fit.current?.fit();
    } catch {
      // Not laid out yet.
    }
    term.current?.focus();
  }, [active, visible, focusToken]);

  const where = tab.place.kind === "thisPc" ? "this PC" : tab.title;
  return (
    <div className="terminal-view">
      {status.kind === "failed" && (
        <ErrorState
          compact
          urgent
          title={`The terminal on ${where} could not open`}
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
        aria-label={`Terminal on ${where}`}
        role="group"
        hidden={status.kind === "failed"}
      />
    </div>
  );
}
