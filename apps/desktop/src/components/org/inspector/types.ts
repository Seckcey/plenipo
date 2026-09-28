import type { ReactNode } from "react";
import type { OrgSnapshot, OversightRole, PositionPatchInput } from "@plenipo/types";

import type { ArchivedKind } from "../../../api/commands";
import type { Go } from "../../views";

/** What the inspector can ask the Organization view to do. */
export interface InspectorActions {
  /** Run a change; resolves with the refusal to show, or `null` once applied. */
  run: (work: () => Promise<OrgSnapshot>) => Promise<string | null>;
  /**
   * Run a change that answers with something other than the organization (a model rule,
   * learning); the organization is reloaded. Resolves with the refusal, or `null`.
   */
  change: (work: () => Promise<unknown>) => Promise<string | null>;
  giveObjective: (positionId: string, objective: string) => Promise<string | null>;
  /** Open the Hire box under `reportsTo` (`null`: you), with `roleId` chosen when given. */
  hire: (reportsTo: string | null, roleId?: string | null) => void;
  newDepartment: (reportsTo: string | null) => void;
  newProject: (departmentId: string | null) => void;
  newRole: () => void;
  editDepartment: (id: string) => void;
  editProject: (id: string) => void;
  editRole: (id: string) => void;
  /** Add one of your own specialties to a role. */
  newSpecialty: (roleId: string) => void;
  /** Change or remove one of your own specialties. */
  editSpecialty: (specialtyId: string) => void;
  rename: () => void;
  confirm: (request: {
    title: string;
    message: ReactNode;
    confirmLabel: string;
    work: () => Promise<OrgSnapshot>;
  }) => void;
  /** Ask before deleting an archived item for good, offering to save its experienced agents. */
  deleteForGood: (kind: ArchivedKind, id: string) => void;
  openSession: (sessionId: string) => void;
  openTask: (taskId: string) => void;
  /** Opens the page of a position, department, or project (Phase 12), or a Settings section. */
  openPage?: Go | undefined;
  api: {
    fill: (id: string) => Promise<OrgSnapshot>;
    vacate: (id: string) => Promise<OrgSnapshot>;
    update: (id: string, patch: PositionPatchInput) => Promise<OrgSnapshot>;
    move: (id: string, reportsTo: string | null) => Promise<OrgSnapshot>;
    archive: (id: string) => Promise<OrgSnapshot>;
    assign: (overseer: string, target: string, role: OversightRole) => Promise<OrgSnapshot>;
    endOversight: (id: string) => Promise<OrgSnapshot>;
    archiveDepartment: (id: string) => Promise<OrgSnapshot>;
    archiveProject: (id: string) => Promise<OrgSnapshot>;
    bringBack: (kind: ArchivedKind, id: string) => Promise<OrgSnapshot>;
    saveToWorkforce: (id: string) => Promise<OrgSnapshot>;
  };
}
