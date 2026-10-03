import { sentenceStart, systemWords } from "../system/words";
import { oneLine } from "./safeText";

function str(v: unknown): string | null {
  return typeof v === "string" && v.trim() ? v : null;
}

/** Each part of your profile, as What people see names it (ADR-163 §2). */
const PART_WORDS = new Map<unknown, string>([
  ["picture", "picture"],
  ["name", "name"],
  ["status", "status"],
  ["mood", "mood"],
  ["message", "message"],
  ["company", "company"],
  ["business", "what your business does"],
  ["region", "where"],
]);

/** "a", "a and b", or "a, b, and c". */
function listed(words: string[]): string {
  if (words.length < 3) return words.join(" and ");
  return `${words.slice(0, -1).join(", ")}, and ${words[words.length - 1]}`;
}

/**
 * Activity's line for a Community event (Phase 24, ADR-162 §9, ADR-163 §11, ADR-167 §16), or
 * null when the event is not Community's. The Ledger never holds a pass, a key, a code, a birth
 * date, or anyone's words, so neither does this. Names came from outside Plenipo, so they are
 * shown as one plain line.
 */
export function describeCommunityEvent(type: string, p: Record<string, unknown>): string | null {
  switch (type) {
    case "community.signed_in": {
      const account = str(p.account);
      const pc = str(p.pc);
      return `Signed in to Community${account ? ` as ${oneLine(account)}` : ""}${pc ? ` on ${oneLine(pc)}` : ""}`;
    }
    case "community.joined":
      return str(p.name)
        ? `Joined Community as @${oneLine(str(p.name) ?? "")}`
        : "Joined Community";
    case "community.signed_out":
      switch (p.why) {
        case "removed":
          return `${sentenceStart(systemWords().thisComputer)} was removed from Community on the account site, so it is signed out`;
        case "too_young":
          return `Community is for people 13 and older: ${systemWords().thisComputer} signed out, and nothing was kept`;
        default:
          return `You signed ${systemWords().thisComputer} out of Community`;
      }
    case "community.left":
      return "You left Community";
    case "community.profile_changed": {
      const shown = Array.isArray(p.shown)
        ? p.shown.flatMap((part: unknown) => PART_WORDS.get(part) ?? [])
        : [];
      return shown.length > 0
        ? `You changed your Community profile. People see your ${listed(shown)}`
        : "You changed your Community profile. People see none of it";
    }
    case "community.appeared_offline":
      return "You chose Appear offline in Community";
    case "community.appeared_online":
      return "You turned off Appear offline in Community";
    default:
      return null;
  }
}
