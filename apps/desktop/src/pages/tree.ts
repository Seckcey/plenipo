import type { OrgSnapshot, TaskTree, TaskTreeNode } from "@plenipo/types";
import type { MapLink, MapNode } from "@plenipo/ui";

import { TASK_STATUS, firstLine } from "./words";

/** The position a task was given to, from its Workforce record. */
export function taskPosition(metadata: Record<string, unknown>): string | null {
  const w = metadata.workforce;
  if (typeof w !== "object" || w === null) return null;
  const id = (w as Record<string, unknown>).positionId;
  return typeof id === "string" ? id : null;
}

/** Who did a task in a tree: its position, else where it was handed, else its AI tool. */
export function whoDid(n: TaskTreeNode, org: OrgSnapshot | null): string {
  const id = taskPosition(n.task.metadata);
  const title = id ? org?.positions.find((p) => p.id === id)?.title : undefined;
  return title ?? n.handoff?.destinationLabel ?? n.runtimeLabel ?? "A worker";
}

/** A delegation tree as a map: a tile for each task (who and its state), linked to its parent. */
export function treeMap(
  tree: TaskTree,
  org: OrgSnapshot | null,
): { nodes: MapNode[]; links: MapLink[] } {
  const ids = new Set(tree.nodes.map((n) => n.task.id));
  return {
    nodes: tree.nodes.map((n) => ({
      id: n.task.id,
      label: whoDid(n, org),
      status: TASK_STATUS[n.task.state].status,
      statusLabel: TASK_STATUS[n.task.state].label,
      caption: firstLine(n.task.objective, 40),
    })),
    links: tree.nodes
      .filter((n) => n.task.parentTaskId !== null && ids.has(n.task.parentTaskId))
      .map((n) => ({ from: n.task.parentTaskId!, to: n.task.id })),
  };
}
