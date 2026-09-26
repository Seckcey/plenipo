import type { PermissionsSnapshot, UnitLimit } from "@plenipo/types";

import { assignPermissions } from "../../api/commands";
import { usePermissions } from "../../guard/usePermissions";
import { useRun } from "../../guard/useRun";
import { Refusal } from "../models/shared";
import { PermissionSets } from "./PermissionSets";
import { ApprovalWindow, BlockedFiles, CommandLists, SensitiveActions } from "./RuleLists";
import { SecretList } from "./SecretList";

type Apply = (s: PermissionsSnapshot) => void;

/**
 * Settings → Permissions: what workers may do on this computer. Roles get permission sets;
 * projects and departments can narrow them; rules protect files, programs, and sensitive
 * actions; secrets live in the operating system's protected storage.
 */
export function PermissionSettings() {
  const permissions = usePermissions();
  const s = permissions.snapshot;
  if (!s) {
    return (
      <p
        className={permissions.error ? "form-error" : "muted"}
        role={permissions.error ? "alert" : undefined}
      >
        {permissions.error ?? "Loading the permission settings…"}
      </p>
    );
  }
  return (
    <div className="permissions">
      <p className="muted">
        Workers of your organization use your computer only through Plenipo&apos;s own tools —
        files, programs, and git inside their project&apos;s folder — and only as far as these
        settings allow. Every use is checked and recorded; changes apply to a worker&apos;s next
        request.{" "}
        <span className={`pill ${s.tools.running ? "pill--ok" : "pill--bad"}`}>
          {s.tools.running ? "Tools ready" : "Tools not running"}
        </span>
      </p>
      {s.notices.length > 0 && (
        <ul className="notices">
          {s.notices.map((n) => (
            <li key={n}>{n}</li>
          ))}
        </ul>
      )}
      <WhoHasWhat snapshot={s} onApply={permissions.apply} />
      <PermissionSets snapshot={s} onApply={permissions.apply} />
      <CommandLists snapshot={s} onApply={permissions.apply} />
      <BlockedFiles snapshot={s} onApply={permissions.apply} />
      <SensitiveActions snapshot={s} onApply={permissions.apply} />
      <ApprovalWindow snapshot={s} onApply={permissions.apply} />
      <SecretList snapshot={s} onApply={permissions.apply} />
    </div>
  );
}

function setName(s: PermissionsSnapshot, id: string | undefined): string {
  return s.settings.sets.find((x) => x.id === id)?.name ?? `"${id}" (not a set)`;
}

/** Roles (grant), departments (limit), and projects (limit and folder). */
function WhoHasWhat({ snapshot, onApply }: { snapshot: PermissionsSnapshot; onApply: Apply }) {
  const { pending, error, run } = useRun(onApply);
  const sets = snapshot.settings.sets;
  const assign = (target: "role" | "department", id: string, value: string) =>
    void run(() => assignPermissions(target, id, value === "" ? null : value));
  return (
    <section aria-labelledby="who-title">
      <h3 id="who-title">Who may do what</h3>
      <table className="table">
        <thead>
          <tr>
            <th scope="col">Role</th>
            <th scope="col">Permission set</th>
          </tr>
        </thead>
        <tbody>
          {snapshot.settings.roles.map((r) => (
            <tr key={r.roleId}>
              <th scope="row">
                {r.roleName}
                <span className="table__sub">{r.fullTime ? "Full-time" : "On call"}</span>
              </th>
              <td>
                <select
                  aria-label={`${r.roleName}'s permission set`}
                  value={r.setId ?? ""}
                  disabled={pending}
                  onChange={(e) => assign("role", r.roleId, e.target.value)}
                >
                  <option value="">None (conversation only)</option>
                  {sets.map((x) => (
                    <option key={x.id} value={x.id}>
                      {x.name}
                    </option>
                  ))}
                </select>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {snapshot.settings.departments.length > 0 && (
        <table className="table">
          <thead>
            <tr>
              <th scope="col">Department</th>
              <th scope="col">Limit</th>
            </tr>
          </thead>
          <tbody>
            {snapshot.settings.departments.map((d) => (
              <tr key={d.id}>
                <th scope="row">{d.name}</th>
                <td>
                  <select
                    aria-label={`${d.name}'s limit`}
                    value={d.setId ?? ""}
                    disabled={pending}
                    onChange={(e) => assign("department", d.id, e.target.value)}
                  >
                    <option value="">No limit</option>
                    {sets.map((x) => (
                      <option key={x.id} value={x.id}>
                        {x.name}
                      </option>
                    ))}
                  </select>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {snapshot.settings.projects.length > 0 && (
        <table className="table">
          <thead>
            <tr>
              <th scope="col">Project</th>
              <th scope="col">Folder</th>
              <th scope="col">Limit</th>
            </tr>
          </thead>
          <tbody>
            {snapshot.settings.projects.map((p: UnitLimit) => (
              <tr key={p.id}>
                <th scope="row">
                  {p.name}
                  {p.problem && <span className="table__sub form-error">{p.problem}</span>}
                </th>
                <td>
                  <span className="path">{p.folder ?? "—"}</span>
                </td>
                <td>{p.setId ? setName(snapshot, p.setId) : "No limit"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p className="muted">
        A role&apos;s set grants; a department&apos;s or project&apos;s limit only narrows it — the
        strictest answer wins. Change a project&apos;s folder and limit in its settings on the
        Organization page.
      </p>
      <Refusal error={error} />
    </section>
  );
}
