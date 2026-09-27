import { useState } from "react";
import type { TerminalSettings as Settings, TerminalShell } from "@plenipo/types";
import { ErrorState, LoadingState, StatusPill } from "@plenipo/ui";

import { getTerminalSettings, setTerminalShell, toCommandError } from "../api/commands";
import { useLive } from "../pages/useLive";

/**
 * Settings → Terminal (ADR-031, the terminal panel): the shell a new terminal on this PC starts —
 * a choice, never a path; Plenipo finds each shell itself. Terminals on servers need the
 * "Remote computers (SSH)" switch.
 */
export function TerminalSettings() {
  const live = useLive<Settings>(
    "terminal",
    () => getTerminalSettings(),
    (e) => e.eventType.startsWith("terminal.") || e.eventType === "org.settings_changed",
  );
  const [pending, setPending] = useState<TerminalShell | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (live.status === "loading") return <LoadingState label="Loading the terminal settings" />;
  if (live.status === "error" || !live.value) {
    return (
      <ErrorState
        title="Couldn't load the terminal settings"
        message={live.error}
        onRetry={live.reload}
      />
    );
  }
  const s = live.value;
  const current = pending ?? s.shell;

  const choose = async (shell: TerminalShell) => {
    setPending(shell);
    setError(null);
    try {
      await setTerminalShell(shell);
      live.reload();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setPending(null);
    }
  };

  return (
    <div className="settings-terminal">
      <section aria-labelledby="terminal-shell">
        <h3 id="terminal-shell">The shell on this PC</h3>
        {s.otherShell && (
          <p className="muted">
            This computer is not running Windows, so the terminal uses {s.otherShell}. The choice
            below applies on Windows.
          </p>
        )}
        <fieldset className="fieldset" disabled={pending !== null}>
          <legend className="visually-hidden">The shell a new terminal on this PC starts</legend>
          {s.shells.map((option) => (
            <label key={option.shell} className="check">
              <input
                type="radio"
                name="terminal-shell"
                checked={current === option.shell}
                disabled={!option.installed && current !== option.shell}
                onChange={() => void choose(option.shell)}
              />
              <span>
                {option.label}
                {!option.installed && <span className="muted"> — not on this PC</span>}
              </span>
            </label>
          ))}
        </fieldset>
        <p className="muted">
          It starts in your home folder, as your own Windows user — never as administrator. Changing
          it applies to the next terminal you open.
        </p>
        {error && (
          <p className="status status--error" role="alert">
            Couldn't keep your choice: {error}
          </p>
        )}
      </section>
      <section aria-labelledby="terminal-servers">
        <h3 id="terminal-servers">Terminals on servers</h3>
        <p>
          <StatusPill
            status={s.serversSwitchedOn ? "ok" : "offline"}
            label={s.serversSwitchedOn ? "On" : "Off"}
          />{" "}
          {s.serversSwitchedOn
            ? "You can open a terminal on the servers in Settings → Servers."
            : "Turn on Settings → Switches → Remote computers (SSH) to open terminals on your servers."}
        </p>
      </section>
      <section aria-labelledby="terminal-open">
        <h3 id="terminal-open">Open now</h3>
        <p className="muted">
          {s.open.length === 0
            ? "No terminals are open."
            : `${s.open.length} ${s.open.length === 1 ? "terminal is" : "terminals are"} open: ${s.open
                .map((t) => t.title)
                .join(", ")}.`}{" "}
          Open and close them from the Terminal button at the top, or with Ctrl+`. What you type is
          never recorded.
        </p>
      </section>
    </div>
  );
}
