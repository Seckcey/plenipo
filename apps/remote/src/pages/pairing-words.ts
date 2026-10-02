import { RelayRefused } from "../line/relay";

/** Why pairing stopped, in plain words. */
export function pairingProblem(e: unknown): string {
  if (e instanceof RelayRefused) {
    switch (e.code) {
      case "mailbox_closed":
        return "That code didn't work. Check it, or press Add a phone on your PC again: a code lasts 10 minutes and works once.";
      case "too_many_tries":
        return "Too many tries with that code. Press Add a phone on your PC again for a new one.";
      case "pc_offline":
        return "Your PC can't be reached. Make sure Plenipo is running on it.";
    }
  }
  if (e instanceof Error && e.message && !/^the /.test(e.message)) return e.message;
  return "That didn't work: the code may be wrong or used. Press Add a phone on your PC again.";
}
