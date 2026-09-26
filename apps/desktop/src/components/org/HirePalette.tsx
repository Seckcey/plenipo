import type { PointerEvent as ReactPointerEvent } from "react";
import type { OrgSnapshot } from "@plenipo/types";

import { hireableRoles } from "../../org/rules";
import { Glyph } from "./Glyph";
import type { DragPayload } from "./TopologyCanvas";

/**
 * Roles to hire from, like a device palette: drag a card onto a position to hire into its team
 * (or onto You / the organization for a superintendent), or click a card to choose in a form.
 */
export function HirePalette({
  snapshot,
  onStartDrag,
  onPick,
  onNewDepartment,
  onNewProject,
  onNewRole,
}: {
  snapshot: OrgSnapshot;
  onStartDrag: (payload: DragPayload, event: ReactPointerEvent) => void;
  onPick: (roleId: string) => void;
  onNewDepartment: () => void;
  onNewProject: () => void;
  onNewRole: () => void;
}) {
  const roles = hireableRoles(snapshot);
  const leadership = roles.filter((r) => r.kind === "superintendent");
  const members = roles.filter((r) => r.kind === "worker");
  const group = (label: string, list: typeof roles) =>
    list.length > 0 && (
      <>
        <h3>{label}</h3>
        <ul className="palette__roles">
          {list.map((r) => (
            <li key={r.id}>
              <button
                type="button"
                className="palette__role"
                aria-label={`Hire ${r.name}`}
                title={r.description}
                onPointerDown={(e) => onStartDrag({ kind: "role", roleId: r.id }, e)}
                onClick={() => onPick(r.id)}
              >
                <span className="palette__glyph">
                  <Glyph name={r.glyph} size={16} />
                </span>
                <span className="palette__name">{r.name}</span>
                <span className="palette__staffing">
                  {r.staffing === "persistent" ? "Persistent" : "On demand"}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </>
    );
  return (
    <aside className="palette" aria-label="Hire palette" data-canvas-scroll>
      <h2>Hire</h2>
      <p className="palette__help">Drag a role onto a position, or click one.</p>
      {group("Leadership", leadership)}
      {group("Team members", members)}
      <div className="palette__more">
        <button
          type="button"
          className="button button--small button--quiet"
          onClick={onNewDepartment}
        >
          + Department
        </button>
        <button type="button" className="button button--small button--quiet" onClick={onNewProject}>
          + Project
        </button>
        <button type="button" className="button button--small button--quiet" onClick={onNewRole}>
          + Role
        </button>
      </div>
    </aside>
  );
}
