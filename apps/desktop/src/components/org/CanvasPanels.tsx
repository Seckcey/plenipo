/**
 * The canvas's toolbar and its panels (Phase 18, ADR-053 §12–§16, §21): the three ways to use
 * the pointer, Tidy up, zoom, filters, the legend, the "where" marks, oversight lines, Add, the
 * trash can (which opens the Archived drawer), and the guide. Every button has words (shown, or
 * as its name and tooltip) and a key where it has one.
 */
import {
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import type { OrgSnapshot } from "@plenipo/types";
import { Button, IconButton, MenuButton, Select, type MenuItem } from "@plenipo/ui";

import {
  FILTER_KEYS,
  FILTER_LABEL,
  filterOptions,
  type CanvasFilters,
  type FilterKey,
} from "../../org/filters";
import { SYMBOLS, SYMBOL_GROUPS, type CanvasSymbol } from "../../org/symbols";
import { TOUR_STEPS, type PointerMode } from "../../org/tour";
import { CanvasControlsContext } from "./canvasContext";
import { Archived, type DirectoryActions } from "./Directory";
import { Glyph } from "./Glyph";

const MODES: { mode: PointerMode; label: string; key: string; glyph: string; words: string }[] = [
  {
    mode: "select",
    label: "Select",
    key: "V",
    glyph: "select",
    words: "Click to see details; drag an agent onto another, the trash can, or an empty spot",
  },
  {
    mode: "pan",
    label: "Move the view",
    key: "H",
    glyph: "hand",
    words: "Drag anywhere to move the view (or hold the space bar)",
  },
  {
    mode: "arrange",
    label: "Arrange",
    key: "A",
    glyph: "arrange",
    words: "Drag tiles to place them; hold Alt to move one tile without its team",
  },
];

export function CanvasToolbar({
  mode,
  onMode,
  onTidy,
  canTidy,
  filtersOpen,
  filterCount,
  onFilters,
  legendOpen,
  onLegend,
  whereOn,
  onWhere,
  oversightOn,
  onOversight,
  addItems,
  onAdd,
  trashOpen,
  archived,
  onTrash,
  helpOpen,
  onHelp,
}: {
  mode: PointerMode;
  onMode: (mode: PointerMode) => void;
  onTidy: () => void;
  canTidy: boolean;
  filtersOpen: boolean;
  filterCount: number;
  onFilters: () => void;
  legendOpen: boolean;
  onLegend: () => void;
  whereOn: boolean;
  onWhere: () => void;
  oversightOn: boolean;
  onOversight: () => void;
  addItems: MenuItem[];
  onAdd: (id: string) => void;
  trashOpen: boolean;
  archived: number;
  onTrash: () => void;
  helpOpen: boolean;
  onHelp: () => void;
}) {
  const bar = useRef<HTMLDivElement>(null);
  const controls = useContext(CanvasControlsContext);
  const noop = () => undefined;
  const zoom = controls?.zoom ?? { level: 1, zoomIn: noop, zoomOut: noop, fit: noop };
  /** A drag over the trash can: whether it can be dropped there. */
  const trashDrop = controls?.trash ?? null;
  /** Arrow keys move along the toolbar (Home and End to its ends). */
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) return;
    const target = e.target as HTMLElement;
    if (target.closest('[role="menu"]')) return;
    const items = [
      ...(bar.current?.querySelectorAll<HTMLButtonElement>("button:not([disabled])") ?? []),
    ].filter((b) => !b.closest('[role="menu"]'));
    const at = items.indexOf(target as HTMLButtonElement);
    if (at < 0) return;
    e.preventDefault();
    const next =
      e.key === "Home"
        ? 0
        : e.key === "End"
          ? items.length - 1
          : (at + (e.key === "ArrowRight" ? 1 : -1) + items.length) % items.length;
    items[next]?.focus();
  };
  const tool = (props: {
    label: string;
    glyph: string;
    title?: string;
    pressed?: boolean;
    expanded?: boolean;
    onClick: () => void;
    disabled?: boolean;
    children?: ReactNode;
    symbol?: string;
    drop?: string;
  }) => (
    <button
      key={props.label}
      type="button"
      className="canvas-tool"
      aria-label={props.label}
      title={props.title ?? props.label}
      aria-pressed={props.pressed}
      aria-expanded={props.expanded}
      disabled={props.disabled}
      onClick={props.onClick}
      data-drop={props.drop}
    >
      <Glyph name={props.glyph} size={17} />
      {props.children}
    </button>
  );
  return (
    <div
      ref={bar}
      className="canvas-toolbar"
      data-canvas-ui
      role="toolbar"
      aria-label="Canvas tools"
      onKeyDown={onKeyDown}
    >
      <div className="canvas-toolbar__group" role="group" aria-label="Pointer">
        {MODES.map((m) =>
          tool({
            label: m.label,
            glyph: m.glyph,
            title: `${m.label} (${m.key}): ${m.words}`,
            pressed: mode === m.mode,
            onClick: () => onMode(m.mode),
          }),
        )}
      </div>
      <div className="canvas-toolbar__group">
        <button
          type="button"
          className="canvas-tool canvas-tool--words"
          title="Put every tile back in neat rows (with Undo)"
          disabled={!canTidy}
          onClick={onTidy}
        >
          <Glyph name="tidy" size={17} />
          Tidy up
        </button>
      </div>
      <div className="canvas-toolbar__group" role="group" aria-label="Zoom">
        {tool({ label: "Zoom out", title: "Zoom out (−)", glyph: "minus", onClick: zoom.zoomOut })}
        <span className="canvas-toolbar__zoom" aria-label="Zoom level">
          {Math.round(zoom.level * 100)}%
        </span>
        {tool({ label: "Zoom in", title: "Zoom in (+)", glyph: "plus", onClick: zoom.zoomIn })}
        {tool({
          label: "Fit to screen",
          title: "Fit to screen (0)",
          glyph: "fit",
          onClick: zoom.fit,
        })}
      </div>
      <div className="canvas-toolbar__group" role="group" aria-label="Show">
        {tool({
          label: filterCount > 0 ? `Filters (${filterCount} on)` : "Filters",
          glyph: "filter",
          expanded: filtersOpen,
          onClick: onFilters,
          children: filterCount > 0 && <span className="canvas-tool__count">{filterCount}</span>,
        })}
        {tool({
          label: "Legend",
          title: "Legend: what every mark means",
          glyph: "legend",
          pressed: legendOpen,
          onClick: onLegend,
        })}
        {tool({
          label: "Where",
          title: "Where: where each worker's work runs and what it touches",
          glyph: "where",
          pressed: whereOn,
          onClick: onWhere,
        })}
        {tool({
          label: "Oversight lines",
          title: "Show review, QA, and security lines",
          glyph: "link",
          pressed: oversightOn,
          onClick: onOversight,
        })}
      </div>
      <div className="canvas-toolbar__group">
        <MenuButton label="Add" icon="plus" variant="quiet" items={addItems} onSelect={onAdd} />
      </div>
      <div className="canvas-toolbar__group canvas-toolbar__end">
        <button
          type="button"
          className={`canvas-tool canvas-tool--trash${
            trashDrop === "valid"
              ? " is-drop-target"
              : trashDrop === "invalid"
                ? " is-drop-refused"
                : ""
          }`}
          data-drop="trash"
          aria-label={`Trash can: drop an agent here to archive it. Archived (${archived})`}
          title="Trash can: drop an agent here to archive it (with Undo). Click to see what is archived."
          aria-expanded={trashOpen}
          onClick={onTrash}
        >
          <Glyph name="trash" size={17} />
          {archived > 0 && <span className="canvas-tool__count">{archived}</span>}
        </button>
        {tool({
          label: "Guide to the canvas",
          title: "Guide to the canvas (?)",
          glyph: "help",
          expanded: helpOpen,
          onClick: onHelp,
        })}
      </div>
    </div>
  );
}

/** A panel floating over the canvas, under the toolbar. */
function Panel({
  label,
  className,
  onClose,
  children,
}: {
  label: string;
  className: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section
      className={`canvas-panel ${className}`}
      data-canvas-ui
      data-canvas-scroll
      aria-labelledby={id}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <header className="canvas-panel__head">
        <h2 id={id}>{label}</h2>
        <IconButton icon="close" label={`Close ${label.toLowerCase()}`} onClick={onClose} />
      </header>
      {children}
    </section>
  );
}

export function FiltersPanel({
  snapshot,
  filters,
  onChange,
  matched,
  total,
  onClose,
}: {
  snapshot: OrgSnapshot;
  filters: CanvasFilters;
  onChange: (next: CanvasFilters) => void;
  matched: number | null;
  total: number;
  onClose: () => void;
}) {
  const options = filterOptions(snapshot);
  const any = FILTER_KEYS.some((k) => filters[k] !== null);
  const choose = (key: FilterKey, value: string) =>
    onChange({ ...filters, [key]: value === "" ? null : value });
  return (
    <Panel label="Filters" className="canvas-panel--filters" onClose={onClose}>
      <p className="canvas-panel__note" role="status">
        {matched === null
          ? `Showing all ${total} agents`
          : `Showing ${matched} of ${total} agents (leads above them stay, faded)`}
      </p>
      <div className="canvas-filters">
        {FILTER_KEYS.map((key) => (
          <Select
            key={key}
            label={FILTER_LABEL[key]}
            value={filters[key] ?? ""}
            options={[
              { value: "", label: `Any ${FILTER_LABEL[key].toLowerCase()}` },
              ...options[key],
            ]}
            onChange={(v) => choose(key, v)}
          />
        ))}
      </div>
      <Button
        size="sm"
        variant="quiet"
        disabled={!any}
        onClick={() =>
          onChange(
            Object.fromEntries(FILTER_KEYS.map((k) => [k, null])) as unknown as CanvasFilters,
          )
        }
      >
        Clear filters
      </Button>
    </Panel>
  );
}

/** How the legend draws one mark, with the canvas's own classes. */
function Sample({ symbol }: { symbol: CanvasSymbol }) {
  const s = symbol.sample;
  switch (s.kind) {
    case "tile":
      return (
        <span className={`legend-sample legend-sample--tile topo-node ${s.className}`}>
          <Glyph name={s.glyph} size={12} />
        </span>
      );
    case "status":
      return (
        <span className="topo-status" data-status={s.status}>
          <span className="topo-status__dot" aria-hidden="true" />
        </span>
      );
    case "line":
      return (
        <svg
          className="legend-sample legend-sample--line"
          width="36"
          height="12"
          aria-hidden="true"
        >
          <path d="M 2 6 H 34" className={s.className} />
        </svg>
      );
    case "chip":
    case "badge":
      return <span className={`legend-sample ${s.className}`}>{s.text}</span>;
    case "where":
      return (
        <span className="legend-sample legend-sample--where">
          <Glyph name={s.glyph} size={14} />
        </span>
      );
  }
}

export function LegendPanel({ onClose }: { onClose: () => void }) {
  return (
    <Panel label="Legend" className="canvas-panel--legend" onClose={onClose}>
      {SYMBOL_GROUPS.map((group) => (
        <div key={group} className="legend-group">
          <h3>{group}</h3>
          <ul>
            {SYMBOLS.filter((s) => s.group === group).map((s) => (
              <li key={s.key} data-legend={s.key}>
                <span className="legend-group__sample" aria-hidden="true">
                  <Sample symbol={s} />
                </span>
                <span>
                  <strong>{s.label}</strong> — {s.words}
                </span>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </Panel>
  );
}

export function HelpPanel({ onTour, onClose }: { onTour: () => void; onClose: () => void }) {
  return (
    <Panel label="The canvas" className="canvas-panel--help" onClose={onClose}>
      <ul className="canvas-help">
        <li>
          <strong>Select</strong> (V): click a tile for its details. Drag an agent onto another for
          Move here, Lend, or oversee its team; onto the trash can to archive it; or onto an empty
          spot to place it.
        </li>
        <li>
          <strong>Move the view</strong> (H, or hold the space bar), and <strong>Arrange</strong>{" "}
          (A) to place tiles without any questions. A tile moves with its team; hold Alt to move it
          alone. Alt + arrow keys move the selected tile. <strong>Tidy up</strong> puts everything
          back in rows.
        </li>
        <li>
          Select an agent to see round <strong>line ends</strong>: drag one onto another agent to
          change who it reports to, or which team a reviewer checks.
        </li>
        <li>
          <strong>Filters</strong> and the <strong>Legend</strong> are in the toolbar;{" "}
          <strong>Where</strong> shows where each worker&apos;s work runs and what it touches.
        </li>
        <li>
          <strong>Watch</strong> a working agent to see the code its worker writes, as it writes it.
        </li>
      </ul>
      <Button size="sm" variant="primary" onClick={onTour}>
        Take the tour again
      </Button>
    </Panel>
  );
}

/** The first-time tour (ADR-053 §21): six short steps, which can be skipped. */
export function CanvasTour({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState(0);
  const card = useRef<HTMLDivElement>(null);
  const id = useId();
  const s = TOUR_STEPS[step] ?? TOUR_STEPS[0]!;
  const last = step === TOUR_STEPS.length - 1;
  useEffect(() => {
    card.current?.querySelector<HTMLElement>("[data-tour-next]")?.focus();
  }, [step]);
  return (
    <div
      ref={card}
      className="canvas-tour"
      data-canvas-ui
      role="dialog"
      aria-modal="false"
      aria-labelledby={id}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onDone();
        }
      }}
    >
      <p className="canvas-tour__step">
        Step {step + 1} of {TOUR_STEPS.length}
      </p>
      <h2 id={id}>
        <Glyph name={s.glyph} size={18} /> {s.title}
      </h2>
      <p>{s.words}</p>
      <div className="canvas-tour__actions">
        <Button size="sm" variant="quiet" onClick={onDone}>
          Skip the tour
        </Button>
        {step > 0 && (
          <Button size="sm" onClick={() => setStep(step - 1)}>
            Back
          </Button>
        )}
        <Button
          size="sm"
          variant="primary"
          data-tour-next
          onClick={() => (last ? onDone() : setStep(step + 1))}
        >
          {last ? "Done" : "Next"}
        </Button>
      </div>
    </div>
  );
}

export function ArchivedDrawer({
  snapshot,
  actions,
  onSelect,
  onClose,
}: {
  snapshot: OrgSnapshot;
  actions: DirectoryActions;
  onSelect: (id: string) => void;
  onClose: () => void;
}) {
  return (
    <Panel label="Archived" className="canvas-panel--archived" onClose={onClose}>
      <Archived snapshot={snapshot} q="" onSelect={onSelect} actions={actions} />
    </Panel>
  );
}
