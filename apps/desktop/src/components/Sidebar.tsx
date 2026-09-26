import { VIEWS, type ViewId } from "./views";

export function Sidebar({
  current,
  onNavigate,
  activeCount,
  workingCount,
  approvalCount = 0,
}: {
  current: ViewId;
  onNavigate: (view: ViewId) => void;
  activeCount: number;
  /** Agent turns in progress. */
  workingCount: number;
  /** Requests waiting for the owner's approval. */
  approvalCount?: number;
}) {
  return (
    <nav className="sidebar" aria-label="Main">
      <ul>
        {VIEWS.map((view) => (
          <li key={view.id}>
            <button
              type="button"
              className="sidebar__item"
              aria-current={current === view.id ? "page" : undefined}
              onClick={() => onNavigate(view.id)}
            >
              <span>{view.label}</span>
              {view.id === "runtimes" && activeCount > 0 && (
                <span className="badge badge--running" aria-label={`${activeCount} active`}>
                  {activeCount}
                </span>
              )}
              {view.id === "approvals" && approvalCount > 0 && (
                <span
                  className="badge badge--task-awaitingApproval"
                  aria-label={`${approvalCount} waiting for you`}
                >
                  {approvalCount}
                </span>
              )}
              {view.id === "workers" && workingCount > 0 && (
                <span className="badge badge--running" aria-label={`${workingCount} working`}>
                  {workingCount}
                </span>
              )}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
