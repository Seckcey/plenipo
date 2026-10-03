import { RelayRefused } from "../line/relay";

/** Why pairing stopped, in plain words. */
export function pairingProblem(e: unknown): string {
  if (e instanceof RelayRefused) {
    switch (e.code) {
      // The code was mistyped, ran out, or was used already. In the last case the PC may be asking
      // about a phone that is not the owner's (ADR-212), so: look at the PC first.
      case "mailbox_closed":
        return "This code was already used, or mistyped, or your PC stopped adding a phone. Look at your PC: if it is asking about a phone that is not yours, click Cancel and start again. Otherwise check the code, or press Add a phone on your PC for a new one. A code lasts 10 minutes and works once.";
      case "too_many_tries":
        return "Too many tries with that code. Press Add a phone on your PC again for a new one.";
      case "pc_offline":
        return "Your PC can't be reached. Make sure Plenipo is running on it.";
    }
  }
  if (e instanceof Error && e.message && !/^the /.test(e.message)) return e.message;
  return "That didn't work: the code may be wrong or used. Press Add a phone on your PC again.";
}
