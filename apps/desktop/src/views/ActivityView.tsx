import { useEffect, useState } from "react";
import type { LedgerEvent, Task, TaskTimeline, TaskTree } from "@plenipo/types";

import { HANDOFF_OUTCOME_LABEL } from "../agents/format";
import {
  advanceSyntheticTask,
  getTaskTimeline,
  getTaskTree,
  toCommandError,
} from "../api/commands";
import {
  ACTION_LABEL,
  ACTIONS_FOR,
  describeEvent,
  isRejection,
  sourceLabel,
  TASK_STATE_LABEL,
} from "../ledger/format";
import { useLedgerFeed } from "../ledger/useLedgerFeed";
import { formatTime } from "../runtime/format";

type Tab = "tasks" | "events";

function TaskBadge({ task }: { task: Task }) {
  return <span className={`badge badge--task-${task.state}`}>{TASK_STATE_LABEL[task.state]}</span>;
}

/** `step` numbers a task's own trail (1, 2, 3…); the global ledger sequence is in the tooltip. */
function EventRow({ event, step }: { event: LedgerEvent; step?: number }) {
  return (
    <li
      className={`trail__item${isRejection(event) ? " trail__item--rejected" : ""}`}
      data-event-type={event.eventType}
      data-seq={event.seq}
    >
      <span className="trail__seq" title={`Ledger sequence #${event.seq}`}>
        {step !== undefined ? `${step}.` : `#${event.seq}`}
      </span>
      <time>{formatTime(event.createdAt)}</time>
      <span className="trail__text">{describeEvent(event)}</span>
      <span className="trail__source" title={event.source}>
        {sourceLabel(event.source, { capitalize: true })}
      </span>
    </li>
  );
}

/** The task belongs to a Liaison workflow (it carries `metadata.liaison`). */
function inWorkflow(task: Task | undefined): boolean {
  const l = task?.metadata?.liaison;
  return typeof l === "object" && l !== null;
}

/** A Liaison workflow's tasks: the owner's task and every handoff below it. */
function DelegationTree({
  tree,
  selectedTaskId,
  onSelectTask,
}: {
  tree: TaskTree;
  selectedTaskId: string;
  onSelectTask: (id: string) => void;
}) {
  return (
    <>
      <h3>Delegation</h3>
      <p className="muted">
        One workflow
        {tree.correlationId && <> ({tree.correlationId.slice(0, 8)})</>}: the owner&apos;s task and
        the handoffs between workers, in order.
      </p>
      <ol className="tree" aria-label="Delegation tree">
        {tree.nodes.map((n) => (
          <li
            key={n.task.id}
            className="tree__node"
            style={{ paddingLeft: `${n.depth * 20}px` }}
            aria-current={n.task.id === selectedTaskId ? "true" : undefined}
          >
            {n.depth > 0 && <span aria-hidden="true">↳</span>}
            <button type="button" className="link" onClick={() => onSelectTask(n.task.id)}>
              {n.task.objective}
            </button>
            <TaskBadge task={n.task} />
            <span className="muted">
              {n.runtimeLabel ?? n.handoff?.destinationLabel ?? n.task.requestedBy}
              {n.handoff?.replyOutcome && (
                <> · reply: {HANDOFF_OUTCOME_LABEL[n.handoff.replyOutcome]}</>
              )}
            </span>
          </li>
        ))}
      </ol>
    </>
  );
}

export function ActivityView({
  selectedTaskId,
  onSelectTask,
}: {
  selectedTaskId: string | null;
  onSelectTask: (id: string | null) => void;
}) {
  const feed = useLedgerFeed();
  const [tab, setTab] = useState<Tab>("tasks");
  const [timeline, setTimeline] = useState<TaskTimeline | null>(null);
  const [tree, setTree] = useState<TaskTree | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    if (!selectedTaskId) return;
    let cancelled = false;
    getTaskTimeline(selectedTaskId)
      .then((t) => !cancelled && setTimeline(t))
      .catch(() => !cancelled && setTimeline(null));
    return () => {
      cancelled = true;
    };
  }, [selectedTaskId, feed.revision]);

  async function act(taskId: string, action: Parameters<typeof advanceSyntheticTask>[1]) {
    setPending(true);
    setActionError(null);
    try {
      // Stay on the current task; a new step appears under "Sub-tasks".
      await advanceSyntheticTask(taskId, action);
    } catch (reason) {
      setActionError(toCommandError(reason).message);
    } finally {
      setPending(false);
    }
  }

  // Only show a timeline that belongs to the current selection.
  const current = timeline && timeline.task.id === selectedTaskId ? timeline : null;
  const task = current?.task;
  const synthetic = task?.metadata?.synthetic === true;
  const workflow = inWorkflow(task);

  useEffect(() => {
    if (!selectedTaskId || !workflow) return;
    let cancelled = false;
    getTaskTree(selectedTaskId)
      .then((t) => !cancelled && setTree(t))
      .catch(() => !cancelled && setTree(null));
    return () => {
      cancelled = true;
    };
  }, [selectedTaskId, workflow, feed.revision]);
  const delegation =
    workflow && tree && tree.focusId === selectedTaskId && tree.nodes.length > 1 ? tree : null;

  return (
    <section className="view" aria-labelledby="activity-title">
      <h1 id="activity-title">Activity</h1>
      <p className="view__lead">
        Every task change is recorded in the Ledger in order, and the record survives restarts.
      </p>

      <div className="tabs" role="tablist" aria-label="Activity views">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "tasks"}
          className="tabs__tab"
          onClick={() => setTab("tasks")}
        >
          Tasks
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "events"}
          className="tabs__tab"
          onClick={() => setTab("events")}
        >
          All events
        </button>
      </div>

      {feed.status === "error" && (
        <p className="status status--error" role="alert">
          Could not load the ledger: {feed.error}
        </p>
      )}

      {tab === "events" ? (
        feed.events.length === 0 ? (
          <p className="muted">No events recorded yet.</p>
        ) : (
          <ol className="trail" aria-label="All events">
            {feed.events.map((e) => (
              <EventRow key={e.seq} event={e} />
            ))}
          </ol>
        )
      ) : (
        <div className="split">
          <div className="split__list">
            <h2>Tasks</h2>
            {feed.tasks.length === 0 ? (
              <p className="muted">
                No tasks yet. Create a synthetic task from Diagnostics to exercise the Ledger.
              </p>
            ) : (
              <ul className="executions" aria-label="Tasks">
                {feed.tasks.map((t) => (
                  <li key={t.id}>
                    <button
                      type="button"
                      className="execution"
                      aria-current={t.id === selectedTaskId ? "true" : undefined}
                      aria-label={`${t.objective} — ${TASK_STATE_LABEL[t.state]}`}
                      onClick={() => onSelectTask(t.id)}
                    >
                      <span className="execution__label">
                        {t.parentTaskId && <span aria-hidden="true">↳ </span>}
                        {t.objective}
                      </span>
                      <TaskBadge task={t} />
                      <span className="execution__meta">
                        {formatTime(t.createdAt)} · by {sourceLabel(t.requestedBy)}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="split__detail">
            {task && current ? (
              <>
                <div className="detail__header">
                  <div>
                    <h2>{task.objective}</h2>
                    <div className="card__meta">
                      <TaskBadge task={task} /> · requested by {sourceLabel(task.requestedBy)}
                      {task.startedAt !== null && <> · started {formatTime(task.startedAt)}</>}
                      {task.completedAt !== null && <> · finished {formatTime(task.completedAt)}</>}
                    </div>
                    {task.parentTaskId && (
                      <button
                        type="button"
                        className="link"
                        onClick={() => onSelectTask(task.parentTaskId)}
                      >
                        ↑ Parent task
                      </button>
                    )}
                  </div>
                </div>

                {synthetic && ACTIONS_FOR[task.state].length > 0 && (
                  <div className="actions" aria-label="Synthetic task actions">
                    {ACTIONS_FOR[task.state].map((action) => (
                      <button
                        key={action}
                        type="button"
                        className="button button--small"
                        disabled={pending}
                        onClick={() => void act(task.id, action)}
                      >
                        {ACTION_LABEL[action]}
                      </button>
                    ))}
                  </div>
                )}
                {actionError && (
                  <p className="status status--error" role="alert">
                    {actionError}
                  </p>
                )}

                {delegation && selectedTaskId && (
                  <DelegationTree
                    tree={delegation}
                    selectedTaskId={selectedTaskId}
                    onSelectTask={onSelectTask}
                  />
                )}

                {!delegation && current.children.length > 0 && (
                  <>
                    <h3>Sub-tasks</h3>
                    <ul className="children">
                      {current.children.map((c) => (
                        <li key={c.id}>
                          <button type="button" className="link" onClick={() => onSelectTask(c.id)}>
                            {c.objective}
                          </button>{" "}
                          <TaskBadge task={c} />
                        </li>
                      ))}
                    </ul>
                  </>
                )}

                <h3>Activity trail</h3>
                <ol className="trail" aria-label="Activity trail">
                  {current.events.map((e, i) => (
                    <EventRow key={e.seq} event={e} step={i + 1} />
                  ))}
                </ol>
              </>
            ) : (
              <p className="muted">Select a task to see its complete activity trail.</p>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
