import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { EditorState } from "@codemirror/state";
import type { FileView, WatchChange } from "@plenipo/types";
import {
  Banner,
  Button,
  EmptyState,
  ErrorState,
  LoadingState,
  MenuButton,
  PageHeader,
  StatusDot,
  Tabs,
} from "@plenipo/ui";

import {
  cancelAgentTurn,
  getWatchChange,
  openFileOutside,
  readFile,
  saveFile,
  showInFolder,
  toCommandError,
} from "../api/commands";
import { subscribeLedgerEvents, subscribeWatch } from "../api/events";
import { useControl } from "../control/useControl";
import type { Go } from "../components/views";
import { CodeEditor } from "./CodeEditor";
import { editorState, SAVE_EVENT, type LineMark } from "./editorSetup";
import { editorStore, useEditorFiles } from "./editorStore";
import { folderOf, nameOf, parseFileKey, sizeWords } from "./refs";
import { shortcut, systemWords } from "../system/words";

/** A worker's change to this file, showing as it lands (ADR-093 §15). */
interface Live {
  worker: string;
  state: "writing" | "saved";
  text: string;
  marks: LineMark[];
}

/** Events after which who writes where may have changed: the file is read again. */
const changesWriters = (type: string) =>
  type.startsWith("guard.grant_") || type === "workspace.removed";

/**
 * The editor (Phase 21, ADR-093): a tab for each open file, and the file chosen — code with
 * colors, a picture, or what the file is. Save writes it through Plenipo (recorded in the
 * Activity trail as yours). A working copy a worker is writing opens read-only, names the worker,
 * and offers Wait or Stop the worker. A worker's change to the file shows as it lands.
 */
export function EditorPage({
  id,
  go,
  onBack,
}: {
  id: string;
  go: Go;
  onBack?: (() => void) | undefined;
}) {
  const files = useEditorFiles();
  useEffect(() => editorStore.open(id), [id]);
  const [closing, setClosing] = useState<string | null>(null);
  const keys = files.keys.includes(id) ? files.keys : [...files.keys, id];
  const close = (key: string) => {
    if (files.unsaved.has(key) && closing !== key) {
      setClosing(key);
      return;
    }
    setClosing(null);
    const at = keys.indexOf(key);
    const rest = keys.filter((k) => k !== key);
    editorStore.close(key);
    if (key === id) {
      const next = rest[at] ?? rest[at - 1];
      if (next) go({ view: "file", id: next });
      else onBack?.();
    }
  };
  const target = parseFileKey(id);
  return (
    <div className="editor-page">
      <PageHeader title="Files" onBack={onBack} />
      <Tabs
        label="Open files"
        value={id}
        idPrefix="editor"
        onChange={(key) => go({ view: "file", id: key })}
        tabs={keys.map((key) => {
          const name = nameOf(parseFileKey(key)?.path ?? key);
          const unsaved = files.unsaved.has(key);
          return {
            value: key,
            label: (
              <>
                {name}
                {unsaved && <StatusDot status="pending" label="not saved yet" />}
              </>
            ),
            onClose: () => close(key),
            closeLabel: `Close ${name}`,
          };
        })}
      />
      {closing && (
        <Banner
          tone="warn"
          role="alert"
          title={`${nameOf(parseFileKey(closing)?.path ?? closing)} has changes not saved yet`}
          action={
            <>
              <Button size="sm" variant="danger" onClick={() => close(closing)}>
                Close without saving
              </Button>
              <Button size="sm" onClick={() => setClosing(null)}>
                Keep it open
              </Button>
            </>
          }
        />
      )}
      {target ? (
        <FileEditor key={id} fileId={id} root={target.root} path={target.path} />
      ) : (
        <ErrorState title="Plenipo does not know that file" message="Open it from Files." />
      )}
    </div>
  );
}

function FileEditor({ fileId, root, path }: { fileId: string; root: string; path: string }) {
  const [view, setView] = useState<FileView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [state, setState] = useState<EditorState | null>(() => editorStore.kept(fileId) ?? null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [changedOnDisk, setChangedOnDisk] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const [stopAsked, setStopAsked] = useState(false);
  const [live, setLive] = useState<Live | null>(null);
  const [doneNote, setDoneNote] = useState<string | null>(null);
  const control = useControl();
  const desktop = (control.status?.sessions ?? []).some(
    (s) => s.kind === "desktop" && s.state === "active",
  );
  const viewRef = useRef<FileView | null>(null);
  useEffect(() => {
    viewRef.current = view;
  }, [view]);

  /** Read the file (again). `fresh`: drop what was kept, and show the file as it is now. */
  const load = useCallback(
    (fresh: boolean) => {
      readFile(root, path)
        .then((v) => {
          const before = viewRef.current;
          setView(v);
          setError(null);
          if (v.content.kind !== "text") return;
          // What you typed stays, with the fingerprint of the file as you started editing it: a
          // change on the disk since is caught when you save (ADR-093 §7, §12).
          const keep = !fresh && editorStore.kept(fileId);
          if (keep) {
            if (!editorStore.base(fileId)) {
              editorStore.setBase(fileId, { hash: v.hash ?? null, text: v.content.text });
            }
            return;
          }
          // Unchanged on disk and nothing typed: the editor stays as it is.
          if (before?.hash === v.hash && state !== null && !fresh) return;
          editorStore.setBase(fileId, { hash: v.hash ?? null, text: v.content.text });
          editorStore.forget(fileId);
          setState(editorState(v.content.text, false));
        })
        .catch((e: unknown) => setError(toCommandError(e).message));
    },
    [root, path, fileId, state],
  );

  useEffect(() => {
    load(false);
    // Read once when the file opens; later reads come from the events below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fileId]);

  const writer = view?.readOnly?.kind === "writer" ? view.readOnly.writer : null;
  const writerRef = useRef(writer);
  useEffect(() => {
    writerRef.current = writer;
  }, [writer]);

  // A worker starting or ending a step here: the file is read again (read-only or not).
  useEffect(() => {
    let live = true;
    let stop: (() => void) | null = null;
    let timer: ReturnType<typeof setTimeout> | null = null;
    void subscribeLedgerEvents((e) => {
      if (!changesWriters(e.eventType) || timer) return;
      timer = setTimeout(() => {
        timer = null;
        if (!live) return;
        readFile(root, path)
          .then((v) => {
            const was = writerRef.current;
            const now = v.readOnly?.kind === "writer" ? v.readOnly.writer : null;
            setView(v);
            if (was && !now) {
              // The worker is done: its change is on the disk; the editor shows the file as it
              // is now (what you typed and did not save stays).
              setLive(null);
              setWaiting(false);
              setStopAsked(false);
              setDoneNote(`${was.worker} is done. You can edit again.`);
              const unsaved = editorStore.snapshot().unsaved.has(fileId);
              if (!unsaved && v.content.kind === "text") {
                editorStore.setBase(fileId, { hash: v.hash ?? null, text: v.content.text });
                editorStore.forget(fileId);
                setState(editorState(v.content.text, false));
              }
            }
          })
          .catch(() => undefined);
      }, 250);
    })
      .then((s) => (live ? (stop = s) : s()))
      .catch(() => undefined);
    return () => {
      live = false;
      stop?.();
      if (timer) clearTimeout(timer);
    };
  }, [root, path, fileId]);

  // A worker's change to this file: shown as it is written, then as saved, lines marked.
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | null = null;
    const matches = (c: WatchChange) => c.root === root && c.path === path;
    void subscribeWatch((u) => {
      if (!alive || !matches(u.change)) return;
      const c = u.change;
      if (c.state === "writing" && u.writing !== undefined) {
        setLive({ worker: c.worker, state: "writing", text: u.writing, marks: [] });
      } else if (c.state === "saved") {
        getWatchChange(c.id)
          .then((f) => {
            if (!alive || !f || f.lines.length === 0) return;
            setLive({
              worker: c.worker,
              state: "saved",
              text: f.lines.map((l) => l.text).join("\n"),
              marks: f.lines.map((l) => l.mark ?? null),
            });
          })
          .catch(() => undefined);
      } else if (c.state === "refused" || c.state === "notSaved") {
        setLive(null);
      }
    })
      .then((s) => (alive ? (stop = s) : s()))
      .catch(() => undefined);
    return () => {
      alive = false;
      stop?.();
    };
  }, [root, path]);

  const unsaved = useEditorFiles().unsaved.has(fileId);
  const content = view?.content;
  const text = content?.kind === "text" ? content : null;
  // A blocked file is hidden while a worker uses the screen, mouse, and keyboard.
  const hidden = !!view?.blocked && desktop;
  const readOnly = !!view?.readOnly || desktop;

  const save = useCallback(
    (anyway: boolean) => {
      const v = viewRef.current;
      if (!v || v.content.kind !== "text" || !state || saving) return;
      const now = editorStore.kept(fileId) ?? state;
      const body = now.doc.toString();
      setSaving(true);
      setProblem(null);
      saveFile(
        root,
        path,
        body,
        v.content.bom,
        v.content.lineEnding,
        // The file as you started editing it, not as a later look found it (ADR-093 §7).
        anyway ? null : (editorStore.base(fileId)?.hash ?? v.hash ?? null),
      )
        .then((outcome) => {
          if (outcome.kind === "changedOnDisk") {
            setChangedOnDisk(true);
            return;
          }
          setChangedOnDisk(false);
          editorStore.setBase(fileId, { hash: outcome.hash, text: body });
          editorStore.keep(fileId, now, false);
          if (v.content.kind === "text") {
            setView({
              ...v,
              hash: outcome.hash,
              size: outcome.size,
              modified: outcome.modified,
              content: { ...v.content, text: body },
            });
          }
          setSaved(
            `Saved · ${outcome.added} line${outcome.added === 1 ? "" : "s"} added, ${outcome.removed} removed`,
          );
        })
        .catch((e: unknown) => setProblem(toCommandError(e).message))
        .finally(() => setSaving(false));
    },
    [root, path, fileId, state, saving],
  );

  // Ctrl+S in the editor.
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const onSave = () => {
      if (!readOnly && unsaved) save(false);
    };
    el.addEventListener(SAVE_EVENT, onSave);
    return () => el.removeEventListener(SAVE_EVENT, onSave);
  }, [save, readOnly, unsaved]);

  const onChange = useCallback(
    (next: EditorState) => {
      editorStore.keep(fileId, next, next.doc.toString() !== editorStore.base(fileId)?.text);
      setSaved(null);
      setDoneNote(null);
    },
    [fileId],
  );

  const act = (what: () => Promise<void>) => {
    setProblem(null);
    what().catch((e: unknown) => setProblem(toCommandError(e).message));
  };
  const name = nameOf(path);
  const folder = folderOf(path);
  const marks = useMemo(() => live?.marks ?? [], [live]);

  if (error) {
    return (
      <ErrorState
        title={`${name} could not open`}
        message={error}
        onRetry={() => {
          setError(null);
          load(true);
        }}
      />
    );
  }
  if (!view) return <LoadingState label={`Opening ${name}…`} />;
  const menuItems = [
    ...(view.runs
      ? []
      : [{ id: "outside", label: "Open in another program", icon: "external" as const }]),
    { id: "folder", label: systemWords().showFile, icon: "projects" as const },
  ];
  return (
    <div className="file-editor" ref={box}>
      <div className="file-editor__bar">
        <div className="file-editor__where" title={`${root} · ${path}`}>
          <span className="file-editor__folder">{folder ? `${folder}/` : ""}</span>
          <strong>{name}</strong>
          <span className="muted"> · {sizeWords(view.size)}</span>
          {view.blocked && <span className="file-editor__mark">Blocked for workers</span>}
          {view.runs && <span className="file-editor__mark">A program or a script</span>}
        </div>
        <span className="file-editor__state" role="status">
          {saving ? "Saving…" : unsaved ? "Not saved yet" : (saved ?? "")}
        </span>
        {text && (
          <>
            <Button
              size="sm"
              variant="primary"
              disabled={readOnly || !unsaved || saving || hidden}
              onClick={() => save(false)}
              title={`Save (${shortcut(["mod"], "S")})`}
            >
              Save
            </Button>
            <Button
              size="sm"
              variant="quiet"
              onClick={() => {
                setChangedOnDisk(false);
                load(true);
              }}
            >
              Reload
            </Button>
          </>
        )}
        <MenuButton
          label="More"
          icon="more"
          variant="quiet"
          align="end"
          items={menuItems}
          onSelect={(choice) =>
            act(() =>
              choice === "outside" ? openFileOutside(root, path) : showInFolder(root, path),
            )
          }
        />
      </div>
      {problem && (
        <Banner tone="error" role="alert" title={problem} onDismiss={() => setProblem(null)} />
      )}
      {changedOnDisk && (
        <Banner
          tone="warn"
          role="alert"
          title="This file changed on the disk since you opened it"
          action={
            <>
              <Button size="sm" onClick={() => load(true)}>
                Reload
              </Button>
              <Button size="sm" variant="danger" onClick={() => save(true)}>
                Save anyway
              </Button>
            </>
          }
        >
          Reload shows the file as it is now (your changes are lost). Save anyway writes yours over
          it.
        </Banner>
      )}
      {writer && (
        <Banner
          tone="pending"
          role="status"
          title={
            waiting
              ? `Waiting for ${writer.worker} to finish…`
              : `${writer.worker} is writing in this working copy`
          }
          action={
            stopAsked ? (
              <>
                <Button
                  size="sm"
                  variant="danger"
                  onClick={() => act(() => cancelAgentTurn(writer.sessionId).then(() => undefined))}
                >
                  Stop the worker
                </Button>
                <Button size="sm" onClick={() => setStopAsked(false)}>
                  Keep it working
                </Button>
              </>
            ) : (
              <>
                {!waiting && (
                  <Button size="sm" onClick={() => setWaiting(true)}>
                    Wait
                  </Button>
                )}
                <Button size="sm" variant="danger" onClick={() => setStopAsked(true)}>
                  Stop the worker
                </Button>
              </>
            )
          }
        >
          {stopAsked
            ? `Stop ${writer.worker}'s task? The file becomes yours to edit once its step ends.`
            : unsaved
              ? "You can read along. Your changes are kept here; save them once it is done."
              : "You can read along; editing waits until it is done."}
        </Banner>
      )}
      {doneNote && (
        <Banner tone="ok" role="status" title={doneNote} onDismiss={() => setDoneNote(null)} />
      )}
      {desktop && (
        <Banner
          tone="pending"
          role="status"
          title="A worker is using the screen, mouse, and keyboard"
        >
          {view.blocked
            ? "This file is hidden, and nothing is saved, until you take over."
            : "Nothing is typed or saved here until you take over."}
        </Banner>
      )}
      {view.readOnly?.kind === "disk" && (
        <Banner tone="info" role="status" title="This file is marked read-only on the disk" />
      )}
      {live && (
        <div className="file-editor__live" role="status">
          <StatusDot
            status={live.state === "writing" ? "pending" : "ok"}
            label={
              live.state === "writing"
                ? `${live.worker}: being written — not saved yet`
                : `${live.worker} saved a change: new and changed lines are marked`
            }
          />
        </div>
      )}
      <div className="file-editor__body">
        {hidden ? (
          <EmptyState title="Hidden while a worker uses the screen">
            Take over to see this file again.
          </EmptyState>
        ) : content?.kind === "picture" ? (
          <div className="file-editor__picture">
            <img src={`data:${content.mime};base64,${content.data}`} alt={name} />
          </div>
        ) : content?.kind === "other" ? (
          <EmptyState title={name}>
            {content.what} {sizeWords(view.size)}.
            <div className="file-editor__other">
              {!view.runs && (
                <Button size="sm" onClick={() => act(() => openFileOutside(root, path))}>
                  Open in another program
                </Button>
              )}
              <Button size="sm" variant="quiet" onClick={() => act(() => showInFolder(root, path))}>
                {systemWords().showFile}
              </Button>
            </div>
          </EmptyState>
        ) : state ? (
          <CodeEditor
            state={state}
            fileName={name}
            readOnly={readOnly}
            shown={live && !unsaved ? live.text : null}
            marks={live && !unsaved ? marks : []}
            label={`${name}, ${readOnly ? "read-only" : "editable"}`}
            onChange={onChange}
          />
        ) : (
          <LoadingState label={`Opening ${name}…`} />
        )}
      </div>
    </div>
  );
}
