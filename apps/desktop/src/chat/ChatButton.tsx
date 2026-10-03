import { Button } from "@plenipo/ui";

import { useWorkspaceIfAny } from "../workspace/context";
import { useChatIfAny } from "./context";

export const CHAT_BUTTON_ID = "chat-button";

/**
 * The Chat button in the top bar (ADR-200): shows or hides the Chat panel, or brings its window
 * to the front when it is popped out. It says how many agents are working in the open chats.
 */
export function ChatButton() {
  const ws = useWorkspaceIfAny();
  const chat = useChatIfAny();
  if (!ws || !chat) return null;
  const shown = ws.shown("chat");
  const working = chat.tabs.tabs.filter((t) => chat.busy(t)).length;
  return (
    <Button
      id={CHAT_BUTTON_ID}
      size="sm"
      variant="quiet"
      icon="chat"
      aria-pressed={shown}
      title={`${shown ? "Hide" : "Show"} Chat${working > 0 ? ` (${working} working)` : ""}`}
      onClick={() => ws.toggle("chat")}
    >
      Chat{working > 0 ? ` · ${working}` : ""}
    </Button>
  );
}
