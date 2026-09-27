import type { Control } from "../control/useControl";
import { sessionWords } from "../control/words";

/**
 * The sign on every page whenever a worker uses Plenipo's browser or the mouse and keyboard
 * (Phase 10): who, where, and what it did last, with Take over for each and Stop all. After a
 * stop, it says control is stopped until you allow it again.
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
      aria-label="Browser and desktop control"
    >
      <div className="control__sessions">
        {status.stopped && (
          <div className="control__session">
            <strong>Browser and desktop control is stopped.</strong>
            <div className="muted">
              No worker can use the browser, the mouse, or the keyboard until you allow it again.
            </div>
          </div>
        )}
        {[...active, ...others].map((s) => {
          const words = sessionWords(s);
          return (
            <div className="control__session" key={s.id}>
              <strong>
                {s.state === "active" && <span className="control__dot" aria-hidden="true" />}
                {words.title}
              </strong>
              {words.detail && <div className="muted control__detail">{words.detail}</div>}
              {s.state === "active" && (
                <button
                  type="button"
                  className="button button--small button--quiet"
                  disabled={pending}
                  onClick={() => void control.takeOver(s.id)}
                >
                  Take over
                </button>
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
          <button
            type="button"
            className="button button--stop"
            disabled={pending}
            onClick={() => void control.stopAll()}
          >
            Stop all
          </button>
        )}
        {status.stopped && (
          <button
            type="button"
            className="button button--small"
            disabled={pending}
            onClick={() => void control.allow()}
          >
            Allow again
          </button>
        )}
      </div>
    </div>
  );
}
