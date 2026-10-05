import { useEffect, useMemo, useState } from "react";
import type { AgentSession } from "@plenipo/types";
import { Button, StatusPill, useStoredState } from "@plenipo/ui";

import { isRunning, isWaiting, liaisonInfo } from "../agents/store";
import { useAgents } from "../agents/useAgents";
import { toCommandError } from "../api/commands";
import { copyText } from "../chat/clipboard";
import { ChatWindow } from "../chat/ChatWindow";
import { useChat } from "../chat/context";
import { positionChatTarget } from "../chat/positionTarget";
import type { ChatTarget } from "../chat/tabs";
import { useShownChat } from "../chat/useShownChat";
import { PILL_TONE, TASK_TONE } from "../components/tones";
import type { Go } from "../components/views";
import { STATUS_LABEL, plural } from "../org/format";
import { useOrganization } from "../org/useOrganization";
import { formatTime } from "../runtime/format";
import { StartConversation } from "./StartConversation";
import { talksOf, type Talks } from "./talks";
import { buildTree, pathTo, readSelection, START } from "./tree";
import { WorkersTree } from "./WorkersTree";

const CLOSED_KEY = "plenipo.workers.closed";
const isKeys = (v: unknown): v is string[] =>
  Array.isArray(v) && v.every((x) => typeof x === "string");

/** Who it works with (B5): who asked it for work, and whom it asked, each one a way to them. */
function TalksWith({
  talks,
  titleOf,
  onSelect,
}: {
  talks: Talks;
  titleOf: (positionId: string) => string;
  onSelect: (positionId: string) => void;
}) {
  if (talks.askedBy.length === 0 && talks.asked.length === 0) return null;
  const list = (items: Talks["asked"]) =>
    items.map((t, i) => (
      <span key={t.positionId}>
        {i > 0 && ", "}
        <button type="button" className="link" onClick={() => onSelect(t.positionId)}>
          {titleOf(t.positionId)}
        </button>{" "}
        ({plural(t.tasks, "task")})
      </span>
    ));
  return (
    <div className="workers__bar workers__talks" aria-label="Talks with">
      {talks.askedBy.length > 0 && <span>Asked by {list(talks.askedBy)}</span>}
      {talks.asked.length > 0 && <span>Asked {list(talks.asked)}</span>}
    </div>
  );
}

function SessionState({ session }: { session: AgentSession }) {
  if (isRunning(session)) return <StatusPill status={TASK_TONE.running} label="Working" />;
  if (isWaiting(session)) return <StatusPill status={TASK_TONE.blocked} label="Waiting" />;
  if (session.state === "closed") return <StatusPill status={PILL_TONE.muted} label="Closed" />;
  return <StatusPill status={TASK_TONE.succeeded} label="Ready" />;
}

/**
 * The Workers page (I4): everyone in the organization shown in the top bar, as a tree of its
 * departments, projects, and teams, with each one's state. Selecting a worker shows its chat
 * beside the tree, as it happens: the same chat as in the Chat panel (ADR-200), with its words
 * as they are written and its thinking and steps as short lines you can open. Below the tree,
 * conversations outside the organization, and a way to start one.
 */
export function WorkersPage({
  selectedSessionId,
  onSelectSession,
  onOpenRuntimes,
  onOpenPosition,
  onOpenPage,
}: {
  /** What is selected: `position:<id>`, `start`, or a conversation's ID (see `readSelection`). */
  selectedSessionId: string | null;
  onSelectSession: (id: string | null) => void;
  /** Opens the AI tools page, at that AI tool's card. */
  onOpenRuntimes: (runtimeId?: string) => void;
  /** Shows a position on the Organization page. */
  onOpenPosition?: ((positionId: string) => void) | undefined;
  /** Opens a page (kept for the app's links into this page). */
  onOpenPage?: Go | undefined;
}) {
  const { snapshot, status, error } = useOrganization();
  const agents = useAgents();
  const chat = useChat();
  const selection = readSelection(selectedSessionId);
  const tree = useMemo(() => (snapshot ? buildTree(snapshot) : []), [snapshot]);
  const [closedList, setClosedList] = useStoredState<string[]>(CLOSED_KEY, [], isKeys);
  const closed = useMemo(() => new Set(closedList), [closedList]);
  const [problem, setProblem] = useState<string | null>(null);

  const session = selection?.kind === "session" ? agents.state.sessions[selection.id] : undefined;
  // The position the selection belongs to: chosen in the tree, or the one a conversation is for.
  const positionId =
    selection?.kind === "position" ? selection.id : (liaisonInfo(session).positionId ?? null);
  const position = positionId ? snapshot?.positions.find((p) => p.id === positionId) : undefined;

  // A position chosen elsewhere opens the groups that hold it.
  const path = useMemo(() => (positionId ? pathTo(tree, positionId) : []), [tree, positionId]);
  useEffect(() => {
    if (path.some((key) => closed.has(key))) {
      setClosedList(closedList.filter((key) => !path.includes(key)));
    }
  }, [path, closed, closedList, setClosedList]);

  const target: ChatTarget | null =
    selection?.kind === "position"
      ? position
        ? positionChatTarget(position)
        : null
      : selection?.kind === "session"
        ? { sessionId: selection.id, title: position?.title ?? session?.title ?? "Conversation" }
        : null;
  const tab = useShownChat(target);

  // Conversations outside the organization: no position, newest first.
  const others = agents.state.order
    .map((id) => agents.state.sessions[id])
    .filter((s): s is AgentSession => s !== undefined && !liaisonInfo(s).positionId);

  async function closeConversation(id: string) {
    setProblem(null);
    try {
      await agents.close(id);
    } catch (reason) {
      setProblem(toCommandError(reason).message);
    }
  }

  return (
    <section className="view workers-page" aria-labelledby="workers-title">
      <h1 id="workers-title">Workers</h1>
      <p className="view__lead">
        Everyone in your organization and what each is doing. Select one to see its chat as it
        happens: its words as it writes them, and each step as a short line you can open.
      </p>

      {agents.state.status === "error" && (
        <p className="status status--error" role="alert">
          Could not load the workers: {agents.state.error}
        </p>
      )}
      {agents.state.notices.length > 0 && (
        <ul className="notices" aria-label="Worker notices">
          {agents.state.notices.map((n) => (
            <li key={n} className="muted">
              {n}
            </li>
          ))}
        </ul>
      )}

      <div className="workers">
        <div className="workers__side">
          <nav aria-label="Your organization">
            {status === "loading" && <p className="muted">Loading your organization…</p>}
            {status === "error" && (
              <p className="status status--error" role="alert">
                Could not load your organization: {error}
              </p>
            )}
            {snapshot && tree.length === 0 && (
              <p className="muted">No one is in your organization yet.</p>
            )}
            {snapshot && (
              <WorkersTree
                rows={tree}
                snapshot={snapshot}
                selectedPosition={positionId}
                selectedSession={selection?.kind === "session" ? selection.id : null}
                closed={closed}
                onToggle={(key) =>
                  setClosedList(
                    closed.has(key) ? closedList.filter((k) => k !== key) : [...closedList, key],
                  )
                }
                onSelectPosition={(id) => onSelectSession(`position:${id}`)}
                onSelectSession={onSelectSession}
              />
            )}
          </nav>

          <section className="workers__others" aria-labelledby="workers-others">
            <h2 id="workers-others">Other conversations</h2>
            {others.length > 0 && (
              <ul className="workers-others" aria-label="Other conversations">
                {others.map((s) => (
                  <li key={s.id}>
                    <button
                      type="button"
                      className="execution"
                      aria-current={
                        selection?.kind === "session" && selection.id === s.id ? "true" : undefined
                      }
                      onClick={() => onSelectSession(s.id)}
                    >
                      <span className="execution__label">
                        {liaisonInfo(s).origin === "handoff" && <span aria-hidden="true">↳ </span>}
                        {s.title}
                      </span>
                      <SessionState session={s} />
                      <span className="execution__meta">
                        {agents.state.runtimes.find((r) => r.id === s.runtimeId)?.label ??
                          s.runtimeId}{" "}
                        · {formatTime(s.updatedAt)}
                        {liaisonInfo(s).origin === "handoff" && " · asked by another worker"}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <Button
              size="sm"
              variant="quiet"
              icon="plus"
              aria-pressed={selection?.kind === "start"}
              onClick={() => onSelectSession(START)}
            >
              Start a conversation outside your organization
            </Button>
          </section>
        </div>

        <div className="workers__chat">
          {selection?.kind === "start" ? (
            <StartConversation onStarted={onSelectSession} onOpenRuntimes={onOpenRuntimes} />
          ) : tab && target ? (
            <>
              {session && !liaisonInfo(session).positionId && session.state === "open" && (
                <div className="workers__bar">
                  {!isRunning(session) && !isWaiting(session) && (
                    <Button
                      size="sm"
                      variant="quiet"
                      onClick={() => void closeConversation(session.id)}
                    >
                      Close conversation
                    </Button>
                  )}
                  {problem && (
                    <p className="status status--error" role="alert">
                      {problem}
                    </p>
                  )}
                </div>
              )}
              {positionId && snapshot && (
                <TalksWith
                  talks={talksOf(agents.state.sessions, positionId)}
                  titleOf={(id) =>
                    snapshot.positions.find((p) => p.id === id)?.title ?? "someone who left"
                  }
                  onSelect={(id) => onSelectSession(`position:${id}`)}
                />
              )}
              <ChatWindow
                tab={tab}
                onOpenLink={(url) => void copyText(url)}
                onPopOut={chat.canPopOut ? () => chat.openWindow(target) : undefined}
                // The other side of an exchange opens here, beside the tree.
                onOpenConversation={(id) => onSelectSession(id)}
                onOpenTask={onOpenPage ? (id) => onOpenPage({ view: "task", id }) : undefined}
              />
            </>
          ) : position ? (
            <div className="workers__empty">
              <h2>{position.title}</h2>
              <p className="muted">
                {position.staffing === "persistent" && !position.agent
                  ? "Vacant: no one holds this position yet, so it has no chat."
                  : `${STATUS_LABEL[position.status]}. It has no chat yet.`}
              </p>
              {onOpenPosition && (
                <Button size="sm" variant="quiet" onClick={() => onOpenPosition(position.id)}>
                  Show it on the map
                </Button>
              )}
            </div>
          ) : (
            <p className="muted workers__empty">
              Select a worker to see its chat, or start a conversation outside your organization.
            </p>
          )}
        </div>
      </div>
    </section>
  );
}
