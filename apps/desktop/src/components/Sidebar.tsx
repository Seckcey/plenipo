import { VIEWS, type ViewId } from "./views";

export function Sidebar({
  current,
  onNavigate,
  activeCount,
}: {
  current: ViewId;
  onNavigate: (view: ViewId) => void;
  activeCount: number;
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
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
