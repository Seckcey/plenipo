/**
 * What each organization remembers on its own in a window (Phase 21, ADR-094): the page you
 * were on, the Organization map's view, the selected rows, the open files, and the panels. The
 * first organization keeps the names it always had; another adds its ID.
 */
import { setStorageScope } from "@plenipo/ui";

export const PER_ORGANIZATION = new Set([
  "plenipo.place",
  "plenipo.layout",
  "plenipo.editor.open",
  "plenipo.orgCamera",
  "plenipo.orgCollapsed",
  "plenipo.orgFilters",
  "plenipo.orgSelected",
  "plenipo.orgWhere",
  "plenipo.selectedTask",
  "plenipo.selectedSession",
  "plenipo.selectedExecution",
]);

/** The name `key` is remembered under for organization `id` (`first`: the first one's). */
export function keyFor(key: string, id: string, first: boolean): string {
  return first || !PER_ORGANIZATION.has(key) ? key : `${key}@${id}`;
}

/** From now on this window remembers organization `id`'s things. */
export function rememberFor(id: string, first: boolean): void {
  setStorageScope(first ? null : (key) => keyFor(key, id, first));
}
