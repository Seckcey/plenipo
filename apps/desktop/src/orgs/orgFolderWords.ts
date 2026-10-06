import type { OrgFolderInfo } from "@plenipo/types";

/** The last part of a folder's path: its own name. */
export function folderName(path: string): string {
  const parts = path.split(/[\\/]+/).filter((p) => p !== "");
  return parts[parts.length - 1] ?? path;
}

/**
 * Whether to tell the owner to keep the organization folder on this computer (the owner's
 * decision, 2026-10-05; ADR-205 §3): OneDrive syncs it and it isn't set to "Always keep on this
 * device", or another service syncs it and Plenipo can't tell.
 */
export function needsKeepAlert(info: OrgFolderInfo | null): boolean {
  if (!info?.path || !info.syncedBy) return false;
  return info.syncedBy === "other" || info.keptOnThisDevice !== true;
}
