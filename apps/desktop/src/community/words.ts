import { sentenceStart, systemWords } from "../system/words";
import { oneLine } from "./safeText";

function str(v: unknown): string | null {
  return typeof v === "string" && v.trim() ? v : null;
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
    default:
      return null;
  }
}
