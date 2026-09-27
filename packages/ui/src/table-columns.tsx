/** Table columns and sorting, shared by the data table and the screens that build columns. */

import type { ReactNode } from "react";

import { StatusDot } from "./status";
import { STATUSES, type Status } from "./status-types";

export interface Column<T> {
  id: string;
  header: string;
  cell: (row: T) => ReactNode;
  /** The value sorted on; a column without one cannot be sorted. */
  sortValue?: (row: T) => string | number | null | undefined;
  /** Right-aligned, tabular numbers. */
  numeric?: boolean;
  /** A CSS width, e.g. "120px". */
  width?: string;
  /** The person can hide it (default: every column but the first). */
  hideable?: boolean;
  /** Hidden until the person shows it. */
  hidden?: boolean;
}

export type SortDirection = "asc" | "desc";
export interface SortState {
  column: string;
  direction: SortDirection;
}

export type PageSize = number | "all";

/** A status column: the mark and its word, sorted by severity. */
export function statusColumn<T>(
  get: (row: T) => { status: Status; label: string },
  header = "Status",
  id = "status",
): Column<T> {
  const order: Record<Status, number> = { error: 0, warn: 1, pending: 2, ok: 3, offline: 4 };
  return {
    id,
    header,
    width: "132px",
    cell: (row) => {
      const s = get(row);
      return <StatusDot status={s.status} label={s.label} />;
    },
    sortValue: (row) => order[get(row).status] ?? STATUSES.length,
  };
}

function compare(a: string | number | null | undefined, b: string | number | null | undefined) {
  const aEmpty = a === null || a === undefined || a === "";
  const bEmpty = b === null || b === undefined || b === "";
  if (aEmpty || bEmpty) return aEmpty === bEmpty ? 0 : aEmpty ? 1 : -1;
  if (typeof a === "number" && typeof b === "number") return a - b;
  return String(a).localeCompare(String(b), undefined, { numeric: true, sensitivity: "base" });
}

/** Rows in `sort` order (stable; empty values last either way). */
export function sortRows<T>(
  rows: readonly T[],
  columns: readonly Column<T>[],
  sort: SortState | null,
) {
  const column = sort && columns.find((c) => c.id === sort.column);
  if (!sort || !column?.sortValue) return rows;
  const key = column.sortValue;
  const sign = sort.direction === "asc" ? 1 : -1;
  return rows
    .map((row, index) => ({ row, index, value: key(row) }))
    .sort((a, b) => {
      const aEmpty = a.value === null || a.value === undefined || a.value === "";
      const bEmpty = b.value === null || b.value === undefined || b.value === "";
      if (aEmpty !== bEmpty) return aEmpty ? 1 : -1;
      return sign * compare(a.value, b.value) || a.index - b.index;
    })
    .map((x) => x.row);
}
