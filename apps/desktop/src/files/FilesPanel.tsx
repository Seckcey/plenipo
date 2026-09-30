import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type { ChangingFile, FileRoot, FileRoots, FolderListing } from "@plenipo/types";
import { Banner, Button, EmptyState, IconButton, Icon, StatusDot, cx } from "@plenipo/ui";

import {
  getChangingFiles,
  getFileRoots,
  listFolder,
  openFileOutside,
  showInFolder,
  toCommandError,
} from "../api/commands";
import { subscribeLedgerEvents, subscribeWatch } from "../api/events";
import type { Go } from "../components/views";
import { usePanelWindow, useWorkspaceIfAny } from "../workspace/context";
import { insideWindow } from "../workspace/popout";
import { ATTACH_EVENT, DROP_ATTRIBUTE, fileKey, nameOf, sizeWords, type DraggedFile } from "./refs";

/** Events after which the folders, or who writes in them, may have changed. */
function changesTheFolders(type: string): boolean {
  return (
    type.startsWith("guard.grant_") ||
    type.startsWith("workspace.") ||
    type.startsWith("org.project_") ||
    type === "file.saved"
  );
}

type Listed = FolderListing | { error: string };

/** One row of the tree. */
interface Row {
  key: string;
  level: number;
  kind: "project" | "root" | "copies" | "folder" | "file";
  label: string;
  root?: FileRoot;
  /** The top folder and the path inside it (folders and files). */
  rootId?: string;
  path?: string;
  expanded?: boolean | undefined;
  size?: number | null;
  blocked?: boolean;
  changing?: ChangingFile;
  writer?: string | undefined;
  note?: string | undefined;
}

const PROJECT = "p:";
const COPIES = "c:";

/** The dropped-on element under a point, in Plenipo's own window (from a pop-out too). */
function dropTargetAt(
  from: Window,
  clientX: number,
  clientY: number,
  screenX: number,
  screenY: number,
) {
  if (from === window) {
    return document.elementFromPoint(clientX, clientY)?.closest(`[${DROP_ATTRIBUTE}]`) ?? null;
  }
  if (!insideWindow(window, screenX, screenY)) return null;
  const frameX = (window.outerWidth - window.innerWidth) / 2;
  const frameY = window.outerHeight - window.innerHeight - frameX;
  return (
    document
      .elementFromPoint(screenX - window.screenX - frameX, screenY - window.screenY - frameY)
      ?.closest(`[${DROP_ATTRIBUTE}]`) ?? null
  );
}

/**
 * The Files panel (Phase 21, ADR-093): each project's folder and its working copies, as a tree.
 * Only the folders Plenipo knows. Folders load as they open. Marks say which files a worker is
 * changing now, which files workers may not touch, and which working copy a worker is writing.
 * Open a file in Plenipo (double-click or Enter), in another program, or in its folder. Drag a
 * file onto an objective to put it on the objective.
 */
export function FilesPanel({ go }: { go: Go }) {
  const ws = useWorkspaceIfAny();
  const visible = ws ? ws.shown("files") : true;
  const panelWindow = usePanelWindow();
  const [roots, setRoots] = useState<FileRoots | null>(null);
  const [rootsError, setRootsError] = useState<string | null>(null);
  const [changing, setChanging] = useState<ChangingFile[]>([]);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set());
  const [listings, setListings] = useState<ReadonlyMap<string, Listed>>(() => new Map());
  const [selected, setSelected] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const expandedRef = useRef(expanded);
  useEffect(() => {
    expandedRef.current = expanded;
  }, [expanded]);

  const load = useCallback((rootId: string, path: string) => {
    const key = fileKey(rootId, path);
    listFolder(rootId, path)
      .then((l) => setListings((all) => new Map(all).set(key, l)))
      .catch((e: unknown) =>
        setListings((all) => new Map(all).set(key, { error: toCommandError(e).message })),
      );
  }, []);

  const refresh = useCallback(() => {
    getFileRoots()
      .then((r) => {
        setRoots(r);
        setRootsError(null);
      })
      .catch((e: unknown) => setRootsError(toCommandError(e).message));
    getChangingFiles()
      .then(setChanging)
      .catch(() => undefined);
    // The open folders again (a worker may have added files).
    for (const key of expandedRef.current) {
      if (key.startsWith(PROJECT) || key.startsWith(COPIES)) continue;
      const at = key.indexOf("/");
      if (at > 0) load(key.slice(0, at), key.slice(at + 1));
    }
  }, [load]);

  // While shown: the folders now, and again when what they show may have changed.
  useEffect(() => {
    if (!visible) return;
    let live = true;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const soon = () => {
      if (timer) return;
      timer = setTimeout(() => {
        timer = null;
        if (live) refresh();
      }, 300);
    };
    const stops: (() => void)[] = [];
    void subscribeLedgerEvents((e) => {
      if (changesTheFolders(e.eventType)) soon();
    })
      .then((s) => (live ? stops.push(s) : s()))
      .catch(() => undefined);
    void subscribeWatch((u) => {
      if (u.change.root) soon();
    })
      .then((s) => (live ? stops.push(s) : s()))
      .catch(() => undefined);
    refresh();
    return () => {
      live = false;
      if (timer) clearTimeout(timer);
      for (const s of stops) s();
    };
  }, [visible, refresh]);

  const toggle = (key: string, rootId?: string, path?: string) => {
    setExpanded((all) => {
      const next = new Set(all);
      if (next.has(key)) next.delete(key);
      else {
        next.add(key);
        if (rootId !== undefined && path !== undefined && !listings.has(key)) load(rootId, path);
      }
      return next;
    });
  };

  const rows = useMemo(() => {
    const out: Row[] = [];
    if (!roots) return out;
    const changingAt = new Map(changing.map((c) => [fileKey(c.root, c.path), c]));
    const addFolder = (rootId: string, path: string, level: number) => {
      const listed = listings.get(fileKey(rootId, path));
      if (!listed) {
        out.push({
          key: `${fileKey(rootId, path)}#loading`,
          level,
          kind: "file",
          label: "Loading…",
          note: "loading",
        });
        return;
      }
      if ("error" in listed) {
        out.push({
          key: `${fileKey(rootId, path)}#error`,
          level,
          kind: "file",
          label: listed.error,
          note: "error",
        });
        return;
      }
      for (const e of listed.entries) {
        const key = fileKey(rootId, e.path);
        const open = e.folder && expanded.has(key);
        const row: Row = {
          key,
          level,
          kind: e.folder ? "folder" : "file",
          label: e.name,
          rootId,
          path: e.path,
          expanded: e.folder ? open : undefined,
          size: e.size,
          blocked: e.blocked,
        };
        const c = changingAt.get(key);
        if (c) row.changing = c;
        out.push(row);
        if (open) addFolder(rootId, e.path, level + 1);
      }
      if (listed.more > 0) {
        out.push({
          key: `${fileKey(rootId, path)}#more`,
          level,
          kind: "file",
          label: `${listed.more} more not shown`,
          note: "more",
        });
      }
    };
    const byProject = new Map<string, FileRoot[]>();
    for (const r of roots.roots)
      byProject.set(r.projectId, [...(byProject.get(r.projectId) ?? []), r]);
    for (const [projectId, list] of byProject) {
      const projectKey = `${PROJECT}${projectId}`;
      const projectOpen = expanded.has(projectKey);
      out.push({
        key: projectKey,
        level: 1,
        kind: "project",
        label: list[0]?.projectName ?? "Project",
        expanded: projectOpen,
      });
      if (!projectOpen) continue;
      const folder = list.find((r) => r.kind === "projectFolder");
      const copies = list.filter((r) => r.kind === "workingCopy");
      const addRoot = (r: FileRoot, level: number, label: string) => {
        const key = fileKey(r.id, "");
        const open = expanded.has(key);
        out.push({
          key,
          level,
          kind: "root",
          label,
          root: r,
          rootId: r.id,
          path: "",
          expanded: r.exists ? open : undefined,
          writer: r.writer?.worker,
          note: r.exists ? undefined : "The folder is not there any more",
        });
        if (open && r.exists) addFolder(r.id, "", level + 1);
      };
      if (folder) addRoot(folder, 2, "Project folder");
      if (copies.length > 0) {
        const copiesKey = `${COPIES}${projectId}`;
        const copiesOpen = expanded.has(copiesKey);
        out.push({
          key: copiesKey,
          level: 2,
          kind: "copies",
          label: `Working copies (${copies.length})`,
          expanded: copiesOpen,
        });
        if (copiesOpen) for (const c of copies) addRoot(c, 3, c.label);
      }
    }
    return out;
  }, [roots, listings, expanded, changing]);

  const selectedRow = rows.find((r) => r.key === selected) ?? null;
  const selectedFile =
    selectedRow?.kind === "file" && selectedRow.rootId && selectedRow.path ? selectedRow : null;

  const openInPlenipo = (row: Row) => {
    if (row.rootId && row.path) go({ view: "file", id: fileKey(row.rootId, row.path) });
  };
  const act = (what: () => Promise<void>) => {
    setProblem(null);
    what().catch((e: unknown) => setProblem(toCommandError(e).message));
  };
  const activate = (row: Row) => {
    if (row.kind === "file") {
      if (!row.note) openInPlenipo(row);
      return;
    }
    if (row.kind === "root" && row.root && !row.root.exists) return;
    toggle(row.key, row.rootId, row.path);
  };

  // The keyboard moves through the rows (a tree: Up, Down, Right opens, Left closes).
  const tree = useRef<HTMLDivElement>(null);
  const focusRow = (key: string) => {
    setSelected(key);
    const doc = tree.current?.ownerDocument ?? document;
    doc.getElementById(`files-row-${cssKey(key)}`)?.focus();
  };
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const at = rows.findIndex((r) => r.key === selected);
    const row = rows[at];
    const move = (to: number) => {
      const next = rows[Math.max(0, Math.min(rows.length - 1, to))];
      if (next) focusRow(next.key);
    };
    if (e.key === "ArrowDown") move(at + 1);
    else if (e.key === "ArrowUp") move(at - 1);
    else if (e.key === "Home") move(0);
    else if (e.key === "End") move(rows.length - 1);
    else if (e.key === "ArrowRight" && row?.expanded === false) activate(row);
    else if (e.key === "ArrowLeft" && row?.expanded === true) toggle(row.key);
    else if (e.key === "Enter" && row) activate(row);
    else return;
    e.preventDefault();
  };

  // Dragging a file onto an objective (pointer events, ADR-009 §12).
  const drag = useRef<{ file: DraggedFile; x: number; y: number; moved: boolean } | null>(null);
  const [dragging, setDragging] = useState<DraggedFile | null>(null);

  if (rootsError) {
    return (
      <div className="files-panel files-panel--empty">
        <Banner tone="error" role="alert" title="Plenipo could not list the folders">
          {rootsError}
        </Banner>
        <Button size="sm" onClick={refresh}>
          Try again
        </Button>
      </div>
    );
  }
  if (roots && roots.roots.length === 0) {
    return (
      <div className="files-panel files-panel--empty">
        <EmptyState pip="coding" title="No project folders yet">
          Give a project a folder on its page, and its files show here, with the working copies
          workers make.
        </EmptyState>
      </div>
    );
  }
  return (
    <section className="files-panel" aria-label="Files">
      <div className="files-panel__bar">
        <Button
          size="sm"
          variant="primary"
          disabled={!selectedFile}
          onClick={() => selectedFile && openInPlenipo(selectedFile)}
        >
          Open in Plenipo
        </Button>
        <Button
          size="sm"
          variant="quiet"
          disabled={!selectedFile}
          title="Open it with the program Windows uses for it (never a program or a script)"
          onClick={() =>
            selectedFile?.rootId &&
            selectedFile.path &&
            act(() => openFileOutside(selectedFile.rootId ?? "", selectedFile.path ?? ""))
          }
        >
          Open in another program
        </Button>
        <IconButton
          icon="projects"
          label="Show in folder"
          disabled={!selectedRow?.rootId || selectedRow.path === undefined}
          onClick={() =>
            selectedRow?.rootId &&
            act(() => showInFolder(selectedRow.rootId ?? "", selectedRow.path ?? ""))
          }
        />
        <IconButton
          icon="list"
          label="Copy path"
          disabled={!selectedRow?.rootId}
          onClick={() => {
            const base = selectedRow?.root?.path ?? rootPathOf(roots, selectedRow?.rootId);
            if (!base || !selectedRow) return;
            const full = selectedRow.path ? `${base}/${selectedRow.path}` : base;
            void panelWindow.navigator.clipboard?.writeText(full).catch(() => undefined);
          }}
        />
        <IconButton icon="refresh" label="Refresh" onClick={refresh} />
      </div>
      {problem && (
        <Banner tone="error" role="alert" title={problem} onDismiss={() => setProblem(null)} />
      )}
      {roots?.desktopInUse && (
        <Banner
          tone="pending"
          role="status"
          title="A worker is using the screen, mouse, and keyboard"
        >
          Blocked files are hidden and nothing is saved until you take over.
        </Banner>
      )}
      <div
        ref={tree}
        className={cx("files-tree", dragging && "files-tree--dragging")}
        role="tree"
        aria-label="Project folders and working copies"
        onKeyDown={onKey}
      >
        {rows.map((row) => {
          const isSelected = row.key === selected;
          return (
            <div
              key={row.key}
              id={`files-row-${cssKey(row.key)}`}
              role="treeitem"
              aria-level={row.level}
              aria-expanded={row.expanded}
              aria-selected={isSelected}
              tabIndex={isSelected || (selected === null && row === rows[0]) ? 0 : -1}
              className={cx(
                "files-tree__row",
                `files-tree__row--${row.kind}`,
                isSelected && "files-tree__row--selected",
                row.note && `files-tree__row--${row.note.split(" ")[0]}`,
              )}
              style={{
                paddingLeft: `calc(${row.level - 1} * var(--ui-space-5) + var(--ui-space-3))`,
              }}
              onClick={() => setSelected(row.key)}
              onDoubleClick={() => activate(row)}
              onPointerDown={(e) => {
                if (e.button !== 0 || row.kind !== "file" || !row.rootId || !row.path) return;
                drag.current = {
                  file: { root: row.rootId, path: row.path, name: row.label },
                  x: e.clientX,
                  y: e.clientY,
                  moved: false,
                };
                e.currentTarget.setPointerCapture?.(e.pointerId);
              }}
              onPointerMove={(e) => {
                const d = drag.current;
                if (!d || d.moved || Math.hypot(e.clientX - d.x, e.clientY - d.y) < 5) return;
                d.moved = true;
                setDragging(d.file);
              }}
              onPointerUp={(e) => {
                const d = drag.current;
                drag.current = null;
                setDragging(null);
                if (e.currentTarget.hasPointerCapture?.(e.pointerId)) {
                  e.currentTarget.releasePointerCapture(e.pointerId);
                }
                if (!d?.moved) return;
                const from = e.currentTarget.ownerDocument.defaultView ?? window;
                const target = dropTargetAt(from, e.clientX, e.clientY, e.screenX, e.screenY);
                target?.dispatchEvent(new CustomEvent(ATTACH_EVENT, { detail: [d.file] }));
              }}
            >
              {row.expanded !== undefined ? (
                <Icon name={row.expanded ? "chevronDown" : "chevronRight"} size={12} />
              ) : (
                <span className="files-tree__spacer" />
              )}
              <Icon
                name={
                  row.kind === "project"
                    ? "projects"
                    : row.kind === "copies"
                      ? "branch"
                      : row.kind === "root"
                        ? row.root?.kind === "workingCopy"
                          ? "branch"
                          : "projects"
                        : row.kind === "folder"
                          ? "projects"
                          : "file"
                }
                size={14}
              />
              <span className="files-tree__name">{row.label}</span>
              {row.writer && (
                <StatusDot
                  className="files-tree__mark"
                  status="pending"
                  label={`${row.writer} is writing here`}
                />
              )}
              {row.changing && (
                <StatusDot
                  className="files-tree__mark"
                  status="pending"
                  label={`${row.changing.worker} is changing this`}
                />
              )}
              {row.blocked && (
                <span
                  className="files-tree__mark files-tree__mark--blocked"
                  title="Workers may not touch this file"
                >
                  <Icon name="lock" size={12} />
                  Blocked for workers
                </span>
              )}
              {row.note && row.kind === "root" && (
                <span className="files-tree__mark">{row.note}</span>
              )}
              {row.kind === "file" && typeof row.size === "number" && (
                <span className="files-tree__size">{sizeWords(row.size)}</span>
              )}
            </div>
          );
        })}
      </div>
      {dragging && (
        <div className="files-panel__drag" role="status">
          {`Drop ${nameOf(dragging.path)} on an objective to put it on the objective`}
        </div>
      )}
    </section>
  );
}

function rootPathOf(roots: FileRoots | null, rootId: string | undefined): string | undefined {
  return roots?.roots.find((r) => r.id === rootId)?.path;
}

/** A key that is safe in an element's ID. */
function cssKey(key: string): string {
  return key.replace(/[^A-Za-z0-9_-]/g, (c) => `_${c.charCodeAt(0).toString(16)}_`);
}
