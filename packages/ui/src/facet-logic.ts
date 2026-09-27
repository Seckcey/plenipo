/** Filter state and matching for the filter panel, and the hook that binds it to a list. */

import { useMemo, useState } from "react";

export interface FacetOption {
  value: string;
  label: string;
  count: number;
}

export interface FacetGroup {
  id: string;
  label: string;
  options: FacetOption[];
}

export interface RangeFacet {
  id: string;
  label: string;
  min: number;
  max: number;
  step?: number;
  /** Labels under the slider, e.g. 1D, 1W, All. */
  marks?: { value: number; label: string }[];
  /** How a value is spoken and shown. */
  format?: (value: number) => string;
}

export interface FacetState {
  search: string;
  checks: Record<string, string[]>;
  ranges: Record<string, [number, number]>;
}

export const EMPTY_FACETS: FacetState = { search: "", checks: {}, ranges: {} };

export function facetsActive(state: FacetState, ranges: readonly RangeFacet[] = []): boolean {
  if (state.search.trim()) return true;
  if (Object.values(state.checks).some((v) => v.length > 0)) return true;
  return ranges.some((r) => {
    const v = state.ranges[r.id];
    return v !== undefined && (v[0] > r.min || v[1] < r.max);
  });
}

/** What each facet reads from an item. */
export interface FacetConfig<T> {
  /** Text the search box matches (case-insensitive). */
  search?: (item: T) => string;
  /** Checkbox groups: the value(s) an item has in each. */
  groups?: {
    id: string;
    label: string;
    value: (item: T) => string | string[];
    labels?: Record<string, string>;
  }[];
  /** Range sliders: the number an item has. */
  ranges?: (RangeFacet & { value: (item: T) => number })[];
}

const asList = (v: string | string[]) => (Array.isArray(v) ? v : [v]);

/** Items that pass every active facet. */
export function filterItems<T>(
  items: readonly T[],
  state: FacetState,
  config: FacetConfig<T>,
): T[] {
  const needle = state.search.trim().toLowerCase();
  return items.filter((item) => {
    if (needle && config.search && !config.search(item).toLowerCase().includes(needle))
      return false;
    for (const g of config.groups ?? []) {
      const wanted = state.checks[g.id];
      if (wanted && wanted.length > 0 && !asList(g.value(item)).some((v) => wanted.includes(v))) {
        return false;
      }
    }
    for (const r of config.ranges ?? []) {
      const range = state.ranges[r.id];
      // A slider set back to its full width does not filter (items beyond its ends included).
      if (!range || (range[0] <= r.min && range[1] >= r.max)) continue;
      const v = r.value(item);
      if (v < range[0] || v > range[1]) return false;
    }
    return true;
  });
}

/**
 * Filter state bound to `items`: the filtered items, and each group's options with counts. A
 * group's counts ignore that group's own checkboxes, so picking one option does not zero the
 * others.
 */
export function useFacets<T>(items: readonly T[], config: FacetConfig<T>) {
  const [state, setState] = useState<FacetState>(EMPTY_FACETS);
  const filtered = useMemo(() => filterItems(items, state, config), [items, state, config]);
  const groups = useMemo<FacetGroup[]>(
    () =>
      (config.groups ?? []).map((g) => {
        const others = filterItems(
          items,
          { ...state, checks: { ...state.checks, [g.id]: [] } },
          config,
        );
        const counts = new Map<string, number>();
        for (const item of items)
          for (const v of asList(g.value(item))) counts.set(v, counts.get(v) ?? 0);
        for (const item of others)
          for (const v of asList(g.value(item))) counts.set(v, (counts.get(v) ?? 0) + 1);
        // A ticked option stays listed (with 0) even when no item has it now, so it can be
        // unticked.
        for (const v of state.checks[g.id] ?? []) if (!counts.has(v)) counts.set(v, 0);
        return {
          id: g.id,
          label: g.label,
          options: [...counts.entries()]
            .map(([value, count]) => ({ value, label: g.labels?.[value] ?? value, count }))
            .sort((a, b) => a.label.localeCompare(b.label)),
        };
      }),
    [items, state, config],
  );
  return { state, setState, filtered, groups, ranges: config.ranges ?? [] };
}
