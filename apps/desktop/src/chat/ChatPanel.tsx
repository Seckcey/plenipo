import { useRef, type KeyboardEvent } from "react";
import { EmptyState, Icon, IconButton, cx } from "@plenipo/ui";

import type { Go } from "../components/views";
import { ChatWindow } from "./ChatWindow";
import { copyText } from "./clipboard";
import { useChat } from "./context";
import { isOver } from "./model";
import { shownTabs, slotOf, type ChatTab } from "./tabs";

/**
 * The Chat panel (ADR-200): a tab for each agent you talk to or watch, and its chat. Side by side
 * shows up to four at once, each streaming as it works. The panel sits in a dock or pops out into
 * its own window, like Terminal and Files.
 */
export function ChatPanel({ go }: { go: Go }) {
  const chat = useChat();
  const { tabs, active, sideBySide } = chat.tabs;
  const strip = useRef<HTMLDivElement>(null);
  const shown = shownTabs(chat.tabs);

  if (tabs.length === 0) {
    return (
      <div className="chat-panel chat-panel--empty">
        <EmptyState
          icon="chat"
          title="No chats open"
          action={
            <button
              type="button"
              className="ui-button ui-button--secondary ui-button--sm"
              onClick={() => go({ view: "organization", id: null })}
            >
              Open the Organization map
            </button>
          }
        >
          On the map, choose an agent, then Chat. You can talk to its agent and watch it work here,
          live.
        </EmptyState>
      </div>
    );
  }

  // Left and Right move between the tabs, as in any row of tabs.
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    const at = tabs.findIndex((t) => t.key === active);
    const next = tabs[(at + (e.key === "ArrowRight" ? 1 : tabs.length - 1)) % tabs.length];
    if (!next) return;
    e.preventDefault();
    chat.show(next.key);
    strip.current?.querySelector<HTMLElement>(`[data-tab="${CSS.escape(next.key)}"]`)?.focus();
  };

  return (
    <div className="chat-panel">
      <div className="chat-panel__bar">
        <div ref={strip} className="chat-tabs" role="tablist" aria-label="Chats" onKeyDown={onKey}>
          {tabs.map((tab) => (
            <TabButton key={tab.key} tab={tab} selected={tab.key === active} />
          ))}
        </div>
        <IconButton
          icon="columns"
          label={sideBySide ? "One chat at a time" : "Chats side by side"}
          pressed={sideBySide}
          disabled={tabs.length < 2}
          onClick={() => chat.setSideBySide(!sideBySide)}
        />
      </div>
      <div className={cx("chat-panel__body", sideBySide && shown.length > 1 && "is-side-by-side")}>
        {shown.map((tab) => (
          <div
            key={tab.key}
            className="chat-panel__window"
            role="tabpanel"
            aria-label={tab.title}
            id={`chat-panel-${tab.key}`}
          >
            {slotOf(chat.tabs, tab.key) !== null ? (
              <ChatAway tab={tab} />
            ) : (
              <ChatWindow
                tab={tab}
                compact={sideBySide && shown.length > 1}
                // A link an agent wrote never opens by itself: choosing it copies its address.
                onOpenLink={(url) => void copyText(url)}
                onPopOut={chat.canPopOut ? () => chat.popOut(tab.key) : undefined}
              />
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

/** A chat that is in a window of its own (ADR-203): where it is, and a way back. */
function ChatAway({ tab }: { tab: ChatTab }) {
  const chat = useChat();
  return (
    <div className="chat-panel__away">
      <EmptyState
        icon="external"
        title={`${tab.title}'s chat is in its own window`}
        action={
          <span className="chat-panel__away-actions">
            <button
              type="button"
              className="ui-button ui-button--secondary ui-button--sm"
              onClick={() => chat.focusWindow(tab.key)}
            >
              Show its window
            </button>
            <button
              type="button"
              className="ui-button ui-button--quiet ui-button--sm"
              onClick={() => chat.putBack(tab.key)}
            >
              Put back here
            </button>
          </span>
        }
      >
        You talk to it and watch it there. It comes back here when you put it back or close its
        window.
      </EmptyState>
    </div>
  );
}

function TabButton({ tab, selected }: { tab: ChatTab; selected: boolean }) {
  const chat = useChat();
  const conversation = chat.conversation(tab);
  const last = conversation?.turns[conversation.turns.length - 1];
  const working = chat.busy(tab);
  const failed = last !== undefined && isOver(last) && last.state === "failed";
  return (
    <span className={cx("chat-tab", selected && "is-selected")}>
      <button
        type="button"
        role="tab"
        className="chat-tab__name"
        data-tab={tab.key}
        aria-selected={selected}
        aria-controls={`chat-panel-${tab.key}`}
        tabIndex={selected ? 0 : -1}
        title={working ? `${tab.title}: working` : tab.title}
        onClick={() => chat.show(tab.key)}
      >
        {working ? (
          <span className="chat-tab__dot is-working" aria-label="working" role="img" />
        ) : failed ? (
          <Icon name="alert" size={12} label="could not finish" />
        ) : null}
        {tab.title}
      </button>
      <IconButton
        icon="close"
        label={`Close the chat with ${tab.title}`}
        className="chat-tab__close"
        onClick={() => chat.close(tab.key)}
      />
    </span>
  );
}
