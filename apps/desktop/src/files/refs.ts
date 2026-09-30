/**
 * How the owner's files are named in the app (Phase 21, ADR-093 §24): a top folder Plenipo knows
 * (`project:<ID>` or `copy:<ID>`, never with a `/`) and a path inside it, with `/`.
 */

/** A file's key, and the editor page's place ID: `<root>/<path>`. */
export function fileKey(root: string, path: string): string {
  return `${root}/${path}`;
}

/** A file key read back (`null` if it is not one). */
export function parseFileKey(key: string): { root: string; path: string } | null {
  const at = key.indexOf("/");
  if (at <= 0) return null;
  const root = key.slice(0, at);
  const path = key.slice(at + 1);
  if (!/^(project|copy):[A-Za-z0-9-]+$/.test(root) || path.length === 0) return null;
  return { root, path };
}

/** The last part of a path: its file's name. */
export function nameOf(path: string): string {
  const at = path.lastIndexOf("/");
  return at < 0 ? path : path.slice(at + 1);
}

/** The folder part of a path ("" at the top). */
export function folderOf(path: string): string {
  const at = path.lastIndexOf("/");
  return at < 0 ? "" : path.slice(0, at);
}

/** "12 KB", "1.2 MB". */
export function sizeWords(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.ceil(bytes / 1024)} KB`;
  return `${bytes} bytes`;
}

/** A file being dragged from the Files panel, onto an objective (ADR-093 §19). */
export interface DraggedFile {
  root: string;
  path: string;
  name: string;
}

/** The DOM event an objective's box hears when a file from the Files panel is dropped on it. */
export const ATTACH_EVENT = "plenipo:attach-files";
/** Marks an element files can be dropped on. */
export const DROP_ATTRIBUTE = "data-drop-files";
