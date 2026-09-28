/**
 * The canvas's filters (Phase 18, ADR-053 §13–§14): department, project, status, AI tool, AI
 * company, rank, and specialty, plus the search. A filter narrows what the canvas shows: agents
 * that do not match are hidden, and the leads above a match stay, faded, so its lines still make
 * sense. Pure; no DOM.
 */
import type { OrgSnapshot, PositionInfo, PositionKind, PositionStatus } from "@plenipo/types";

import { STATUS_LABEL } from "./format";
import { rankName, titlesOf } from "./titles";

export interface CanvasFilters {
  department: string | null;
  project: string | null;
  status: PositionStatus | null;
  /** An AI tool's ID. */
  runtime: string | null;
  /** An AI company's name ("Anthropic"). */
  company: string | null;
  rank: PositionKind | null;
  /** A specialty's ID. */
  specialty: string | null;
}

export const NO_FILTERS: CanvasFilters = {
  department: null,
  project: null,
  status: null,
  runtime: null,
  company: null,
  rank: null,
  specialty: null,
};

export type FilterKey = keyof CanvasFilters;

export const FILTER_KEYS: readonly FilterKey[] = [
  "department",
  "project",
  "status",
  "runtime",
  "company",
  "rank",
  "specialty",
];

/** What each filter is called on screen. */
export const FILTER_LABEL: Record<FilterKey, string> = {
  department: "Department",
  project: "Project",
  status: "Status",
  runtime: "AI tool",
  company: "AI company",
  rank: "Rank",
  specialty: "Specialty",
};

export function filtersActive(f: CanvasFilters): boolean {
  return FILTER_KEYS.some((k) => f[k] !== null);
}

export function isCanvasFilters(v: unknown): v is CanvasFilters {
  if (typeof v !== "object" || v === null) return false;
  const f = v as Record<string, unknown>;
  return FILTER_KEYS.every((k) => f[k] === null || typeof f[k] === "string");
}

/** The AI company of a position's AI tool (ADR-053 §14: the tool's company until Phase 16). */
export function companyOf(snapshot: OrgSnapshot, p: PositionInfo): string | null {
  if (p.runtimeId === null) return null;
  return snapshot.runtimes.find((r) => r.id === p.runtimeId)?.company ?? null;
}

function matchesFilters(snapshot: OrgSnapshot, p: PositionInfo, f: CanvasFilters): boolean {
  return (
    (f.department === null ||
      p.departmentId === f.department ||
      p.headsDepartmentId === f.department) &&
    (f.project === null || p.projectId === f.project || p.coordinatesProjectId === f.project) &&
    (f.status === null || p.status === f.status) &&
    (f.runtime === null || p.runtimeId === f.runtime) &&
    (f.company === null || companyOf(snapshot, p) === f.company) &&
    (f.rank === null || p.kind === f.rank) &&
    (f.specialty === null || p.specialtyId === f.specialty)
  );
}

export interface FilterResult {
  /** Positions shown: the matches, and the leads above each. */
  shown: Set<string>;
  /** Shown only to keep a match's lines in place: faded. */
  faded: Set<string>;
  /** Active agents that match, and all active agents. */
  matched: number;
  total: number;
}

/**
 * What the canvas shows under these filters; `null` when no filter is on (everything shows).
 * `searched` is the search's matches (`null`: no search); a search narrows like a filter.
 */
export function visibleTiles(
  snapshot: OrgSnapshot,
  filters: CanvasFilters,
  searched: readonly string[] | null = null,
): FilterResult | null {
  if (!filtersActive(filters) && searched === null) return null;
  const active = snapshot.positions.filter((p) => p.active);
  const byId = new Map(active.map((p) => [p.id, p]));
  const found = searched === null ? null : new Set(searched);
  const shown = new Set<string>();
  const faded = new Set<string>();
  let matched = 0;
  for (const p of active) {
    if (!matchesFilters(snapshot, p, filters) || (found !== null && !found.has(p.id))) continue;
    matched += 1;
    shown.add(p.id);
    faded.delete(p.id);
    const seen = new Set([p.id]);
    let lead = p.reportsTo ? byId.get(p.reportsTo) : undefined;
    while (lead && !seen.has(lead.id)) {
      seen.add(lead.id);
      if (!shown.has(lead.id)) {
        shown.add(lead.id);
        faded.add(lead.id);
      }
      lead = lead.reportsTo ? byId.get(lead.reportsTo) : undefined;
    }
  }
  return { shown, faded, matched, total: active.length };
}

export interface FilterOption {
  value: string;
  label: string;
}

/** The choices each filter offers, from what the organization has. */
export function filterOptions(snapshot: OrgSnapshot): Record<FilterKey, FilterOption[]> {
  const active = snapshot.positions.filter((p) => p.active);
  const titles = titlesOf(snapshot);
  const byName = (a: FilterOption, b: FilterOption) => a.label.localeCompare(b.label);
  const statuses = [...new Set(active.map((p) => p.status))];
  const ranks: PositionKind[] = [
    "superintendent",
    "departmentManager",
    "projectCoordinator",
    "worker",
  ];
  const companies = [
    ...new Set(active.map((p) => companyOf(snapshot, p)).filter((c): c is string => !!c)),
  ];
  const specialties = new Map<string, string>();
  for (const p of active) {
    if (p.specialtyId && p.specialty) specialties.set(p.specialtyId, p.specialty);
  }
  return {
    department: snapshot.departments
      .filter((d) => d.archivedAt === null)
      .map((d) => ({ value: d.id, label: d.name }))
      .sort(byName),
    project: snapshot.projects
      .filter((p) => p.archivedAt === null)
      .map((p) => ({ value: p.id, label: p.name }))
      .sort(byName),
    status: (Object.keys(STATUS_LABEL) as PositionStatus[])
      .filter((s) => statuses.includes(s))
      .map((s) => ({ value: s, label: STATUS_LABEL[s] })),
    runtime: snapshot.runtimes.map((r) => ({ value: r.id, label: r.label })),
    company: companies.map((c) => ({ value: c, label: c })).sort(byName),
    rank: ranks
      .filter((k) => active.some((p) => p.kind === k))
      .map((k) => ({ value: k, label: rankName(titles, k) })),
    specialty: [...specialties].map(([value, label]) => ({ value, label })).sort(byName),
  };
}
