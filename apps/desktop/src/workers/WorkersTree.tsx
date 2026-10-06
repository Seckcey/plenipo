import type { ReactNode } from "react";
import type { OrgSnapshot } from "@plenipo/types";
import { Icon, StatusDot, cx } from "@plenipo/ui";

import { POSITION_STATUS } from "../org/cards";
import { STATUS_LABEL, runtimeLabel } from "../org/format";
import { busyCount, type AgentRow, type GroupRow, type TreeRow } from "./tree";

/** First line of a task, short enough for a row. */
function firstLine(text: string, max = 80): string {
  const line = text.trim().split("\n")[0] ?? "";
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/**
 * The organization as a tree (I4): departments and projects open and close; selecting a worker
 * shows its chat. An on-call worker's tasks being worked on now are listed under it.
 */
export function WorkersTree({
  rows,
  snapshot,
  selectedPosition,
  selectedSession,
  closed,
  onToggle,
  onSelectPosition,
  onSelectSession,
}: {
  rows: readonly TreeRow[];
  snapshot: OrgSnapshot;
  selectedPosition: string | null;
  selectedSession: string | null;
  /** The groups closed now (all are open until closed). */
  closed: ReadonlySet<string>;
  onToggle: (key: string) => void;
  onSelectPosition: (positionId: string) => void;
  onSelectSession: (sessionId: string) => void;
}) {
  const draw = (list: readonly TreeRow[]) =>
    list.map((row) =>
      row.kind === "agent" ? (
        <Agent
          key={row.key}
          row={row}
          snapshot={snapshot}
          selected={row.position.id === selectedPosition}
          selectedSession={selectedSession}
          onSelect={() => onSelectPosition(row.position.id)}
          onSelectSession={onSelectSession}
        />
      ) : (
        <Group key={row.key} row={row} open={!closed.has(row.key)} onToggle={onToggle}>
          {draw(row.children)}
        </Group>
      ),
    );
  return <ul className="workers-tree">{draw(rows)}</ul>;
}

function Group({
  row,
  open,
  onToggle,
  children,
}: {
  row: GroupRow;
  open: boolean;
  onToggle: (key: string) => void;
  children: ReactNode;
}) {
  const busy = busyCount(row);
  return (
    <li className={cx("workers-tree__item", `workers-tree__item--${row.kind}`)}>
      <button
        type="button"
        className="workers-tree__group"
        aria-expanded={open}
        onClick={() => onToggle(row.key)}
      >
        <Icon name={open ? "chevronDown" : "chevronRight"} size={14} />
        <span className="workers-tree__name">{row.name}</span>
        {!open && busy > 0 && <span className="workers-tree__count">{busy} working</span>}
      </button>
      {open && row.children.length > 0 && <ul className="workers-tree__children">{children}</ul>}
    </li>
  );
}

function Agent({
  row,
  snapshot,
  selected,
  selectedSession,
  onSelect,
  onSelectSession,
}: {
  row: AgentRow;
  snapshot: OrgSnapshot;
  selected: boolean;
  selectedSession: string | null;
  onSelect: () => void;
  onSelectSession: (sessionId: string) => void;
}) {
  const p = row.position;
  return (
    <li className="workers-tree__item workers-tree__item--agent">
      <button
        type="button"
        className="workers-tree__agent"
        aria-current={selected ? "true" : undefined}
        onClick={onSelect}
      >
        <span className="workers-tree__name">{p.title}</span>
        <StatusDot status={POSITION_STATUS[p.status]} label={STATUS_LABEL[p.status]} />
        {p.runtimeId && (
          <span className="workers-tree__tool">{runtimeLabel(snapshot, p.runtimeId)}</span>
        )}
      </button>
      {row.workers.length > 0 && (
        <ul className="workers-tree__children" aria-label={`${p.title}'s tasks now`}>
          {row.workers.map((w) =>
            w.sessionId ? (
              <li key={w.agentId} className="workers-tree__item">
                <button
                  type="button"
                  className="workers-tree__task"
                  aria-current={w.sessionId === selectedSession ? "true" : undefined}
                  onClick={() => w.sessionId && onSelectSession(w.sessionId)}
                >
                  {firstLine(w.objective)}
                </button>
              </li>
            ) : null,
          )}
        </ul>
      )}
    </li>
  );
}
