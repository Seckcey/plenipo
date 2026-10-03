import { useEffect, useState } from "react";
import { Icon, IconButton, StatusDot, cx, type Status } from "@plenipo/ui";

import { useAgents } from "../agents/useAgents";
import { liaisonInfo } from "../agents/store";
import { useNow } from "../runtime/useNow";
import { EFFORT_LABEL } from "../routing/format";
import { usePanelWindow } from "../workspace/context";
import { Composer } from "./Composer";
import { useChat } from "./context";
import { isOver, type ChatTurn } from "./model";
import { PlanPanel } from "./PlanPanel";
import type { ChatTab } from "./tabs";
import { Transcript } from "./Transcript";
import { elapsed } from "./words";

/** Wide enough for the plan beside the conversation. */
const WIDE = 720;

/** An element's width, measured in its own window (a popped-out panel is in another one). */
function useWidth(el: HTMLElement | null, win: Window): number {
  const [width, setWidth] = useState(0);
  useEffect(() => {
    if (!el) return;
    // Observed from the element's own window, which draws it (ADR-092 pop-outs).
    const own = win as Window & { ResizeObserver?: typeof ResizeObserver };
    const Observer =
      own.ResizeObserver ?? (typeof ResizeObserver === "undefined" ? undefined : ResizeObserver);
    // Without one (an old system, or a test page), the chat keeps its narrow layout.
    if (!Observer) return;
    const watch = new Observer((entries: ResizeObserverEntry[]) => {
      const box = entries[0]?.contentRect;
      if (box) setWidth(Math.round(box.width));
    });
    watch.observe(el);
    return () => watch.disconnect();
  }, [el, win]);
  return width;
}

function stateOf(turn: ChatTurn | undefined, busy: boolean): { status: Status; words: string } {
  if (busy && turn?.state === "waiting")
    return { status: "pending", words: "Waiting for its team" };
  if (busy) return { status: "ok", words: "Working" };
  if (turn?.state === "failed") return { status: "error", words: "Could not finish" };
  return { status: "offline", words: "Ready" };
}

/** How long the work now has taken, counting up. */
function Since({ from }: { from: number }) {
  const now = useNow(1000);
  return <span className="chat-head__time">{elapsed(now - from)}</span>;
}

/**
 * One chat with one agent (ADR-200), as in Claude Code: what it says, streamed as it writes,
 * what it is doing now, and a box to write in. Its plan (the tasks it handed to its team) shows
 * beside it when there is room, and over it on request when there is not.
 */
export function ChatWindow({
  tab,
  compact = false,
  onOpenLink,
}: {
  tab: ChatTab;
  /** Drawn small, beside others. */
  compact?: boolean;
  onOpenLink?: ((url: string) => void) | undefined;
}) {
  const chat = useChat();
  const agents = useAgents();
  const [el, setEl] = useState<HTMLElement | null>(null);
  const width = useWidth(el, usePanelWindow());
  const [planChoice, setPlanChoice] = useState<boolean | null>(null);

  const sessionId = chat.sessionOf(tab);
  const session = sessionId ? agents.state.sessions[sessionId] : undefined;
  const conversation = chat.conversation(tab);
  const turns = conversation?.turns ?? [];
  const last = turns[turns.length - 1];
  const busy = chat.busy(tab);
  const runtime = agents.state.runtimes.find((r) => r.id === session?.runtimeId);
  const origin = liaisonInfo(session).origin;

  const wide = width >= WIDE && !compact;
  const planShown = planChoice ?? wide;
  const state = stateOf(last, busy);

  // Which AI tool, model, and effort answer: known once there is a conversation.
  const facts = session
    ? [
        runtime?.label ?? session.runtimeId,
        session.model ?? "its usual model",
        ...(session.effort ? [`${EFFORT_LABEL[session.effort]} effort`] : []),
      ]
    : [];
  const disabledReason =
    origin === "handoff"
      ? `${tab.title} takes its work from its lead, so you can watch it here but not message it. To change what it does, message its lead.`
      : null;
  const textOnly = runtime !== undefined && !runtime.usesTools;

  return (
    <section
      ref={setEl}
      className={cx(
        "chat",
        compact && "chat--compact",
        wide && "chat--wide",
        planShown && "chat--plan",
      )}
      aria-label={`Chat with ${tab.title}`}
    >
      <header className="chat-head">
        <h2 className="chat-head__title" title={tab.title}>
          {tab.title}
        </h2>
        {runtime && <span className="chat-head__chip">{runtime.label}</span>}
        <span className="chat-head__spacer" />
        <StatusDot status={state.status} label={state.words} className="chat-head__state" />
        {busy && last && !isOver(last) && <Since from={last.startedAt} />}
        <IconButton
          icon="list"
          label={planShown ? "Hide its tasks" : "Show its tasks"}
          pressed={planShown}
          onClick={() => setPlanChoice(!planShown)}
        />
      </header>
      <div className="chat-body">
        <div className="chat-main">
          <Transcript session={conversation} title={tab.title} onOpenLink={onOpenLink} />
          {chat.note(tab.key) && (
            <p className="chat-note chat-window__note" role="status">
              <Icon name="info" size={14} />
              {chat.note(tab.key)}
            </p>
          )}
          {textOnly && (
            <p className="chat-text-only">
              {runtime.label} writes answers only: it cannot save files or run programs. To have{" "}
              {tab.title} save files, give it an AI tool that can, on its Model tab.
            </p>
          )}
          <Composer
            title={tab.title}
            busy={busy}
            sending={chat.sending(tab.key)}
            queued={chat.queued(tab.key)}
            problem={chat.problem(tab.key)}
            disabledReason={disabledReason}
            facts={facts}
            onSend={(text) => chat.send(tab.key, text)}
            onStop={() => void chat.stop(tab.key)}
            onUnqueue={(i) => chat.unqueue(tab.key, i)}
            onDismissProblem={() => chat.dismissProblem(tab.key)}
          />
        </div>
        {planShown && (
          <PlanPanel
            turn={last}
            title={tab.title}
            positionId={tab.positionId}
            onOpenWorker={(id, title) => chat.open({ sessionId: id, title })}
          />
        )}
      </div>
    </section>
  );
}
