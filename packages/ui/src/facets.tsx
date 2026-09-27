/**
 * The filter panel (facets): search, grouped checkboxes with counts, range sliders, and
 * "Clear filters". `useFacets` binds it to a table or a card grid.
 */

import { useId, useState, type ReactNode } from "react";

import { Checkbox, IconButton, SearchField } from "./controls";
import { Icon } from "./icons";
import { cx, formatCount } from "./util";

import {
  EMPTY_FACETS,
  facetsActive,
  type FacetGroup,
  type FacetState,
  type RangeFacet,
} from "./facet-logic";

export type { FacetConfig, FacetGroup, FacetOption, FacetState, RangeFacet } from "./facet-logic";

/** A two-handle range slider. */
function RangeSlider({
  facet,
  value,
  onChange,
}: {
  facet: RangeFacet;
  value: [number, number];
  onChange: (next: [number, number]) => void;
}) {
  const fmt = facet.format ?? String;
  const span = facet.max - facet.min || 1;
  const [lo, hi] = value;
  return (
    <div className="ui-range">
      <div className="ui-range__head">
        <span>{facet.label}</span>
        <span className="ui-num">
          {fmt(lo)} to {fmt(hi)}
        </span>
      </div>
      <div className="ui-range__track">
        <span
          className="ui-range__fill"
          style={{
            left: `${((lo - facet.min) / span) * 100}%`,
            right: `${((facet.max - hi) / span) * 100}%`,
          }}
        />
        <input
          type="range"
          min={facet.min}
          max={facet.max}
          step={facet.step ?? 1}
          value={lo}
          aria-label={`${facet.label}, lowest`}
          aria-valuetext={fmt(lo)}
          onChange={(e) => onChange([Math.min(Number(e.target.value), hi), hi])}
        />
        <input
          type="range"
          min={facet.min}
          max={facet.max}
          step={facet.step ?? 1}
          value={hi}
          aria-label={`${facet.label}, highest`}
          aria-valuetext={fmt(hi)}
          onChange={(e) => onChange([lo, Math.max(Number(e.target.value), lo)])}
        />
      </div>
      {facet.marks && (
        <div className="ui-range__marks ui-num" aria-hidden="true">
          {facet.marks.map((m) => (
            <span key={m.value} style={{ left: `${((m.value - facet.min) / span) * 100}%` }}>
              {m.label}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

function Group({
  group,
  selected,
  onChange,
}: {
  group: FacetGroup;
  selected: string[];
  onChange: (next: string[]) => void;
}) {
  const [open, setOpen] = useState(true);
  const id = useId();
  return (
    <div className="ui-facets__group">
      <button
        type="button"
        className="ui-facets__group-head"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen((v) => !v)}
      >
        {group.label}
        <Icon name={open ? "chevronUp" : "chevronDown"} size={14} />
      </button>
      {open && (
        <div id={id} className="ui-facets__options">
          {group.options.length === 0 ? (
            <span className="ui-facets__none">None</span>
          ) : (
            group.options.map((o) => (
              <Checkbox
                key={o.value}
                label={o.label}
                count={o.count}
                checked={selected.includes(o.value)}
                onChange={(on) =>
                  onChange(on ? [...selected, o.value] : selected.filter((v) => v !== o.value))
                }
              />
            ))
          )}
        </div>
      )}
    </div>
  );
}

/** The panel. Collapsed, it leaves a narrow strip with a button to open it again. */
export function FacetPanel({
  label = "Filters",
  state,
  onChange,
  groups,
  ranges = [],
  collapsed,
  onCollapsedChange,
  searchPlaceholder = "Search",
  total,
  shown,
  children,
}: {
  label?: string;
  state: FacetState;
  onChange: (next: FacetState) => void;
  groups: readonly FacetGroup[];
  ranges?: readonly RangeFacet[];
  collapsed: boolean;
  onCollapsedChange: (next: boolean) => void;
  searchPlaceholder?: string;
  /** "Showing 12 of 40" when both are given. */
  total?: number;
  shown?: number;
  /** Extra controls at the top (switches). */
  children?: ReactNode;
}) {
  if (collapsed) {
    return (
      <aside className="ui-facets ui-facets--collapsed" aria-label={label}>
        <IconButton
          icon="panelOpen"
          label={`Show ${label.toLowerCase()}`}
          onClick={() => onCollapsedChange(false)}
        />
      </aside>
    );
  }
  const active = facetsActive(state, ranges);
  return (
    <aside className="ui-facets" aria-label={label}>
      <div className="ui-facets__top">
        <SearchField
          value={state.search}
          placeholder={searchPlaceholder}
          onChange={(search) => onChange({ ...state, search })}
        />
        <IconButton
          icon="panelClose"
          label={`Hide ${label.toLowerCase()}`}
          onClick={() => onCollapsedChange(true)}
        />
      </div>
      {total !== undefined && shown !== undefined && (
        <div className="ui-facets__count ui-num" role="status">
          Showing {formatCount(shown)} of {formatCount(total)}
        </div>
      )}
      {children}
      {groups.map((g) => (
        <Group
          key={g.id}
          group={g}
          selected={state.checks[g.id] ?? []}
          onChange={(values) => onChange({ ...state, checks: { ...state.checks, [g.id]: values } })}
        />
      ))}
      {ranges.map((r) => (
        <RangeSlider
          key={r.id}
          facet={r}
          value={state.ranges[r.id] ?? [r.min, r.max]}
          onChange={(v) => onChange({ ...state, ranges: { ...state.ranges, [r.id]: v } })}
        />
      ))}
      <button
        type="button"
        className={cx("ui-link", "ui-facets__clear")}
        disabled={!active}
        onClick={() => onChange(EMPTY_FACETS)}
      >
        Clear filters
      </button>
    </aside>
  );
}
