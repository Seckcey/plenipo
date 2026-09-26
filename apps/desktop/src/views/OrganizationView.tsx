/**
 * The organization is data-driven: departments, managers, and project coordinators are
 * loaded from the Workforce engine (Phase 5). Nothing is hard-coded here.
 */
export function OrganizationView() {
  return (
    <section className="view" aria-labelledby="org-title">
      <h1 id="org-title">Organization</h1>
      <div className="empty">
        <h2>No departments configured yet</h2>
        <p>
          Departments, managers, and project coordinators will appear here once the Workforce engine
          is available. Until then, use <strong>Workers</strong> to give an objective directly to a
          Claude Code or Codex worker, and <strong>Runtimes</strong> to check their sign-in.
        </p>
      </div>
    </section>
  );
}
