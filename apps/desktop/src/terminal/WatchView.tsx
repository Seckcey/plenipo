import { useState } from "react";
import { Banner, Button, LogView, StatusDot, StatusPill } from "@plenipo/ui";

import { stopServerCommand, takeOverControl, toCommandError } from "../api/commands";
import { runningCommand, watchLines, watchTitle, type WatchTab } from "./watch";

/**
 * A worker's watch tab (ADR-031 §5): each command Guard let through on this server and its
 * output as it arrives, read-only. **Stop** stops the command running now (TERM, then KILL);
 * **Disconnect** ends the worker's server work. Both are recorded.
 */
export function WatchView({ tab }: { tab: WatchTab }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const running = runningCommand(tab);
  const act = (work: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    work()
      .catch((reason: unknown) => setError(toCommandError(reason).message))
      .finally(() => setBusy(false));
  };
  const production = tab.environment === "production";
  return (
    <div className="terminal-watch">
      <div className="terminal-watch__bar">
        <StatusDot
          status={tab.connected ? "ok" : "offline"}
          label={tab.connected ? "Connected" : "Disconnected"}
        />
        <span className="terminal-watch__who">
          {watchTitle(tab)}
          {tab.address ? ` · ${tab.address}` : ""}
        </span>
        {production && <StatusPill status="error" label="PRODUCTION" />}
        <span className="terminal-watch__note">Read-only: you see what Guard let through.</span>
        <Button
          size="sm"
          icon="stop"
          disabled={busy || !running || running.state === "stopping"}
          title={running ? `Stop ${running.command}` : "Nothing is running"}
          onClick={() => running && act(() => stopServerCommand(running.commandId))}
        >
          Stop
        </Button>
        <Button
          size="sm"
          variant="danger"
          disabled={busy || !tab.connected}
          onClick={() => act(() => takeOverControl(`server:${tab.grantId}`))}
        >
          Disconnect
        </Button>
      </div>
      {error && (
        <Banner tone="error" role="alert" title="That did not work">
          {error}
        </Banner>
      )}
      <LogView
        label={`${watchTitle(tab)}: commands and output`}
        className="terminal-watch__log"
        lines={watchLines(tab)}
        empty="Connected. Commands appear here as the worker runs them."
      />
    </div>
  );
}
