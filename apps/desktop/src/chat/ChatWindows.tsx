/**
 * Each chat that has a window of its own (ADR-203), drawn into that window by this page: the same
 * chat as in the Chat panel, live, with its message box, and Put back in its header. The window
 * itself can call nothing (ADR-092 §15): this page makes every call.
 */
import { createPortal } from "react-dom";

import { PanelWindowContext, useWorkspaceIfAny } from "../workspace/context";
import { ChatWindow } from "./ChatWindow";
import { copyText } from "./clipboard";
import { useChat } from "./context";
import { tabInSlot } from "./tabs";

export function ChatWindows() {
  const ws = useWorkspaceIfAny();
  const chat = useChat();
  if (!ws) return null;
  return (
    <>
      {ws.popUps.map((u) => {
        const t = u.target;
        if (t.kind !== "chat") return null;
        const tab = tabInSlot(chat.tabs, t.slot);
        if (!tab) return null;
        return createPortal(
          <PanelWindowContext.Provider value={u.win}>
            <div className="chat-own-window">
              <ChatWindow
                tab={tab}
                // A link an agent wrote never opens by itself: choosing it copies its address.
                onOpenLink={(url) => void copyText(url)}
                onPutBack={() => chat.putBack(tab.key)}
              />
            </div>
          </PanelWindowContext.Provider>,
          u.body,
          `chat-window-${t.slot}`,
        );
      })}
    </>
  );
}
