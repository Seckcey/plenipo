/**
 * What the app calls each rank of the chain of command. The owner picks a set in Settings; the
 * default is the plain business chain: Worker → Supervisor → Manager → VP → President (the owner).
 * Display only: agents are always given the plain titles (ADR-010). Job titles ("Website
 * Supervisor") are the owner's own words and never change with the set.
 */
import type { OrgSnapshot, PositionKind, TitleTheme } from "@plenipo/types";

/** The owner at the top, then the four kinds of position. */
export type Rank = "owner" | PositionKind;

export interface RankName {
  one: string;
  many: string;
}

export interface TitleSet {
  theme: TitleTheme;
  /** Its name in the picker. */
  label: string;
  group: "Business" | "Military" | "Just for fun";
  ranks: Record<Rank, RankName>;
}

/** Top rank first. */
export const RANKS: Rank[] = [
  "owner",
  "superintendent",
  "departmentManager",
  "projectCoordinator",
  "worker",
];

const r = (one: string, many = `${one}s`): RankName => ({ one, many });

function ranks(
  owner: RankName,
  superintendent: RankName,
  departmentManager: RankName,
  projectCoordinator: RankName,
  worker: RankName,
): Record<Rank, RankName> {
  return { owner, superintendent, departmentManager, projectCoordinator, worker };
}

const BUSINESS: TitleSet = {
  theme: "business",
  label: "Business",
  group: "Business",
  ranks: ranks(r("President"), r("VP"), r("Manager"), r("Supervisor"), r("Worker")),
};

export const TITLE_SETS: TitleSet[] = [
  BUSINESS,
  {
    theme: "army",
    label: "U.S. Army",
    group: "Military",
    ranks: ranks(r("General"), r("Colonel"), r("Captain"), r("Sergeant"), r("Private")),
  },
  {
    theme: "navy",
    label: "U.S. Navy",
    group: "Military",
    ranks: ranks(
      r("Admiral"),
      r("Captain"),
      r("Lieutenant"),
      r("Chief Petty Officer"),
      r("Seaman", "Seamen"),
    ),
  },
  {
    theme: "airForce",
    label: "U.S. Air Force",
    group: "Military",
    ranks: ranks(
      r("General"),
      r("Colonel"),
      r("Captain"),
      r("Master Sergeant"),
      r("Airman", "Airmen"),
    ),
  },
  {
    theme: "marineCorps",
    label: "U.S. Marine Corps",
    group: "Military",
    ranks: ranks(
      r("General"),
      r("Colonel"),
      r("Captain"),
      r("Gunnery Sergeant"),
      r("Lance Corporal"),
    ),
  },
  {
    theme: "coastGuard",
    label: "U.S. Coast Guard",
    group: "Military",
    ranks: ranks(
      r("Admiral"),
      r("Captain"),
      r("Lieutenant"),
      r("Chief Petty Officer"),
      r("Seaman", "Seamen"),
    ),
  },
  {
    theme: "spaceForce",
    label: "U.S. Space Force",
    group: "Military",
    ranks: ranks(r("General"), r("Colonel"), r("Captain"), r("Master Sergeant"), r("Specialist")),
  },
  {
    theme: "mafia",
    label: "Mafia",
    group: "Just for fun",
    ranks: ranks(r("Don"), r("Underboss", "Underbosses"), r("Capo"), r("Soldier"), r("Associate")),
  },
];

/** The set for `theme` (Business for anything unknown). */
export function titleSet(theme: TitleTheme | null | undefined): TitleSet {
  return TITLE_SETS.find((t) => t.theme === theme) ?? BUSINESS;
}

/** The set the owner chose for this organization. */
export function titlesOf(snapshot: Pick<OrgSnapshot, "titles">): TitleSet {
  return titleSet(snapshot.titles);
}

/** "Supervisor", or "Supervisors" for `count` ≠ 1. */
export function rankName(t: TitleSet, rank: Rank, count = 1): string {
  return count === 1 ? t.ranks[rank].one : t.ranks[rank].many;
}

/** "a Supervisor", "an Underboss". */
export function withArticle(word: string): string {
  return `${/^[aeiou]/i.test(word) ? "an" : "a"} ${word}`;
}

/** The chain from the top: "President → VP → Manager → Supervisor → Worker". */
export function chainOf(t: TitleSet): string {
  return RANKS.map((rank) => t.ranks[rank].one).join(" → ");
}

/** A role's name as shown: a leadership template is called by its rank in the chosen set. */
export function roleLabel(
  t: TitleSet,
  role: { name: string; kind: PositionKind; template: boolean },
): string {
  return role.template && role.kind !== "worker" ? rankName(t, role.kind) : role.name;
}
