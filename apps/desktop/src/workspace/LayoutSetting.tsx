import { useState } from "react";
import { Button } from "@plenipo/ui";

import { useWorkspaceIfAny } from "./context";

/**
 * Settings → Personalization: Reset layout (Phase 21, ADR-092 §12). Every panel goes back where
 * it started, pop-outs close, and the sizes start again. The same as Reset layout in a panel's
 * menu.
 */
export function LayoutSetting() {
  const workspace = useWorkspaceIfAny();
  const [done, setDone] = useState(false);
  if (!workspace) return null;
  return (
    <div className="layout-setting">
      <h3>Panels</h3>
      <p className="muted">
        The terminal and Files panels can be moved, resized, and popped out. Reset layout puts them
        all back where they started.
      </p>
      <div className="settings-section__actions">
        <Button
          size="sm"
          icon="panelClose"
          onClick={() => {
            workspace.reset();
            setDone(true);
          }}
        >
          Reset layout
        </Button>
        {done && (
          <span className="muted" role="status">
            The panels are back where they started.
          </span>
        )}
      </div>
    </div>
  );
}
