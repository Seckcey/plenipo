/**
 * The words and lists for your profile in Community (Phase 24, ADR-163): the boxes for **What
 * people see**, the kinds of business, and **Where** (a state or a country, never a town).
 */

import type { ProfileDraft, ProfileShown } from "@plenipo/types";

/** The longest name on a card, as the Rust side and the contract count it. */
export const MOST_NAME = 60;
/** The longest company and the longest line about your business. */
export const MOST_LINE = 80;
/** The most kinds of business you can choose. */
export const MOST_KINDS = 3;

/** One box per part of your profile, in the order they are shown. */
export const PARTS: readonly { key: keyof ProfileShown; label: string }[] = [
  { key: "picture", label: "Show my picture" },
  { key: "name", label: "Show my name" },
  { key: "status", label: "Show my status" },
  { key: "mood", label: "Show my mood" },
  { key: "message", label: "Show my message" },
  { key: "company", label: "Show my company" },
  { key: "business", label: "Show what my business does" },
  { key: "region", label: "Show where I am" },
];

/** Two profiles are the same when every text, kind, and box is the same (kinds in the same order). */
export function sameProfile(a: ProfileDraft, b: ProfileDraft): boolean {
  return (
    a.displayName === b.displayName &&
    a.company === b.company &&
    a.businessLine === b.businessLine &&
    a.region === b.region &&
    a.businessKinds.length === b.businessKinds.length &&
    a.businessKinds.every((kind, i) => kind === b.businessKinds[i]) &&
    PARTS.every(({ key }) => a.shown[key] === b.shown[key])
  );
}

/**
 * The parts 8 West can hide (contract `HiddenPart`), as the note names them: "8 West hid your
 * picture because it broke the Community rules." Any other word is left alone.
 */
export const HIDDEN_PART_WORDS: ReadonlyMap<string, string> = new Map([
  ["picture", "picture"],
  ["display_name", "name"],
  ["message", "message"],
  ["company", "company"],
  ["business_line", "line about your business"],
]);

/** The note for a part 8 West hid. */
export function hiddenNote(word: string): string {
  return `8 West hid your ${word} because it broke the Community rules.`;
}

/**
 * The kinds of business: the contract's words (`BusinessKind`) and the plain words shown for each.
 */
export const BUSINESS_KINDS: readonly { value: string; label: string }[] = [
  { value: "accounting", label: "Accounting" },
  { value: "agriculture", label: "Farming" },
  { value: "automotive", label: "Cars and trucks" },
  { value: "construction", label: "Construction" },
  { value: "consulting", label: "Consulting" },
  { value: "design", label: "Design" },
  { value: "education", label: "Teaching" },
  { value: "energy", label: "Energy" },
  { value: "engineering", label: "Engineering" },
  { value: "finance", label: "Finance" },
  { value: "food_and_drink", label: "Food and drink" },
  { value: "government", label: "Government" },
  { value: "healthcare", label: "Health care" },
  { value: "hospitality", label: "Hotels and travel stays" },
  { value: "insurance", label: "Insurance" },
  { value: "it_services", label: "IT services" },
  { value: "legal", label: "Legal" },
  { value: "logistics", label: "Shipping and delivery" },
  { value: "manufacturing", label: "Manufacturing" },
  { value: "marketing", label: "Marketing" },
  { value: "media", label: "Media" },
  { value: "nonprofit", label: "Nonprofit" },
  { value: "real_estate", label: "Real estate" },
  { value: "retail", label: "Shops" },
  { value: "security", label: "Security" },
  { value: "software", label: "Software" },
  { value: "telecom", label: "Phones and internet" },
  { value: "trades", label: "Trades" },
  { value: "travel", label: "Travel" },
  { value: "other", label: "Other" },
];

/** The plain word for a kind, or `null` for a word this copy of Plenipo does not know. */
export function kindLabel(value: string): string | null {
  return BUSINESS_KINDS.find((k) => k.value === value)?.label ?? null;
}

/** The 50 states and the District of Columbia, as `US-XX` (ISO 3166-2). */
const STATES: readonly (readonly [code: string, name: string])[] = [
  ["US-AL", "Alabama"],
  ["US-AK", "Alaska"],
  ["US-AZ", "Arizona"],
  ["US-AR", "Arkansas"],
  ["US-CA", "California"],
  ["US-CO", "Colorado"],
  ["US-CT", "Connecticut"],
  ["US-DE", "Delaware"],
  ["US-DC", "District of Columbia"],
  ["US-FL", "Florida"],
  ["US-GA", "Georgia"],
  ["US-HI", "Hawaii"],
  ["US-ID", "Idaho"],
  ["US-IL", "Illinois"],
  ["US-IN", "Indiana"],
  ["US-IA", "Iowa"],
  ["US-KS", "Kansas"],
  ["US-KY", "Kentucky"],
  ["US-LA", "Louisiana"],
  ["US-ME", "Maine"],
  ["US-MD", "Maryland"],
  ["US-MA", "Massachusetts"],
  ["US-MI", "Michigan"],
  ["US-MN", "Minnesota"],
  ["US-MS", "Mississippi"],
  ["US-MO", "Missouri"],
  ["US-MT", "Montana"],
  ["US-NE", "Nebraska"],
  ["US-NV", "Nevada"],
  ["US-NH", "New Hampshire"],
  ["US-NJ", "New Jersey"],
  ["US-NM", "New Mexico"],
  ["US-NY", "New York"],
  ["US-NC", "North Carolina"],
  ["US-ND", "North Dakota"],
  ["US-OH", "Ohio"],
  ["US-OK", "Oklahoma"],
  ["US-OR", "Oregon"],
  ["US-PA", "Pennsylvania"],
  ["US-RI", "Rhode Island"],
  ["US-SC", "South Carolina"],
  ["US-SD", "South Dakota"],
  ["US-TN", "Tennessee"],
  ["US-TX", "Texas"],
  ["US-UT", "Utah"],
  ["US-VT", "Vermont"],
  ["US-VA", "Virginia"],
  ["US-WA", "Washington"],
  ["US-WV", "West Virginia"],
  ["US-WI", "Wisconsin"],
  ["US-WY", "Wyoming"],
];

/**
 * The countries and territories where people live (ISO 3166-1 alpha-2), as a fixed list: the
 * names come from the system's own language data, so a country is never typed in. The uninhabited
 * places (Antarctica and a few islands) are left out.
 */
const COUNTRIES: readonly string[] = (
  "AD AE AF AG AI AL AM AO AR AS AT AU AW AX AZ " +
  "BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT BW BY BZ " +
  "CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ " +
  "DE DJ DK DM DO DZ " +
  "EC EE EG EH ER ES ET " +
  "FI FJ FK FM FO FR " +
  "GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GS GT GU GW GY " +
  "HK HN HR HT HU " +
  "ID IE IL IM IN IO IQ IR IS IT " +
  "JE JM JO JP " +
  "KE KG KH KI KM KN KP KR KW KY KZ " +
  "LA LB LC LI LK LR LS LT LU LV LY " +
  "MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT MU MV MW MX MY MZ " +
  "NA NC NE NF NG NI NL NO NP NR NU NZ " +
  "OM " +
  "PA PE PF PG PH PK PL PM PN PR PS PT PW PY " +
  "QA " +
  "RE RO RS RU RW " +
  "SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ " +
  "TC TD TG TH TJ TK TL TM TN TO TR TT TV TW TZ " +
  "UA UG US UY UZ " +
  "VA VC VE VG VI VN VU " +
  "WF WS " +
  "YE YT " +
  "ZA ZM ZW"
).split(" ");

/** What the contract allows for a place: a country (`US`), or a country and a part (`US-CA`). */
export const REGION_PATTERN = /^[A-Z]{2}(-[A-Z0-9]{1,3})?$/;

/** One place in the list. */
export interface RegionOption {
  value: string;
  label: string;
  group?: string | undefined;
}

/** The name of a country in English, or its code when the system has no name for it. */
function countryName(code: string, names: Intl.DisplayNames | null): string {
  try {
    const name = names?.of(code);
    return name && name !== code ? name : code;
  } catch {
    return code;
  }
}

let cached: RegionOption[] | null = null;

/** "Not shown", the states, and then the countries by name. Built once. */
function allRegions(): RegionOption[] {
  if (cached) return cached;
  let names: Intl.DisplayNames | null = null;
  try {
    names = new Intl.DisplayNames(["en"], { type: "region" });
  } catch {
    names = null;
  }
  const countries = COUNTRIES.map((code) => ({
    value: code,
    label: countryName(code, names),
    group: "Countries",
  })).sort((a, b) => a.label.localeCompare(b.label, "en"));
  cached = [
    { value: "", label: "Not shown" },
    ...STATES.map(([value, name]) => ({
      value,
      label: `${name} (United States)`,
      group: "States",
    })),
    ...countries,
  ];
  return cached;
}

/**
 * The places to choose from, with "Not shown" first. A place already kept that is not in the list
 * (a newer copy of Plenipo may know more) is added, so the box shows what is really kept.
 */
export function regionOptions(current: string): RegionOption[] {
  const all = allRegions();
  if (current === "" || all.some((o) => o.value === current)) return all;
  return [...all, { value: current, label: current, group: "Other places" }];
}

/** The words for a place: "California (United States)", "Canada", or the code as it is. */
export function regionLabel(value: string): string {
  return regionOptions(value).find((o) => o.value === value)?.label ?? value;
}
