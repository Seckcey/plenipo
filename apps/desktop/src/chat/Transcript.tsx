import { Fragment, memo, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { HandoffView, WorkFolder } from "@plenipo/types";
import { Button, Icon, cx } from "@plenipo/ui";

import { useLiaisonRevision, useTaskHandoffs } from "../agents/useTaskHandoffs";
import { getWorkFolder, openWorkFolder, toCommandError } from "../api/commands";
import { AskedBy, RepliesBack, ReplyBack, RequestCard, type ChatLiaison } from "./Exchanges";
import { matchRequests, splitHandoffs } from "./handoffBlocks";
import { tokens } from "../routing/format";
import { useNow } from "../runtime/useNow";
import { Markdown } from "./Markdown";
import {
  doingNow,
  filesOf,
  isOver,
  type ChatSession,
  type ChatTurn,
  type Part,
  type ToolCall,
} from "./model";
import { baseName, describeCalls, elapsed, thoughtFor, toolPhrase } from "./words";

type Tools = Extract<Part, { kind: "tools" }>;
type Thinking = Extract<Part, { kind: "thinking" }>;

/** How close to the end counts as being at the end (the newest words keep coming into view). */
const AT_END = 48;

/**
 * A conversation as it happens (ADR-200): your messages, and each answer as the agent writes it —
 * its words, its thinking, each run of tool calls in one line you can open, what it is doing now
 * with a timer, and the files it saved. Follows the newest words while you are at the end; scroll
 * up and it stays put, with a button back to the end.
 */
export function Transcript({
  session,
  title,
  askFrom = null,
  tool = null,
  liaison = null,
  onOpenTask,
  onShowExecution,
  onOpenLink,
}: {
  session: ChatSession | null;
  title: string;
  /** Who each message is from, when it is not you ("From its lead", ADR-203). */
  askFrom?: string | null;
  /** Its AI tool, by name: said when it reports no tokens. */
  tool?: string | null;
  /** It works with other agents through Liaison: their requests and replies show here (B5). */
  liaison?: ChatLiaison | null;
  /** Open a task's own page (its record), from Details under its answer. */
  onOpenTask?: ((taskId: string) => void) | undefined;
  /** Show a run's raw output (the AI tools page), from Raw output under its answer. */
  onShowExecution?: ((executionId: string) => void) | undefined;
  onOpenLink?: ((url: string) => void) | undefined;
}) {
  const revision = useLiaisonRevision();
  const box = useRef<HTMLDivElement>(null);
  const following = useRef(true);
  const [away, setAway] = useState(false);
  const turns = session?.turns ?? [];
  const last = turns[turns.length - 1];

  const onScroll = () => {
    const el = box.current;
    if (!el) return;
    const atEnd = el.scrollHeight - el.scrollTop - el.clientHeight < AT_END;
    following.current = atEnd;
    setAway(!atEnd);
  };
  // New words: to the end, unless you scrolled up to read.
  useLayoutEffect(() => {
    const el = box.current;
    if (el && following.current) el.scrollTop = el.scrollHeight;
  }, [session]);
  const toEnd = () => {
    const el = box.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    following.current = true;
    setAway(false);
  };

  return (
    <div className="chat-log">
      <div
        ref={box}
        className="chat-log__scroll"
        role="log"
        aria-label={`Conversation with ${title}`}
        // Words arrive many times a second: the summary below says what matters, once.
        aria-live="off"
        tabIndex={0}
        onScroll={onScroll}
      >
        {turns.length === 0 ? (
          <div className="chat-log__empty">
            <Icon name="chat" size={22} />
            <p>
              Say what you need. {title} answers here as it works: you see its words as it writes
              them, what it is doing, and the files it saves.
            </p>
          </div>
        ) : (
          turns.map((turn, i) => (
            <TurnView
              key={turn.taskId}
              turn={turn}
              // Its place in what this chat shows (a thread of several conversations, B5).
              number={i + 1}
              title={title}
              askFrom={askFrom}
              tool={tool}
              liaison={liaison}
              onOpenTask={onOpenTask}
              onShowExecution={onShowExecution}
              revision={revision}
              onOpenLink={onOpenLink}
            />
          ))
        )}
      </div>
      {away && (
        <button type="button" className="chat-log__to-end" onClick={toEnd}>
          <Icon name="arrowDown" size={14} />
          {last && !isOver(last) ? "Follow the newest words" : "To the newest"}
        </button>
      )}
      <Announcer turn={last} title={title} />
    </div>
  );
}

/**
 * What a screen reader hears: only when an answer starts, finishes, stops, or fails — never each
 * word.
 */
function Announcer({ turn, title }: { turn: ChatTurn | undefined; title: string }) {
  const state = turn?.state ?? null;
  const words =
    state === "working"
      ? `${title} is working.`
      : state === "waiting"
        ? `${title} is waiting for its team.`
        : state === "done"
          ? `${title} answered.`
          : state === "stopped"
            ? `${title} stopped.`
            : state === "failed"
              ? `${title} could not finish.`
              : "";
  return (
    <div className="visually-hidden" role="status" aria-live="polite">
      {words}
    </div>
  );
}

/**
 * Where each step after the first begins, and the replies that came back into it: each reply goes
 * to the first step that began after it was written (Plenipo passes replies on in the next step).
 */
function stepMarks(
  turn: ChatTurn,
  sent: readonly HandoffView[],
): Map<number, { step: number; replies: HandoffView[] }> {
  const marks = new Map<number, { step: number; replies: HandoffView[] }>();
  for (const start of turn.stepStarts) {
    marks.set(start.firstPart, { step: start.step, replies: [] });
  }
  for (const view of sent) {
    const reply = view.reply;
    if (!reply || reply.state !== "delivered") continue;
    const start = turn.stepStarts.find((s) => s.at >= reply.createdAt);
    if (start) marks.get(start.firstPart)?.replies.push(view);
  }
  return marks;
}

/** One message and its answer. Drawn again only when it changes. */
const TurnView = memo(function TurnView({
  turn,
  number,
  title,
  askFrom,
  tool,
  liaison,
  onOpenTask,
  onShowExecution,
  revision,
  onOpenLink,
}: {
  turn: ChatTurn;
  number: number;
  title: string;
  askFrom: string | null;
  tool: string | null;
  liaison: ChatLiaison | null;
  onOpenTask?: ((taskId: string) => void) | undefined;
  onShowExecution?: ((executionId: string) => void) | undefined;
  revision: number;
  onOpenLink?: ((url: string) => void) | undefined;
}) {
  const live = !isOver(turn);
  const files = isOver(turn) ? filesOf(turn) : [];
  // What it asked of its team, and who asked it (B5): Liaison's record of this task.
  const handoffs = useTaskHandoffs(liaison ? turn.taskId : null, turn.taskId, revision, !live);
  const received = liaison ? (handoffs?.received ?? null) : null;
  const sent = handoffs?.sent ?? [];
  const pieces = turn.parts.map((p) => (p.kind === "text" ? splitHandoffs(p.text) : null));
  const asked = pieces.flatMap((ps) => (ps ?? []).filter((p) => p.kind === "handoff"));
  const views = matchRequests(asked, sent);
  const marks = stepMarks(turn, sent);
  let request = 0;
  return (
    <article
      className="chat-turn"
      aria-label={`Message ${number}`}
      // How it stands, for the real-app tests (the screen says it in words).
      data-state={turn.state}
      data-outcome={turn.outcome ?? undefined}
    >
      {turn.ask.trim() !== "" && (
        <div className="chat-turn__ask">
          {received && liaison ? (
            <AskedBy view={received} worker={title} liaison={liaison} />
          ) : (
            askFrom && <span className="chat-turn__from">{askFrom}</span>
          )}
          <p>{turn.ask}</p>
        </div>
      )}
      <div className="chat-turn__answer">
        {turn.stepStarts.length > 0 && <p className="chat-stepmark">Step 1</p>}
        {turn.parts.map((part, i) => {
          const mark = marks.get(i);
          const own = pieces[i];
          return (
            <Fragment key={part.id}>
              {mark && (
                <>
                  <p className="chat-stepmark">
                    Step {mark.step} ·{" "}
                    {sent.length > 0 ? "continued with handoff replies" : "continued"}
                  </p>
                  <RepliesBack views={mark.replies} />
                </>
              )}
              {own && liaison && own.some((p) => p.kind === "handoff") ? (
                own.map((piece, j) =>
                  piece.kind === "text" ? (
                    <div key={j} className="chat-words">
                      <Markdown text={piece.text} onOpenLink={onOpenLink} />
                    </div>
                  ) : (
                    <RequestCard
                      key={j}
                      to={piece.to}
                      objective={piece.objective}
                      view={views[request++] ?? null}
                      liaison={liaison}
                    />
                  ),
                )
              ) : (
                <PartView part={part} live={live} onOpenLink={onOpenLink} />
              )}
            </Fragment>
          );
        })}
        {live && <LiveRow turn={turn} />}
        {files.length > 0 && <FilesCard taskId={turn.taskId} files={files} title={title} />}
        {turn.problem && (
          <p className={cx("chat-turn__problem", turn.state === "stopped" && "is-stopped")}>
            <Icon name={turn.state === "stopped" ? "stop" : "alert"} size={14} />
            {turn.state === "stopped" ? "Stopped. " : ""}
            {turn.problem}
          </p>
        )}
        {received?.reply && liaison && !live && (
          <ReplyBack view={received} worker={title} liaison={liaison} />
        )}
        {isOver(turn) && (
          <TurnFoot
            turn={turn}
            tool={tool}
            onOpenTask={onOpenTask ? () => onOpenTask(turn.taskId) : undefined}
            onShowRaw={
              onShowExecution && turn.executionId
                ? () => turn.executionId && onShowExecution(turn.executionId)
                : undefined
            }
          />
        )}
      </div>
    </article>
  );
});

function PartView({
  part,
  live,
  onOpenLink,
}: {
  part: Part;
  live: boolean;
  onOpenLink?: ((url: string) => void) | undefined;
}) {
  switch (part.kind) {
    case "text":
      return (
        <div className={cx("chat-words", part.streaming && "is-streaming")}>
          <Markdown text={part.text} onOpenLink={onOpenLink} />
        </div>
      );
    case "thinking":
      return <ThinkingView part={part} />;
    case "tools":
      return <ToolGroup part={part} live={live} />;
    case "note":
      return (
        <p className={cx("chat-note", part.level === "warning" && "chat-note--warning")}>
          <Icon name={part.level === "warning" ? "alert" : "info"} size={14} />
          {part.text}
        </p>
      );
  }
}

/** Its thinking: one line ("Thought for 6 s") you can open to read it. */
function ThinkingView({ part }: { part: Thinking }) {
  const [open, setOpen] = useState(false);
  const thinking = part.endedAt === null;
  const label = part.endedAt === null ? "Thinking…" : thoughtFor(part.endedAt - part.startedAt);
  return (
    <div className={cx("chat-thinking", thinking && "is-live")}>
      <button
        type="button"
        className="chat-fold"
        aria-expanded={open}
        disabled={part.text.trim() === ""}
        onClick={() => setOpen(!open)}
      >
        <span>{label}</span>
        {part.text.trim() !== "" && <Icon name={open ? "chevronDown" : "chevronRight"} size={14} />}
      </button>
      {open && <p className="chat-thinking__text">{part.text}</p>}
    </div>
  );
}

/**
 * A run of tool calls as one line ("Ran 2 programs, read Inspector.tsx"), open while they run so
 * each step shows as it happens, and closed when they are done (open it again to see them).
 */
function ToolGroup({ part, live }: { part: Tools; live: boolean }) {
  const running = part.calls.some((c) => c.state === "running");
  const [choice, setChoice] = useState<boolean | null>(null);
  const open = choice ?? (live && running);
  const failed = part.calls.filter((c) => c.state === "failed").length;
  return (
    <div className={cx("chat-tools", running && "is-running")}>
      <button
        type="button"
        className="chat-fold"
        aria-expanded={open}
        onClick={() => setChoice(!open)}
      >
        <span>{describeCalls(part.calls, !running)}</span>
        {failed > 0 && <span className="chat-tools__failed">{failed} did not work</span>}
        <Icon name={open ? "chevronDown" : "chevronRight"} size={14} />
      </button>
      {open && (
        <ol className="chat-steps">
          {part.calls.map((call) => (
            <ToolRow key={call.id} call={call} />
          ))}
        </ol>
      )}
    </div>
  );
}

function ToolRow({ call }: { call: ToolCall }) {
  const mark: "check" | "alert" | "clock" =
    call.state === "done" ? "check" : call.state === "failed" ? "alert" : "clock";
  return (
    <li className={cx("chat-step", `is-${call.state}`)}>
      <span className="chat-step__head">
        <Icon
          name={mark}
          size={13}
          {...(call.state === "failed" ? { label: "did not work" } : {})}
        />
        <span className="chat-step__what">{toolPhrase(call.tool, call.state === "running")}</span>
        {call.endedAt !== null && (
          <span className="chat-step__time">{elapsed(call.endedAt - call.startedAt)}</span>
        )}
      </span>
      {call.summary.trim() !== "" && <code className="chat-step__detail">{call.summary}</code>}
      {call.result.trim() !== "" && <code className="chat-step__result">{call.result}</code>}
    </li>
  );
}

/** What it is doing now, with a timer that counts up — never a bare "thinking". */
function LiveRow({ turn }: { turn: ChatTurn }) {
  const now = useNow(1000);
  const doing = doingNow(turn, (tool) => toolPhrase(tool, true));
  if (!doing) return null;
  const since = Math.min(doing.since, now);
  return (
    <div className="chat-live" aria-hidden="true">
      <span className="chat-live__line">
        <span className="chat-live__dots">
          <span />
          <span />
          <span />
        </span>
        <span className="chat-live__what">{doing.text}</span>
        <span className="chat-live__time">{elapsed(now - since)}</span>
      </span>
      {doing.detail && <code className="chat-live__detail">{doing.detail}</code>}
    </div>
  );
}

/** The files an answer saved, where they are, and a button that opens their folder. */
function FilesCard({
  taskId,
  files,
  title,
}: {
  taskId: string;
  files: { path: string; changed: boolean; failed: boolean }[];
  title: string;
}) {
  const [folder, setFolder] = useState<WorkFolder | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    getWorkFolder(taskId)
      .then((found) => {
        if (!cancelled) setFolder(found);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [taskId]);
  const saved = files.filter((f) => !f.failed);
  if (saved.length === 0) return null;
  const open = () => {
    setProblem(null);
    openWorkFolder(taskId).catch((reason: unknown) => setProblem(toCommandError(reason).message));
  };
  return (
    <section className="chat-files" aria-label={`Files ${title} saved`}>
      <div className="chat-files__head">
        <Icon name="file" size={14} />
        <span>
          {saved.length === 1 ? "1 file saved" : `${saved.length} files saved`}
          {folder?.plenipoFiles && " in Plenipo's folder"}
          {folder && !folder.plenipoFiles && folder.project && ` in the ${folder.project} project`}
          {folder?.branch && ` (working copy ${folder.branch})`}
        </span>
        {folder?.exists && (
          <Button size="sm" variant="quiet" icon="projects" onClick={open}>
            Open folder
          </Button>
        )}
      </div>
      <ul className="chat-files__list">
        {saved.map((f) => (
          <li key={f.path} title={f.path}>
            <span className="chat-files__name">{baseName(f.path)}</span>
            {f.changed && <span className="muted"> changed</span>}
          </li>
        ))}
      </ul>
      {folder && <p className="chat-files__where">{folder.path}</p>}
      {problem && <p className="chat-files__problem">{problem}</p>}
    </section>
  );
}

/**
 * Under a finished answer: how long it took, its model, and its tokens (pieces of words) over all
 * its steps, which open to show how many it read, reused, and wrote (I2). A turn stopped before a
 * step reported says "at least"; an AI tool that reports none says so. Beside the tokens, what
 * Plenipo itself sent with the task, by size (ADR-044). **Details** opens the task's own page.
 */
function TurnFoot({
  turn,
  tool,
  onOpenTask,
  onShowRaw,
}: {
  turn: ChatTurn;
  tool: string | null;
  onOpenTask?: (() => void) | undefined;
  onShowRaw?: (() => void) | undefined;
}) {
  const [open, setOpen] = useState(false);
  const parts: string[] = [];
  if (turn.endedAt !== null) parts.push(`${elapsed(turn.endedAt - turn.startedAt)}`);
  if (turn.model) parts.push(turn.model);
  const usage = turn.usage;
  const total = usage ? usage.inputTokens + usage.outputTokens : 0;
  const counted =
    usage && total > 0 ? `${turn.usageAtLeast ? "at least " : ""}${tokens(total)} tokens` : null;
  if (!counted && turn.state === "done" && tool) parts.push(`${tool} did not report tokens`);
  // With no tokens to open, its own text's size shows on the line itself.
  if (!counted && turn.ownText) parts.push(turn.ownText);
  if (parts.length === 0 && !counted && !onOpenTask && !onShowRaw) return null;
  return (
    <>
      <p className="chat-turn__foot">
        {parts.join(" · ")}
        {counted && (
          <>
            {parts.length > 0 && " · "}
            <button
              type="button"
              className="chat-turn__tokens"
              aria-expanded={open}
              onClick={() => setOpen(!open)}
            >
              {counted}
            </button>
          </>
        )}
        {onOpenTask && (
          <>
            {(parts.length > 0 || counted) && " · "}
            <button type="button" className="chat-turn__tokens" onClick={onOpenTask}>
              Details
            </button>
          </>
        )}
        {onShowRaw && (
          <>
            {(parts.length > 0 || counted || onOpenTask) && " · "}
            <button type="button" className="chat-turn__tokens" onClick={onShowRaw}>
              Raw output
            </button>
          </>
        )}
      </p>
      {open && usage && (
        <p className="chat-turn__usage">
          {tokens(usage.inputTokens)} read
          {usage.cachedInputTokens > 0 && ` (${tokens(usage.cachedInputTokens)} reused)`} ·{" "}
          {tokens(usage.outputTokens)} written
          {turn.ownText && ` · ${turn.ownText}`}
        </p>
      )}
    </>
  );
}
