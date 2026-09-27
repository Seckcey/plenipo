import { useControl } from "../control/useControl";

/**
 * The small window above all others while a worker uses the mouse and keyboard (Phase 10):
 * who, and Stop and Take over, always within reach.
 */
export function IndicatorView() {
  const control = useControl();
  const desktop = control.status?.sessions.find(
    (s) => s.kind === "desktop" && s.state === "active",
  );
  return (
    <div className="indicator" role="alert" aria-label="A worker is using your mouse and keyboard">
      <span className="control__dot" aria-hidden="true" />
      <div className="indicator__words">
        <strong>
          {desktop
            ? `${desktop.worker} is using your mouse and keyboard`
            : "No worker is using your mouse and keyboard"}
        </strong>
        <span className="muted">Moving the mouse takes control back.</span>
      </div>
      {desktop && (
        <button
          type="button"
          className="button button--small button--quiet"
          disabled={control.pending}
          onClick={() => void control.takeOver(desktop.id)}
        >
          Take over
        </button>
      )}
      <button
        type="button"
        className="button button--stop"
        disabled={control.pending}
        onClick={() => void control.stopAll()}
      >
        Stop
      </button>
    </div>
  );
}
