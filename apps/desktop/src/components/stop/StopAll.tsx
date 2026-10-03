/**
 * Stop all work (Phase 25, item 3.4; ADR-199): one red button on every page that shows work and
 * on the canvas's toolbar. In every organization it stops browser, screen, and server control
 * and every task running now, and holds all waiting work until you press Allow again. An
 * emergency button: it doesn't ask first. While work is stopped, it is **Allow again**.
 */
import { Button } from "@plenipo/ui";

import type { Control } from "../../control/useControl";

export function StopAllButton({ control, size = "sm" }: { control: Control; size?: "sm" | "md" }) {
  const stopped = control.status?.stopped ?? false;
  if (stopped) {
    return (
      <Button
        size={size}
        variant="primary"
        className="stop-all-work stop-all-work--allow"
        disabled={control.pending}
        title="Let work start again: the tasks that waited start"
        onClick={() => void control.allow()}
      >
        Allow again
      </Button>
    );
  }
  return (
    <Button
      size={size}
      variant="danger"
      icon="stop"
      className="stop-all-work"
      disabled={control.pending || !control.status}
      title="Stop all work now, in every organization: every task, and the browser, desktop, and servers. Nothing new starts until you press Allow again."
      onClick={() => void control.stopAll()}
    >
      Stop all
    </Button>
  );
}
