import { Meeting } from "./line/phone";
import type { SocketMaker } from "./line/relay";
import { browserKeep, type Keep } from "./keep";
import type { Tap } from "./notice";

/** What answering from a notice came to, in plain words when it did not go. */
export type Answered = { ok: true } | { ok: false; message: string };

/**
 * **Refuse** an approval or **Discard** a lesson right from its notice (ADR-142 §5), with no
 * sign-in: the phone meets its PC as "from a notice", and the PC takes only a no that way. Every
 * other answer opens Plenipo, where the phone checks it is you first.
 */
export async function answerFromNotice(
  tap: Extract<Tap, { kind: "refuse" } | { kind: "discard" }>,
  keep: Keep = browserKeep(),
  make?: SocketMaker,
): Promise<Answered> {
  const kept = await keep.load().catch(() => null);
  if (!kept) return { ok: false, message: "This phone is no longer paired with your PC." };
  let meeting: Meeting;
  try {
    meeting = await Meeting.open({
      paired: kept.paired,
      keys: kept.keys,
      notice: true,
      ...(make ? { make } : {}),
    });
  } catch {
    return { ok: false, message: "Your PC can’t be reached. Nothing was changed." };
  }
  try {
    const reply = await meeting.ask(
      tap.kind === "refuse"
        ? { kind: "refuse", org: tap.org, approval: tap.approval }
        : { kind: "discardLesson", org: tap.org, lesson: tap.lesson },
    );
    if (reply.ok !== undefined) return { ok: true };
    return {
      ok: false,
      message: reply.refused?.message ?? reply.failed ?? "Your PC did not take that answer.",
    };
  } catch {
    return { ok: false, message: "We don't know if your PC got this. Open Plenipo to check." };
  } finally {
    meeting.close();
  }
}
