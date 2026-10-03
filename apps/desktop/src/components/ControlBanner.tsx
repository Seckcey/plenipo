import { Button } from "@plenipo/ui";

import type { Control } from "../control/useControl";
import { sessionWords, takeOverLabel } from "../control/words";

/**
 * The sign on every page whenever a worker uses Plenipo's browser or the mouse and keyboard
 * (Phase 10), or is connected to a server (Phase 11, production in red): who, where, and what
 * it did last, with Take over (Disconnect, for servers) for each and Stop all. After a stop, it
 * says work is stopped until you allow it again.
 */
export function ControlBanner({ control }: { control: Control }) {
  const { status, pending, error } = control;
  if (!status) return null;
  const active = status.sessions.filter((s) => s.state === "active");
  const others = status.sessions.filter((s) => s.state !== "active");
  if (!status.stopped && status.sessions.length === 0) return null;
  return (
    <div
      className={`banner banner--control${active.length > 0 ? " banner--control-active" : ""}`}
      role="alert"
      aria-label="Browser, desktop, and server work"
    >
      <div className="control__sessions">
        {status.stopped && (
          <div className="control__session">
            <strong>All work is stopped.</strong>
            <div className="muted">
              Every task stopped, in every organization. Nothing new starts, and no worker can use
              the browser, the mouse, the keyboard, or a server, until you press Allow again.
            </div>
          </div>
        )}
        {[...active, ...others].map((s) => {
          const words = sessionWords(s);
          return (
            <div
              className={`control__session${s.production ? " control__session--production" : ""}`}
              key={s.id}
            >
              <strong>
                {s.state === "active" && <span className="control__dot" aria-hidden="true" />}
                {words.title}
              </strong>
              {s.production && <span className="env env--production">PRODUCTION</span>}
              {words.detail && <div className="muted control__detail">{words.detail}</div>}
              {s.state === "active" && (
                <Button
                  variant="quiet"
                  size="sm"
                  disabled={pending}
                  onClick={() => void control.takeOver(s.id)}
                >
                  {takeOverLabel(s)}
                </Button>
              )}
              {s.state === "takenOver" && (
                <Button variant="quiet" size="sm" onClick={() => control.dismiss(s.id)}>
                  Dismiss
                </Button>
              )}
            </div>
          );
        })}
        {error && (
          <div className="form-error" role="alert">
            {error}
          </div>
        )}
      </div>
      <div className="control__actions">
        {active.length > 0 && (
          <Button
            variant="danger"
            className="stop-all"
            disabled={pending}
            onClick={() => void control.stopAll()}
          >
            Stop all
          </Button>
        )}
        {status.stopped && (
          <Button
            variant="primary"
            size="sm"
            disabled={pending}
            onClick={() => void control.allow()}
          >
            Allow again
          </Button>
        )}
      </div>
    </div>
  );
}
