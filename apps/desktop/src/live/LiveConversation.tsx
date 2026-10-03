/**
 * The live conversation (Phase 25, item 3.1): a worker's words as it types them, its steps in
 * plain words, and how far along it is — like a chat in the browser or the AI tool's own window.
 * The words come live from Claude Code, Grok, Kimi, and the paid AI tools; Codex sends whole
 * messages. The Watch tab, the Task and Worker pages, and the details panel show it.
 */
import { useContext, useEffect, useLayoutEffect, useRef } from "react";
import type { AgentActivity } from "@plenipo/types";

import { AgentsContext } from "../agents/context";
import { activityItems } from "../agents/store";
import { useNow } from "../runtime/useNow";
import { liveProgress, progressWords, stepWords } from "./words";

/** One line of the conversation. */
type Line =
  | { key: string; kind: "words"; text: string; typing: boolean }
  | { key: string; kind: "step"; text: string }
  | { key: string; kind: "problem"; text: string }
  | { key: string; kind: "note"; text: string };

function linesOf(activity: readonly AgentActivity[]): Line[] {
  const lines: Line[] = [];
  const items = activityItems([...activity]);
  items.forEach((item, i) => {
    if (item.kind === "streaming") {
      // Still typing only when nothing came after it.
      lines.push({ key: item.key, kind: "words", text: item.text, typing: i === items.length - 1 });
      return;
    }
    const e = item.activity.event;
    switch (e.type) {
      case "message":
        lines.push({ key: item.key, kind: "words", text: e.text, typing: false });
        break;
      case "toolUse":
        lines.push({ key: item.key, kind: "step", text: stepWords(e.tool, e.summary) });
        break;
      case "toolResult":
        if (e.isError) {
          lines.push({
            key: item.key,
            kind: "problem",
            text: `That didn't work${e.summary ? `: ${e.summary}` : ""}`,
          });
        }
        break;
      case "notice":
        lines.push({ key: item.key, kind: "note", text: e.text });
        break;
      case "memoryShortened":
        lines.push({ key: item.key, kind: "note", text: e.detail });
        break;
      default:
        // Its plan and usage are in the progress line; the start of its conversation says
        // nothing new.
        break;
    }
  });
  return lines;
}

export function LiveConversation({
  taskId,
  sessionId,
  startedAt,
  running,
  who,
  lines: last,
}: {
  taskId: string;
  /** Its conversation: read once, so what was said before this page opened shows too. */
  sessionId: string | null;
  startedAt: number | null;
  running: boolean;
  /** Whose words ("Senior Developer"), for the label. */
  who: string;
  /** Only the last few lines (the details panel shows 3). */
  lines?: number;
}) {
  const agents = useContext(AgentsContext);
  const activity = agents?.state.activity[taskId] ?? [];
  const loaded = sessionId ? agents?.state.loaded[sessionId] === true : true;
  const load = agents?.loadSession;
  useEffect(() => {
    if (sessionId && !loaded && load) void load(sessionId).catch(() => undefined);
  }, [sessionId, loaded, load]);
  const now = useNow(running ? 15_000 : 60_000);
  const all = linesOf(activity);
  const shown = last !== undefined ? all.slice(-last) : all;
  const progress = progressWords(liveProgress(activity), startedAt, now);

  // Keeps the newest line in view, unless you scrolled up to read.
  const box = useRef<HTMLOListElement>(null);
  const atEnd = useRef(true);
  useLayoutEffect(() => {
    const el = box.current;
    if (el && atEnd.current) el.scrollTop = el.scrollHeight;
  });

  return (
    <section
      className={`live${last !== undefined ? " live--short" : ""}`}
      aria-label={`${who}'s live conversation`}
    >
      {progress && (
        <p className="live__progress" aria-label="How far along">
          {running && <span className="live__dot" aria-hidden="true" />}
          {progress}
        </p>
      )}
      {shown.length === 0 ? (
        <p className="muted live__empty">
          {running ? `${who} is starting…` : "Nothing was said yet."}
        </p>
      ) : (
        <ol
          ref={box}
          className="live__lines"
          role="log"
          aria-label={`What ${who} says and does`}
          onScroll={(e) => {
            const el = e.currentTarget;
            atEnd.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
          }}
        >
          {shown.map((l) => (
            <li key={l.key} className={`live__line live__line--${l.kind}`}>
              {l.kind === "words" ? (
                <p className="live__words">
                  {l.text}
                  {l.typing && running && <span className="live__caret" aria-hidden="true" />}
                </p>
              ) : (
                l.text
              )}
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
