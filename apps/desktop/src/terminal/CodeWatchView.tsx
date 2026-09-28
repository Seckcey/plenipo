import { Fragment, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import type { WatchChange, WatchFileView, WatchLine } from "@plenipo/types";
import { Banner, Button, EmptyState, Segmented, StatusDot, cx } from "@plenipo/ui";

import { AgentsContext } from "../agents/context";
import { isRunning, isWaiting } from "../agents/store";
import { cancelAgentTurn, getWatch, getWatchChange, toCommandError } from "../api/commands";
import { subscribeWatch } from "../api/events";
import {
  applyWatchUpdate,
  countsWords,
  emptyCodeWatch,
  isOpen,
  KIND_WORD,
  loadWatchView,
  pinFile,
  removedWords,
  setFollowing,
  shownChange,
  splitPath,
  STATE_STATUS,
  STATE_WORD,
  taskLabels,
  writingNow,
  type CodeWatch,
} from "./code";
import type { CodeTab } from "./panel";

/** A change's file, as read from Plenipo. */
interface Loaded {
  /** The change's ID and state: a change read again once its state moves on. */
  key: string;
  /** `null`: Plenipo no longer has it. */
  file: WatchFileView | null;
}

/** Why a change's file could not be read. Not kept with the files: picking it reads it again. */
interface Failed {
  key: string;
  error: string;
}

/** The files read lately, kept so going back to one does not read it again. */
const KEPT_FILES = 8;

function remember(all: readonly Loaded[], one: Loaded): Loaded[] {
  return [one, ...all.filter((f) => f.key !== one.key)].slice(0, KEPT_FILES);
}

/**
 * An agent's Watch tab for code (Phase 18, ADR-055): the files its workers touched in this
 * objective, and the file chosen, with its new and changed lines marked as each change lands,
 * or its text as the AI writes it. Read-only: there is nowhere to type, and nothing here writes
 * to a working copy. **Stop** stops the worker's task, as **Cancel task** does on the Workers
 * page.
 */
export function CodeWatchView({
  tab,
  onWriting,
}: {
  tab: CodeTab;
  /** Whether a worker is writing a change now (the tab's mark). */
  onWriting?: ((tabId: string, writing: boolean) => void) | undefined;
}) {
  const { positionId } = tab;
  const [watch, setWatch] = useState<CodeWatch>(() => emptyCodeWatch(positionId));
  const [reading, setReading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [files, setFiles] = useState<readonly Loaded[]>([]);
  const [failed, setFailed] = useState<Failed | null>(null);

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    subscribeWatch((update) => {
      if (disposed) return;
      setWatch((s) => applyWatchUpdate(s, update));
      // News of this agent's changes: the list is up to date again from here on.
      if (update.change.positionId === positionId) setLoadError(null);
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch(() => undefined)
      // Listen first, then read what Plenipo has: a change made in between is not missed.
      .finally(() => {
        if (disposed) return;
        getWatch(positionId)
          .then((view) => {
            if (!disposed) setWatch((s) => loadWatchView(s, view));
          })
          .catch((reason: unknown) => {
            if (!disposed) setLoadError(toCommandError(reason).message);
          })
          .finally(() => {
            if (!disposed) setReading(false);
          });
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [positionId]);

  const writing = writingNow(watch);
  useEffect(() => {
    onWriting?.(tab.id, writing);
  }, [onWriting, tab.id, writing]);
  useEffect(() => () => onWriting?.(tab.id, false), [onWriting, tab.id]);

  /** Try again, after Plenipo could not read the changes. */
  const readAgain = () => {
    setLoadError(null);
    setReading(true);
    getWatch(positionId)
      .then((view) => setWatch((s) => loadWatchView(s, view)))
      .catch((reason: unknown) => setLoadError(toCommandError(reason).message))
      .finally(() => setReading(false));
  };

  const shown = shownChange(watch);
  const labels = taskLabels(watch.changes);

  // Stop: the task that made the change shown (the file on screen), while it is working. When
  // the list holds more than one task's work, the button names the task.
  const agents = useContext(AgentsContext);
  const stopTask = shown ? labels.get(shown.taskId) : undefined;
  const sessionId = shown?.sessionId ? shown.sessionId : null;
  const session = sessionId ? agents?.state.sessions[sessionId] : undefined;
  const working = sessionId !== null && (session ? isRunning(session) || isWaiting(session) : true);
  const stopTitle = !shown
    ? "Nothing is being worked on"
    : !working
      ? "Nothing to stop in this file's task"
      : stopTask
        ? `Stop ${stopTask}, the task that changed this file`
        : "Stop the task that changed this file";
  const stopIt = () => {
    if (!sessionId) return;
    setBusy(true);
    setError(null);
    // As Cancel task on the Workers page does (it also updates that page), when it is here.
    const cancel: (id: string) => Promise<unknown> = agents?.cancel ?? cancelAgentTurn;
    cancel(sessionId)
      .catch((reason: unknown) => setError(toCommandError(reason).message))
      .finally(() => setBusy(false));
  };

  // The file shown: read from Plenipo once saved (its lines, marked), or while being written
  // when the tab has none of its text yet.
  const liveText = shown ? watch.writing[shown.id] : undefined;
  const key = shown ? `${shown.id}:${shown.state}` : null;
  const shownId = shown?.id ?? null;
  const wanted =
    shown !== null &&
    (shown.state === "saved" ? !shown.summary : isOpen(shown.state) && liveText === undefined);
  const loaded = key === null ? undefined : files.find((f) => f.key === key);
  const have = loaded !== undefined;
  const failedHere = failed !== null && failed.key === key;
  useEffect(() => {
    if (!wanted || have || failedHere || key === null || shownId === null) return;
    let live = true;
    getWatchChange(shownId)
      .then((file) => {
        if (live) setFiles((all) => remember(all, { key, file }));
      })
      .catch((reason: unknown) => {
        if (live) setFailed({ key, error: toCommandError(reason).message });
      });
    return () => {
      live = false;
    };
  }, [wanted, have, failedHere, key, shownId]);

  /** A file picked from the list: it stays in view, and is read again if reading it failed. */
  const pick = (path: string) => {
    setFailed(null);
    setWatch((s) => pinFile(s, path));
  };

  return (
    <div className="code-watch">
      <div className="code-watch__bar">
        <span className="code-watch__who">Watch · {tab.title}</span>
        <span className="code-watch__note">
          Read-only: you see only what Guard lets this worker change.
        </span>
        <Segmented
          label="Which file shows"
          value={watch.following ? "follow" : "pin"}
          options={[
            { value: "follow", label: "Follow along" },
            { value: "pin", label: "Pin this file", icon: "lock" },
          ]}
          onChange={(v) => setWatch((s) => setFollowing(s, v === "follow"))}
        />
        <Button
          size="sm"
          icon="stop"
          disabled={busy || !working}
          title={stopTitle}
          onClick={stopIt}
        >
          {stopTask ? `Stop ${stopTask}` : "Stop"}
        </Button>
      </div>
      {error && (
        <Banner tone="error" role="alert" title="That did not work">
          {error}
        </Banner>
      )}
      {loadError && (
        <Banner
          tone="error"
          role="alert"
          title="Plenipo could not read the changes"
          action={
            <Button size="sm" onClick={readAgain}>
              Try again
            </Button>
          }
        >
          {loadError}
        </Banner>
      )}
      {watch.fromTheRecord && (
        <Banner tone="info" title="Plenipo started again since these changes">
          Their lines are shown only while Plenipo runs, so the lines from before it started again
          are not kept. The files are in the working copy.
        </Banner>
      )}
      {watch.changes.length === 0 ? (
        <div className="code-watch__empty">
          {reading ? (
            <p className="muted" role="status">
              Reading the changes…
            </p>
          ) : (
            <EmptyState title="No file changes yet in this objective" icon="file" compact>
              Changes show here as the worker makes them.
            </EmptyState>
          )}
        </div>
      ) : (
        <div className="code-watch__main">
          <nav className="code-watch__files" aria-label="Files touched in this objective">
            <ul className="code-watch__list">
              {watch.changes.map((c) => {
                const [name, folder] = splitPath(c.path);
                const current = shown?.path === c.path;
                const task = labels.get(c.taskId);
                return (
                  <li key={c.path}>
                    <button
                      type="button"
                      className={cx("code-watch__file", current && "code-watch__file--shown")}
                      aria-current={current ? "true" : undefined}
                      title={c.path}
                      onClick={() => pick(c.path)}
                    >
                      <span className="code-watch__file-name">{name}</span>
                      {folder && <span className="code-watch__file-folder">{folder}</span>}
                      <span className="code-watch__meta">
                        <StatusDot status={STATE_STATUS[c.state]} label={STATE_WORD[c.state]} />
                        {c.kind && <span>{KIND_WORD[c.kind]}</span>}
                        {c.state === "saved" && <Counts change={c} />}
                        {task && <span>{task}</span>}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </nav>
          {shown && (
            <FilePane
              key={shown.id}
              change={shown}
              text={liveText ?? loaded?.file?.writing ?? null}
              file={loaded?.file}
              error={failed !== null && failed.key === key ? failed.error : null}
              following={watch.following}
              task={labels.get(shown.taskId)}
            />
          )}
        </div>
      )}
    </div>
  );
}

/** "+3 −1", said as "3 lines added, 1 removed". */
function Counts({ change }: { change: WatchChange }) {
  return (
    <span className="code-watch__counts">
      <span aria-hidden="true">
        <span className="code-watch__added">+{change.added}</span>{" "}
        <span className="code-watch__taken">−{change.removed}</span>
      </span>
      <span className="ui-visually-hidden">{countsWords(change)}</span>
    </span>
  );
}

/** The chosen file: its state, and its lines (marked), its text so far, or why there is none. */
function FilePane({
  change: heard,
  text,
  file,
  error,
  following,
  task,
}: {
  change: WatchChange;
  /** The text so far, while it is being written (or waits for your approval). */
  text: string | null;
  /** The file as read from Plenipo: `undefined` not read (yet), `null` its lines are gone. */
  file: WatchFileView | null | undefined;
  /** Why the file could not be read. */
  error: string | null;
  following: boolean;
  task: string | undefined;
}) {
  const box = useRef<HTMLDivElement>(null);
  // What Plenipo said when the file was read wins when it is newer than what the tab heard. Its
  // summary counts even when it is not newer: Plenipo lets go of old lines (to keep its memory
  // small) without telling the tab, and then says so only in the file it gives.
  const read = file?.change;
  const change = read && read.id === heard.id && read.at > heard.at ? read : heard;
  const summary = change.summary ?? read?.summary;
  const lines = change.state === "saved" && !summary ? (file?.lines ?? null) : null;
  const open = isOpen(change.state);

  // Scroll to the change: the first new or changed line (or where lines went) once the file's
  // lines are here; the end of the text as it is written, while following along.
  useEffect(() => {
    const el = box.current;
    if (!el || !lines) return;
    const first = el.querySelector<HTMLElement>("[data-mark], [data-removed]");
    el.scrollTop = first ? Math.max(0, first.offsetTop - el.clientHeight / 3) : 0;
  }, [lines]);
  useEffect(() => {
    const el = box.current;
    if (!el || text === null || !open || !following) return;
    el.scrollTop = el.scrollHeight;
  }, [text, open, following]);

  let body: ReactNode;
  if (change.state === "refused") {
    body = (
      <Banner tone="error" role="status" title="Refused">
        {change.reason && <p className="code-watch__reason">{change.reason}</p>}
        <p>Guard or you said no. Nothing of it was saved, and its text is not shown.</p>
      </Banner>
    );
  } else if (change.state === "notSaved") {
    body = (
      <Banner tone="warn" role="status" title="Not saved">
        {change.reason && <p className="code-watch__reason">{change.reason}</p>}
        <p>Nothing of it was saved.</p>
      </Banner>
    );
  } else if (open) {
    body = (
      <>
        {change.state === "writing" ? (
          <Banner tone="pending" title="Being written — not saved yet">
            The AI is writing it now. What shows here is not saved yet.
          </Banner>
        ) : (
          <Banner tone="pending" title="Waiting for your approval">
            Approve or deny it in Approvals. It is not saved yet.
          </Banner>
        )}
        {summary && <p className="code-watch__summary">{summary}</p>}
        {text !== null ? (
          <div
            ref={box}
            className="code-watch__code code-watch__code--writing"
            role="region"
            aria-label={`${change.path}, not saved yet`}
            tabIndex={0}
          >
            {textRows(text).map((row, i) => (
              <div key={i} className="code-watch__line">
                <span className="code-watch__num" aria-hidden="true">
                  {i + 1}
                </span>
                <span className="code-watch__mark" aria-hidden="true" />
                <span className="code-watch__text">{row}</span>
              </div>
            ))}
          </div>
        ) : (
          !summary && (
            <p className="code-watch__summary">
              {change.state === "writing"
                ? "Its text shows here as the AI writes it."
                : "Its lines show here once it is saved."}
            </p>
          )
        )}
      </>
    );
  } else if (summary) {
    body = <p className="code-watch__summary">{summary}</p>;
  } else if (error) {
    body = (
      <Banner tone="error" role="status" title="Plenipo could not read this file">
        <p className="code-watch__reason">{error}</p>
        <p>Pick the file in the list to try again.</p>
      </Banner>
    );
  } else if (file === null) {
    body = (
      <p className="code-watch__summary">
        Plenipo no longer has the lines of this change. The file is in the working copy.
      </p>
    );
  } else if (!lines) {
    body = (
      <p className="code-watch__summary" role="status">
        Reading the file…
      </p>
    );
  } else if (lines.length === 0) {
    body = <p className="code-watch__summary">The file is empty now.</p>;
  } else {
    body = (
      <div
        ref={box}
        className="code-watch__code"
        role="region"
        aria-label={`${change.path}, after the change`}
        tabIndex={0}
      >
        {lines.map((line, i) => (
          <Fragment key={i}>
            {line.removedBefore > 0 && <Removed n={line.removedBefore} />}
            <Line line={line} number={i + 1} />
          </Fragment>
        ))}
        {(file?.removedAtEnd ?? 0) > 0 && <Removed n={file?.removedAtEnd ?? 0} />}
      </div>
    );
  }

  return (
    <div className="code-watch__view">
      <div className="code-watch__head">
        <span className="code-watch__path">{change.path}</span>
        <StatusDot status={STATE_STATUS[change.state]} label={STATE_WORD[change.state]} />
        {change.kind && <span>{KIND_WORD[change.kind]}</span>}
        {change.state === "saved" && <Counts change={change} />}
        {task && <span>{task}</span>}
        {lines && lines.length > 0 && (
          <span className="code-watch__legend" aria-hidden="true">
            <span className="code-watch__key code-watch__key--new">+ new</span>
            <span className="code-watch__key code-watch__key--changed">~ changed</span>
          </span>
        )}
      </div>
      {body}
    </div>
  );
}

const MARK_SIGN = { new: "+", changed: "~" } as const;
const MARK_WORD = { new: "new line: ", changed: "changed line: " } as const;

/** One line of the file: its number, its mark (a sign and a word, not only a color), its text. */
function Line({ line, number }: { line: WatchLine; number: number }) {
  return (
    <div
      className={cx("code-watch__line", line.mark && `code-watch__line--${line.mark}`)}
      data-mark={line.mark}
    >
      <span className="code-watch__num" aria-hidden="true">
        {number}
      </span>
      <span className="code-watch__mark" aria-hidden="true">
        {line.mark ? MARK_SIGN[line.mark] : ""}
      </span>
      {line.mark && <span className="ui-visually-hidden">{MARK_WORD[line.mark]}</span>}
      <span className="code-watch__text">{line.text}</span>
    </div>
  );
}

/** Where lines went: "3 lines removed here". */
function Removed({ n }: { n: number }) {
  return (
    <div className="code-watch__removed" data-removed={n}>
      <span className="code-watch__num" aria-hidden="true" />
      <span className="code-watch__mark" aria-hidden="true">
        −
      </span>
      <span className="code-watch__text">{removedWords(n)}</span>
    </div>
  );
}

/** The text so far, line by line (without the empty line after a last line break). */
function textRows(text: string): string[] {
  const rows = text.split("\n").map((r) => (r.endsWith("\r") ? r.slice(0, -1) : r));
  if (rows.length > 1 && rows[rows.length - 1] === "") rows.pop();
  return rows;
}
