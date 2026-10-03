import { useEffect } from "react";
import { Button } from "@plenipo/ui";

import { useWorkspaceIfAny } from "../workspace/context";
import { shortcut } from "../system/words";

export const FILES_BUTTON_ID = "files-button";

/**
 * The Files button in the top bar (Phase 21, ADR-092 §13): shows or hides the Files panel, or
 * brings its window to the front when it is popped out. Ctrl+Shift+E does the same, as in Visual
 * Studio Code.
 */
export function FilesButton() {
  const ws = useWorkspaceIfAny();
  const toggle = ws?.toggle;
  useEffect(() => {
    if (!toggle) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === "e") {
        e.preventDefault();
        toggle("files");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle]);
  if (!ws) return null;
  const shown = ws.shown("files");
  return (
    <Button
      id={FILES_BUTTON_ID}
      size="sm"
      variant="quiet"
      icon="projects"
      aria-pressed={shown}
      aria-keyshortcuts="Control+Shift+E"
      title={`${shown ? "Hide" : "Show"} Files (${shortcut(["ctrl", "shift"], "E")})`}
      onClick={() => ws.toggle("files")}
    >
      Files
    </Button>
  );
}
