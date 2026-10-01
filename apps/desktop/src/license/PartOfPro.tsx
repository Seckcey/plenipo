import type { ReactNode } from "react";
import { Button } from "@plenipo/ui";

import type { Go } from "../components/views";

/**
 * A "part of Pro" note (Phase 11A): what Free has and what Pro adds, and the way to Settings →
 * License. Shown where a Free copy stops something new; nothing already made is hidden or taken
 * away.
 */
export function PartOfPro({ go, children }: { go: Go; children: ReactNode }) {
  return (
    <div className="notice-box part-of-pro" role="note">
      <p>
        <strong>Part of Plenipo Pro.</strong> {children}
      </p>
      <div className="settings-section__actions">
        <Button size="sm" icon="key" onClick={() => go({ view: "settings", id: "license" })}>
          Open Settings → License
        </Button>
      </div>
    </div>
  );
}
