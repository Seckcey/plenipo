/** "9 minutes", "1 minute", "less than a minute". */
export function minutesLeft(endsAt: number, now: number): string {
  const ms = endsAt - now;
  if (ms < 60_000) return "less than a minute";
  const m = Math.floor(ms / 60_000);
  return m === 1 ? "1 minute" : `${m} minutes`;
}

const str = (v: unknown): string | null => (typeof v === "string" && v.trim() ? v.trim() : null);

/** Why Guard refused a phone's request (ADR-145), in plain words. */
const REFUSED_WORDS: Record<string, string> = {
  switchedOff: "using Plenipo from another device is switched off",
  notPro: "using Plenipo from your phone is part of Pro",
  unknownPhone: "this PC doesn't know that phone",
  paused: "the phone is paused",
  notSignedIn: "the phone was not signed in",
  notFromANotice: "that needs Plenipo open on the phone",
  keptOnPc: "you keep that kind of approval on this PC",
  notAnApproval: "it was not an approval waiting for you",
  copied: "it was a copy of a request already made",
};

/** Why a phone was signed out, in plain words. */
const SIGNED_OUT_WORDS: Record<string, string> = {
  you: "",
  idle: " after 30 minutes without use",
  twelve_hours: " after 12 hours",
  removed: ": it was removed",
  switched_off: ": using Plenipo from another device was switched off",
  pro_ended: ": Pro ended",
  paused: ": it was paused",
};

/** Why adding a phone did not finish, in plain words. */
function pairingRefused(p: Record<string, unknown>): string {
  switch (str(p.reason)) {
    case "cancelled":
      return "Adding a phone was cancelled";
    case "not_added":
      return `You did not add ${str(p.name) ?? "a phone"}: you said it was not your phone`;
    case "phone_left":
      return "Adding a phone stopped: the phone left before you answered";
    case "wrong_code":
      return p.pairingPaused === true
        ? "Wrong pairing codes were tried: Add a phone is paused for 15 minutes"
        : "A wrong pairing code was tried: the code stops working after 3";
    case "passkey":
      return `Adding a phone stopped: its face, fingerprint, or passcode could not be checked${
        str(p.problem) ? ` (${str(p.problem)})` : ""
      }`;
    case "expired":
      return "A pairing code ran out of time";
    default:
      return "Adding a phone did not finish";
  }
}

/**
 * Phase 14 (ADR-145): your phones, and every request from one, with the phone that sent it
 * ("Frank's phone asked to approve"). `null` for any other event.
 */
export function describeRemoteEvent(type: string, p: Record<string, unknown>): string | null {
  const phone = str(p.name) ?? "A phone";
  const asked = str(p.kind) ?? "do something";
  switch (type) {
    case "remote.switched_on":
      return "You turned on using Plenipo from another device";
    case "remote.switched_off":
      return "You turned off using Plenipo from another device";
    case "remote.kept_on_pc_changed":
      return "You changed which approvals are kept on this PC";
    case "remote.relay_connected":
      return "This PC connected to 8 West’s relay, so your phones can reach it";
    case "remote.relay_disconnected":
      return "This PC disconnected from 8 West’s relay";
    case "remote.meetings_stopped":
      return "Someone kept trying to reach this PC as a phone: new phones are stopped for a while";
    case "remote.pairing_refused":
      return pairingRefused(p);
    case "remote.device_added":
      return `You added ${phone}${str(p.browser) ? ` (${str(p.browser)})` : ""}`;
    case "remote.device_renamed":
      return `You renamed a phone: ${phone}`;
    case "remote.device_removed":
      return p.by === "phone"
        ? `${phone} was removed from the phone itself`
        : `You removed ${phone}`;
    case "remote.device_paused":
      return `${phone} was paused after 3 failed checks`;
    case "remote.device_unpaused":
      return `You un-paused ${phone}`;
    case "remote.signed_in":
      return `${phone} signed in`;
    case "remote.signed_out":
      return `${phone} signed out${SIGNED_OUT_WORDS[str(p.why) ?? "you"] ?? ""}`;
    case "remote.check_refused":
      return `${phone}: the face, fingerprint, or passcode check did not pass`;
    case "remote.request":
      return `${phone} asked to ${asked}${p.fromNotice === true ? " (from a notice)" : ""}`;
    case "remote.refused": {
      const why = REFUSED_WORDS[str(p.why) ?? ""];
      return `Guard refused ${phone}: ${asked}${why ? ` (${why})` : ""}`;
    }
  }
  return null;
}
