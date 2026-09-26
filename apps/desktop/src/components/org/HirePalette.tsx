import type { PointerEvent as ReactPointerEvent } from "react";
import type { OrgSnapshot, RoleInfo } from "@plenipo/types";

import { hireableRoles } from "../../org/rules";
import { Glyph } from "./Glyph";
import type { DragPayload } from "./TopologyCanvas";

/**
 * Roles to hire from, like a device palette: drag a card onto a position to hire into its team
 * (or onto You / the organization for a superintendent), or click a card to choose in a form.
 * Folds to a rail of role icons to give the canvas more room.
 */
export function HirePalette({
  snapshot,
  open,
  onToggle,
  onStartDrag,
  onPick,
  onNewDepartment,
  onNewProject,
  onNewRole,
}: {
  snapshot: OrgSnapshot;
  open: boolean;
  onToggle: () => void;
  onStartDrag: (payload: DragPayload, event: ReactPointerEvent) => void;
  onPick: (roleId: string) => void;
  onNewDepartment: () => void;
  onNewProject: () => void;
  onNewRole: () => void;
}) {
  const roles = hireableRoles(snapshot);
  const leadership = roles.filter((r) => r.kind === "superintendent");
  const members = roles.filter((r) => r.kind === "worker");
  const card = (r: RoleInfo) => (
    <li key={r.id}>
      <button
        type="button"
        className="palette__role"
        aria-label={`Hire ${r.name}`}
        title={open ? r.description : `${r.name} — ${r.description}`}
        onPointerDown={(e) => onStartDrag({ kind: "role", roleId: r.id }, e)}
        onClick={() => onPick(r.id)}
      >
        <span className="palette__glyph">
          <Glyph name={r.glyph} size={16} />
        </span>
        {open && (
          <>
            <span className="palette__name">{r.name}</span>
            <span className="palette__staffing">
              {r.staffing === "persistent" ? "Persistent" : "On demand"}
            </span>
          </>
        )}
      </button>
    </li>
  );
  const group = (label: string, list: RoleInfo[]) =>
    list.length > 0 && (
      <>
        {open ? <h3>{label}</h3> : <hr className="palette__rule" />}
        <ul className="palette__roles">{list.map(card)}</ul>
      </>
    );
  return (
    <aside
      className={`palette${open ? "" : " palette--rail"}`}
      aria-label="Hire palette"
      data-canvas-scroll
    >
      <div className="palette__header">
        {open && <h2>Hire</h2>}
        <button
          type="button"
          className="palette__fold"
          aria-expanded={open}
          aria-label={open ? "Fold the hire palette" : "Unfold the hire palette"}
          title={open ? "Fold" : "Hire"}
          onClick={onToggle}
        >
          <span aria-hidden="true">{open ? "‹" : "›"}</span>
        </button>
      </div>
      {open && <p className="palette__help">Drag a role onto a position, or click one.</p>}
      {group("Leadership", leadership)}
      {group("Team members", members)}
      {open && (
        <div className="palette__more">
          <button
            type="button"
            className="button button--small button--quiet"
            onClick={onNewDepartment}
          >
            + Department
          </button>
          <button
            type="button"
            className="button button--small button--quiet"
            onClick={onNewProject}
          >
            + Project
          </button>
          <button type="button" className="button button--small button--quiet" onClick={onNewRole}>
            + Role
          </button>
        </div>
      )}
    </aside>
  );
}
