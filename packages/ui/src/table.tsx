/**
 * The dense data table (the UniFi device list): sortable columns, a status column, tabular
 * numbers, links to parent entities, row checkboxes, choosing columns, page size, and a
 * count of records. Only the rows on screen are drawn (ADR-029 §6).
 */

import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";

import { Button, Checkbox, IconButton } from "./controls";
import { Icon } from "./icons";
import { EmptyState, ErrorState, Skeleton } from "./states";
import { cx, formatCount, useDismiss, useStoredState, useVirtualWindow } from "./util";

import { sortRows, type Column, type PageSize, type SortState } from "./table-columns";

export type { Column, PageSize, SortDirection, SortState } from "./table-columns";

const ROW_HEIGHT = 28;
const HEADER_HEIGHT = 30;

/** A link to another entity inside a cell (a parent project, a department). */
export function CellLink({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" className="ui-link" onClick={onClick}>
      {children}
    </button>
  );
}

const isStringArray = (v: unknown): v is string[] =>
  Array.isArray(v) && v.every((x) => typeof x === "string");
const isPageSize = (v: unknown): v is PageSize =>
  v === "all" || (typeof v === "number" && Number.isInteger(v) && v > 0);

export function DataTable<T>({
  label,
  rows,
  columns,
  getRowId,
  selectable = false,
  selected,
  onSelectedChange,
  defaultSort = null,
  pageSizes = [25, 50, 100, 250, "all"],
  defaultPageSize = 100,
  storageKey,
  state = "ready",
  error,
  onRetry,
  empty,
  height = 480,
  toolbar,
}: {
  label: string;
  rows: readonly T[];
  columns: readonly Column<T>[];
  getRowId: (row: T) => string;
  selectable?: boolean;
  selected?: ReadonlySet<string>;
  onSelectedChange?: (next: Set<string>) => void;
  defaultSort?: SortState | null;
  pageSizes?: readonly PageSize[];
  defaultPageSize?: PageSize;
  /** Remember hidden columns and page size on this computer under this key. */
  storageKey?: string;
  state?: "ready" | "loading" | "error";
  error?: ReactNode;
  onRetry?: () => void;
  empty?: ReactNode;
  /** Height of the scrolling area (px). */
  height?: number;
  /** Extra controls on the left of the toolbar (bulk actions). */
  toolbar?: ReactNode;
}) {
  const [sort, setSort] = useState<SortState | null>(defaultSort);
  const [hidden, setHidden] = useStoredState<string[]>(
    storageKey && `${storageKey}.hidden`,
    columns.filter((c) => c.hidden).map((c) => c.id),
    isStringArray,
  );
  const [pageSize, setPageSize] = useStoredState<PageSize>(
    storageKey && `${storageKey}.pageSize`,
    defaultPageSize,
    isPageSize,
  );
  const [page, setPage] = useState(0);
  const [picking, setPicking] = useState(false);
  const picker = useRef<HTMLDivElement>(null);
  const closePicker = useCallback(() => setPicking(false), []);
  useDismiss(picking, picker, closePicker);
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);

  const visibleColumns = columns.filter((c) => !hidden.includes(c.id));
  const sorted = useMemo(() => sortRows(rows, columns, sort), [rows, columns, sort]);
  const size = pageSize === "all" ? Math.max(1, sorted.length) : pageSize;
  const pages = Math.max(1, Math.ceil(sorted.length / size));
  const current = Math.min(page, pages - 1);
  const pageRows = useMemo(
    () => sorted.slice(current * size, current * size + size),
    [sorted, current, size],
  );
  const win = useVirtualWindow({
    count: pageRows.length,
    itemHeight: ROW_HEIGHT,
    scroller,
    offset: HEADER_HEIGHT,
  });
  const shown = pageRows.slice(win.start, win.end);

  const chosen = selected ?? new Set<string>();
  const allIds = rows.map(getRowId);
  const chosenCount = allIds.filter((id) => chosen.has(id)).length;
  const toggleRow = (id: string, on: boolean) => {
    const next = new Set(chosen);
    if (on) next.add(id);
    else next.delete(id);
    onSelectedChange?.(next);
  };
  const toggleAll = (on: boolean) => onSelectedChange?.(on ? new Set(allIds) : new Set());

  const sortBy = (column: Column<T>) => {
    setPage(0);
    setSort((prev) =>
      prev?.column === column.id
        ? { column: column.id, direction: prev.direction === "asc" ? "desc" : "asc" }
        : { column: column.id, direction: "asc" },
    );
  };

  const first = sorted.length === 0 ? 0 : current * size + 1;
  const last = Math.min(sorted.length, current * size + size);
  const span = visibleColumns.length + (selectable ? 1 : 0);

  return (
    <section className="ui-table" aria-label={label}>
      <div className="ui-table__toolbar">
        <div className="ui-table__bulk">
          {selectable && chosenCount > 0 && (
            <span className="ui-num">{formatCount(chosenCount)} selected</span>
          )}
          {toolbar}
        </div>
        <div className="ui-table__columns" ref={picker}>
          <Button
            size="sm"
            variant="quiet"
            icon="columns"
            aria-expanded={picking}
            onClick={() => setPicking((v) => !v)}
          >
            Columns
          </Button>
          {picking && (
            <div className="ui-popover" role="group" aria-label="Choose columns">
              {columns.map((c, i) => {
                const canHide = c.hideable ?? i > 0;
                return (
                  <Checkbox
                    key={c.id}
                    label={c.header}
                    checked={!hidden.includes(c.id)}
                    disabled={!canHide}
                    onChange={(show) =>
                      setHidden(show ? hidden.filter((h) => h !== c.id) : [...hidden, c.id])
                    }
                  />
                );
              })}
            </div>
          )}
        </div>
      </div>

      {state === "error" ? (
        <ErrorState title="Couldn't load this table" message={error} onRetry={onRetry} />
      ) : (
        <div className="ui-table__scroll" ref={setScroller} style={{ maxHeight: height }}>
          <table
            aria-label={label}
            aria-rowcount={state === "ready" ? sorted.length + 1 : undefined}
          >
            <thead>
              <tr aria-rowindex={1}>
                {selectable && (
                  <th className="ui-table__check" scope="col">
                    <Checkbox
                      label={`Select all ${formatCount(rows.length)}`}
                      hideLabel
                      checked={rows.length > 0 && chosenCount === rows.length}
                      indeterminate={chosenCount > 0 && chosenCount < rows.length}
                      disabled={state !== "ready" || rows.length === 0}
                      onChange={toggleAll}
                    />
                  </th>
                )}
                {visibleColumns.map((c) => {
                  const direction = sort?.column === c.id ? sort.direction : undefined;
                  const active = direction !== undefined;
                  return (
                    <th
                      key={c.id}
                      scope="col"
                      style={c.width ? { width: c.width } : undefined}
                      className={cx(c.numeric && "ui-table__num")}
                      aria-sort={
                        direction ? (direction === "asc" ? "ascending" : "descending") : undefined
                      }
                    >
                      {c.sortValue ? (
                        <button
                          type="button"
                          className={cx("ui-table__sort", active && "ui-table__sort--active")}
                          onClick={() => sortBy(c)}
                        >
                          {c.header}
                          <Icon
                            name={direction === "desc" ? "chevronDown" : "chevronUp"}
                            size={12}
                            className="ui-table__sort-icon"
                          />
                        </button>
                      ) : (
                        c.header
                      )}
                    </th>
                  );
                })}
              </tr>
            </thead>
            <tbody>
              {state === "loading" ? (
                Array.from({ length: 6 }, (_, i) => (
                  <tr key={i} className="ui-table__skeleton">
                    <td colSpan={span}>
                      {i === 0 && (
                        <span className="ui-visually-hidden" role="status">
                          Loading…
                        </span>
                      )}
                      <Skeleton width={`${80 - (i % 3) * 15}%`} />
                    </td>
                  </tr>
                ))
              ) : pageRows.length === 0 ? (
                <tr>
                  <td colSpan={span} className="ui-table__empty">
                    {empty ?? <EmptyState compact title="No records" />}
                  </td>
                </tr>
              ) : (
                <>
                  {win.before > 0 && (
                    <tr aria-hidden="true" className="ui-table__spacer">
                      <td colSpan={span} style={{ height: win.before }} />
                    </tr>
                  )}
                  {shown.map((row, i) => {
                    const id = getRowId(row);
                    const on = chosen.has(id);
                    return (
                      <tr
                        key={id}
                        aria-rowindex={current * size + win.start + i + 2}
                        aria-selected={selectable ? on : undefined}
                        className={cx(on && "ui-table__row--selected")}
                      >
                        {selectable && (
                          <td className="ui-table__check">
                            <Checkbox
                              label={`Select row ${current * size + win.start + i + 1}`}
                              hideLabel
                              checked={on}
                              onChange={(v) => toggleRow(id, v)}
                            />
                          </td>
                        )}
                        {visibleColumns.map((c) => (
                          <td key={c.id} className={cx(c.numeric && "ui-table__num")}>
                            {c.cell(row)}
                          </td>
                        ))}
                      </tr>
                    );
                  })}
                  {win.after > 0 && (
                    <tr aria-hidden="true" className="ui-table__spacer">
                      <td colSpan={span} style={{ height: win.after }} />
                    </tr>
                  )}
                </>
              )}
            </tbody>
          </table>
        </div>
      )}

      <div className="ui-table__footer">
        <IconButton
          icon="chevronLeft"
          label="Previous page"
          disabled={current === 0}
          onClick={() => setPage(current - 1)}
        />
        <IconButton
          icon="chevronRight"
          label="Next page"
          disabled={current >= pages - 1}
          onClick={() => setPage(current + 1)}
        />
        <span className="ui-table__records ui-num" role="status">
          {state === "ready"
            ? `${formatCount(first)}–${formatCount(last)} of ${formatCount(sorted.length)} ${sorted.length === 1 ? "record" : "records"}`
            : ""}
        </span>
        <label className="ui-table__page-size">
          Rows per page
          <select
            value={String(pageSize)}
            onChange={(e) => {
              setPage(0);
              setPageSize(e.target.value === "all" ? "all" : Number(e.target.value));
            }}
          >
            {pageSizes.map((p) => (
              <option key={String(p)} value={String(p)}>
                {p === "all" ? "All" : p}
              </option>
            ))}
          </select>
        </label>
      </div>
    </section>
  );
}
