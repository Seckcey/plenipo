/**
 * The relationship map: tiles filled with their status color, labeled connectors, and a
 * caption under each tile. Used for delegation trees and handoff chains.
 */

import type { ReactNode } from "react";

import { Icon } from "./icons";
import { EmptyState, ErrorState, LoadingState } from "./states";
import { cx } from "./util";

import { layoutMap, TILE_H, TILE_W, type MapLink, type MapNode } from "./topology-layout";

export type { MapLink, MapNode } from "./topology-layout";

export function TopologyMap({
  label,
  nodes,
  links,
  selectedId,
  onSelect,
  state = "ready",
  error,
  empty,
}: {
  label: string;
  nodes: readonly MapNode[];
  links: readonly MapLink[];
  selectedId?: string | null;
  onSelect?: (id: string) => void;
  state?: "ready" | "loading" | "error";
  error?: ReactNode;
  empty?: ReactNode;
}) {
  if (state === "loading") return <LoadingState label={`Loading ${label.toLowerCase()}`} />;
  if (state === "error")
    return <ErrorState title={`Couldn't load ${label.toLowerCase()}`} message={error} />;
  if (nodes.length === 0) return <>{empty ?? <EmptyState compact title="Nothing to map yet" />}</>;

  const { placed, width, height } = layoutMap(nodes, links);
  const byName = new Map(nodes.map((n) => [n.id, n.label]));
  return (
    <div className="ui-map" role="group" aria-label={label}>
      <div className="ui-map__canvas" style={{ width, height }}>
        <svg className="ui-map__links" width={width} height={height} aria-hidden="true">
          {links.map((l) => {
            const a = placed.get(l.from);
            const b = placed.get(l.to);
            if (!a || !b) return null;
            const x1 = a.x + TILE_W / 2;
            const y1 = a.y + TILE_H + 16;
            const x2 = b.x + TILE_W / 2;
            const y2 = b.y;
            const mid = (y1 + y2) / 2;
            return (
              <g key={`${l.from}-${l.to}`}>
                <path d={`M${x1},${y1} V${mid} H${x2} V${y2}`} className="ui-map__link" />
                {l.label && (
                  <text x={x2 + 4} y={mid + (y2 - mid) / 2 + 4} className="ui-map__link-label">
                    {l.label}
                  </text>
                )}
              </g>
            );
          })}
        </svg>
        {[...placed.values()].map(({ node, x, y }) => (
          <div key={node.id} className="ui-map__node" style={{ left: x, top: y, width: TILE_W }}>
            <button
              type="button"
              className={cx(
                "ui-map__tile",
                `ui-map__tile--${node.status}`,
                selectedId === node.id && "ui-map__tile--selected",
              )}
              style={{ height: TILE_H }}
              aria-pressed={onSelect ? selectedId === node.id : undefined}
              aria-label={`${node.label}, ${node.statusLabel}${node.caption ? `, ${node.caption}` : ""}`}
              onClick={onSelect ? () => onSelect(node.id) : undefined}
            >
              <span className="ui-map__tile-name">
                {node.icon && <Icon name={node.icon} size={14} />}
                {node.label}
              </span>
              <span className="ui-map__tile-status">
                <span className="ui-status__mark" aria-hidden="true" />
                {node.statusLabel}
              </span>
            </button>
            {node.caption && <div className="ui-map__caption ui-num">{node.caption}</div>}
          </div>
        ))}
      </div>
      <ul className="ui-visually-hidden" aria-label="Connections">
        {links.map((l) => (
          <li key={`${l.from}-${l.to}`}>
            {byName.get(l.from)} to {byName.get(l.to)}
            {l.label ? `: ${l.label}` : ""}
          </li>
        ))}
      </ul>
    </div>
  );
}
