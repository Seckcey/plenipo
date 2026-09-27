import type { AppInfo } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { LedgerPanel } from "../components/LedgerPanel";
import { LiaisonPanel } from "../components/LiaisonPanel";
import { formatTime } from "../runtime/format";
import { isActive } from "../runtime/store";
import { useRuntime } from "../runtime/useRuntime";

export function DiagnosticsView({
  info,
  onTaskCreated,
  onOpenGallery,
}: {
  info: AppInfo | null;
  onTaskCreated: (taskId: string) => void;
  /** The Gallery of every building block (Phase 12A). */
  onOpenGallery?: () => void;
}) {
  const { state } = useRuntime();
  const active = Object.values(state.executions).filter(isActive).length;
  return (
    <section className="view" aria-labelledby="diagnostics-title">
      <h1 id="diagnostics-title">Diagnostics</h1>

      <dl className="facts" aria-label="App details">
        <div>
          <dt>Core</dt>
          <dd className={info ? "ok" : undefined}>{info ? "Connected" : "Connecting…"}</dd>
        </div>
        <div>
          <dt>Version</dt>
          <dd>{info ? `${info.version} (${info.buildProfile})` : "—"}</dd>
        </div>
        <div>
          <dt>Platform</dt>
          <dd>{info ? `${info.os} / ${info.arch}` : "—"}</dd>
        </div>
        <div>
          <dt>Active processes</dt>
          <dd>{active}</dd>
        </div>
      </dl>

      {onOpenGallery && (
        <p className="diagnostics__gallery">
          <Button size="sm" icon="gallery" onClick={onOpenGallery}>
            Open the gallery
          </Button>
          <span className="muted">
            Every building block of Plenipo's screens, in light and dark, for checking the look.
          </span>
        </p>
      )}

      <LedgerPanel onTaskCreated={onTaskCreated} />

      <LiaisonPanel />

      <h2>Program notices</h2>
      {state.notices.length === 0 ? (
        <p className="muted">None.</p>
      ) : (
        <ul className="notices">
          {state.notices.map((n) => (
            <li key={n}>{n}</li>
          ))}
        </ul>
      )}

      <h2>Recent program events</h2>
      {state.eventLog.length === 0 ? (
        <p className="muted">No events since Plenipo started.</p>
      ) : (
        <ol className="event-log" aria-label="Recent program events">
          {state.eventLog.map(({ at, event }, i) => (
            <li key={`${at}-${i}`}>
              <time>{formatTime(at)}</time>{" "}
              {event.kind === "lifecycle"
                ? `lifecycle ${event.record.label} → ${event.record.state}`
                : `output ${event.lines.length} line(s)`}
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
