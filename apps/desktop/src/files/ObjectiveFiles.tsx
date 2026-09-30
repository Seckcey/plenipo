import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import type { ObjectiveFile } from "@plenipo/types";
import { IconButton, Icon } from "@plenipo/ui";

import { ATTACH_EVENT, DROP_ATTRIBUTE, sizeWords } from "./refs";

/** A file dropped on an objective's box (from the Files panel, or from File Explorer). */
export interface AttachedFile {
  /** What Plenipo is told about it. */
  file: ObjectiveFile;
  name: string;
  size?: number | null | undefined;
  folder?: boolean | undefined;
}

/** The most files on one objective (Plenipo checks it too). */
export const MAX_OBJECTIVE_FILES = 20;

function sameFile(a: ObjectiveFile, b: ObjectiveFile): boolean {
  if (a.kind === "file" && b.kind === "file") return a.root === b.root && a.path === b.path;
  if (a.kind === "dropped" && b.kind === "dropped") return a.drop === b.drop && a.index === b.index;
  return false;
}

/**
 * The files the owner puts on an objective (Phase 21, ADR-093 §19): dropped on the box that
 * `dropRef` marks, from the Files panel or from File Explorer. Folders are not taken (their files
 * are).
 */
export function useObjectiveFiles(allowed: boolean) {
  const [files, setFiles] = useState<AttachedFile[]>([]);
  const [note, setNote] = useState<string | null>(null);
  const [box, setBox] = useState<HTMLElement | null>(null);
  const add = useCallback((more: readonly AttachedFile[]) => {
    const folders = more.filter((f) => f.folder);
    const usable = more.filter((f) => !f.folder);
    setNote(
      folders.length > 0
        ? `${folders.map((f) => f.name).join(", ")}: a folder. Put its files on the objective instead.`
        : null,
    );
    setFiles((all) => {
      const next = [...all];
      for (const f of usable) if (!next.some((x) => sameFile(x.file, f.file))) next.push(f);
      if (next.length > MAX_OBJECTIVE_FILES) {
        setNote(`An objective can have up to ${MAX_OBJECTIVE_FILES} files.`);
        return next.slice(0, MAX_OBJECTIVE_FILES);
      }
      return next;
    });
  }, []);
  useEffect(() => {
    if (!box) return;
    const onAttach = (e: Event) => {
      const detail = (
        e as CustomEvent<AttachedFile[] | { root: string; path: string; name: string }[]>
      ).detail;
      if (!allowed) {
        setNote("Files can go only on an objective for a project: its workers need a folder.");
        return;
      }
      add(
        detail.map((d) =>
          "file" in d ? d : { file: { kind: "file", root: d.root, path: d.path }, name: d.name },
        ),
      );
    };
    box.addEventListener(ATTACH_EVENT, onAttach);
    return () => box.removeEventListener(ATTACH_EVENT, onAttach);
  }, [box, add, allowed]);
  return {
    files,
    note,
    dropRef: setBox,
    dropProps: { [DROP_ATTRIBUTE]: "" } as Record<string, string>,
    remove: (i: number) => setFiles((all) => all.filter((_, j) => j !== i)),
    clear: () => {
      setFiles([]);
      setNote(null);
    },
    toSend: () => files.map((f) => f.file),
  };
}

/** The files on the objective, each with Remove, and how to add more. */
export function ObjectiveFilesList({
  state,
  hint,
}: {
  state: ReturnType<typeof useObjectiveFiles>;
  hint?: ReactNode;
}) {
  return (
    <div className="objective-files">
      {state.files.length > 0 ? (
        <ul className="objective-files__list" aria-label="Files for this objective">
          {state.files.map((f, i) => (
            <li key={`${f.name}-${i}`}>
              <Icon name="file" size={12} />
              <span>{f.name}</span>
              {typeof f.size === "number" && <span className="muted">{sizeWords(f.size)}</span>}
              <IconButton icon="close" label={`Remove ${f.name}`} onClick={() => state.remove(i)} />
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted objective-files__hint">
          {hint ??
            "Drop files here, from Files or from File Explorer, to put them on the objective."}
        </p>
      )}
      {state.note && (
        <p className="hint" role="status">
          {state.note}
        </p>
      )}
    </div>
  );
}

/** Where on the page a File Explorer drop landed: the objective box there hears of it. */
export function useFileExplorerDrops(
  subscribe: (handler: (d: import("@plenipo/types").DroppedFiles) => void) => Promise<() => void>,
) {
  const ref = useRef(subscribe);
  useEffect(() => {
    let stop: (() => void) | null = null;
    let live = true;
    ref
      .current((dropped) => {
        const target = document
          .elementFromPoint(dropped.x, dropped.y)
          ?.closest(`[${DROP_ATTRIBUTE}]`);
        if (!target) return;
        const files: AttachedFile[] = dropped.files.map((f, index) => ({
          file: { kind: "dropped", drop: dropped.drop, index },
          name: f.name,
          size: f.size,
          folder: f.folder,
        }));
        target.dispatchEvent(new CustomEvent(ATTACH_EVENT, { detail: files }));
      })
      .then((s) => (live ? (stop = s) : s()))
      .catch(() => undefined);
    return () => {
      live = false;
      stop?.();
    };
  }, []);
}
