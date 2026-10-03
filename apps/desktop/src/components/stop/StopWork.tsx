/**
 * Stop, on every worker (Phase 25, item 3.3): one part that asks "Stop Alex's task?" first, then
 * stops the work running or waiting now. The canvas tiles, the details panel, the Worker and
 * Task pages, and Home's "Who's working" rows all use it.
 */
import { useContext, useState } from "react";
import { Button } from "@plenipo/ui";

import { cancelAgentTurn, toCommandError } from "../../api/commands";
import { AgentsContext } from "../../agents/context";
import { ConfirmDialog } from "../org/Modal";
import { stopQuestion, type StopTarget } from "./stopWork";

/** The question, then Stop: ends each task's turn. */
export function StopConfirm({
  who,
  work,
  fullTime = false,
  onClose,
}: {
  /** Whose work: "Alex", "Senior Developer". */
  who: string;
  work: readonly StopTarget[];
  /** A full-time agent: its conversation stays. */
  fullTime?: boolean;
  onClose: () => void;
}) {
  const agents = useContext(AgentsContext);
  // As Cancel task on the Workers page does (it also updates that page), when it is here.
  const cancel: (id: string) => Promise<unknown> = agents?.cancel ?? cancelAgentTurn;
  const stop = async () => {
    const results = await Promise.allSettled(work.map((w) => cancel(w.sessionId)));
    const failed = results.flatMap((r) =>
      r.status === "rejected" ? [toCommandError(r.reason).message] : [],
    );
    if (failed.length > 0) return [...new Set(failed)].join(" ");
    onClose();
    return null;
  };
  return (
    <ConfirmDialog
      title={stopQuestion(who, work.length)}
      message={
        <>
          <ul className="stop-work__list" aria-label="What stops">
            {work.map((w) => (
              <li key={w.sessionId}>{w.objective || "(no objective)"}</li>
            ))}
          </ul>
          <p>
            {fullTime
              ? "Only this task stops. Its conversation stays, so you can give it a new objective after."
              : "Each task stops now."}{" "}
            Whoever asked for it is told it was stopped.
          </p>
        </>
      }
      confirmLabel="Stop"
      cancelLabel="Keep working"
      danger
      onConfirm={stop}
      onCancel={onClose}
    />
  );
}

/** A Stop button that asks first; nothing shows while there is nothing to stop. */
export function StopButton({
  who,
  work,
  fullTime = false,
  size = "sm",
  describedBy,
}: {
  who: string;
  work: readonly StopTarget[];
  fullTime?: boolean;
  size?: "sm" | "md";
  /** The line that says what it does (`aria-describedby`). */
  describedBy?: string;
}) {
  const [asking, setAsking] = useState(false);
  if (work.length === 0) return null;
  return (
    <>
      <Button
        size={size}
        variant="danger"
        icon="stop"
        title={stopQuestion(who, work.length).replace(/\?$/, "")}
        aria-label={`Stop ${who}`}
        aria-describedby={describedBy}
        onClick={(e) => {
          // Inside a row or a tile: Stop doesn't open it too.
          e.stopPropagation();
          setAsking(true);
        }}
      >
        Stop
      </Button>
      {asking && (
        <StopConfirm who={who} work={work} fullTime={fullTime} onClose={() => setAsking(false)} />
      )}
    </>
  );
}
