import { useEffect } from "react";

import { useChat } from "./context";
import { tabFor, type ChatTab, type ChatTarget } from "./tabs";

/**
 * A chat shown outside the Chat panel (the Workers page): live while this is drawn, working as
 * in the panel (messages, Stop, waiting messages), with no tab of its own. Draw it with
 * `ChatWindow`. `null` when there is nothing to show.
 */
export function useShownChat(target: ChatTarget | null): ChatTab | null {
  const { showElsewhere, tab } = useChat();
  const positionId = target?.positionId ?? null;
  const sessionId = target?.sessionId ?? null;
  const title = target?.title ?? "";
  useEffect(() => {
    if (!positionId && !sessionId) return;
    return showElsewhere({ positionId, sessionId, title });
  }, [showElsewhere, positionId, sessionId, title]);
  const own = target ? tabFor(target) : null;
  // The chat as the chats hold it: the same one as a tab in the panel, when it has one.
  return own ? (tab(own.key) ?? own) : null;
}
