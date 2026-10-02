import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import type { PhoneAsk } from "@plenipo/types";

import { Lost, Meeting, NoAnswer } from "./line/phone";
import { LineEnded, RelayRefused, type SocketMaker } from "./line/relay";
import { answerChallenge } from "./lock/passkey";
import type { Keep, Kept } from "./keep";
import { SessionContext, type Organization, type Status } from "./session-context";

const ORG_KEY = "plenipo.remote.org";

function rememberedOrg(): string {
  try {
    return localStorage.getItem(ORG_KEY) ?? "first";
  } catch {
    return "first";
  }
}

/**
 * The phone's meeting with its PC, for every page: connecting, signing in, which organization, and
 * asking. When the connection drops, the page says so and changes nothing; a request whose answer
 * did not arrive is asked about once the PC is back.
 */
export function SessionProvider({
  kept,
  keep,
  onForgotten,
  make,
  children,
}: {
  kept: Kept;
  keep: Keep;
  onForgotten: () => void;
  make?: SocketMaker;
  children: ReactNode;
}) {
  const [status, setStatus] = useState<Status>({ kind: "connecting" });
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [org, setOrgState] = useState<string>(rememberedOrg);
  const [unknown, setUnknown] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const current = useRef(kept);
  useEffect(() => {
    current.current = kept;
  }, [kept]);

  const setOrg = useCallback((id: string) => {
    setOrgState(id);
    try {
      localStorage.setItem(ORG_KEY, id);
    } catch {
      // A private window: the choice lasts this visit.
    }
  }, []);

  const loadOrganizations = useCallback(async (m: Meeting) => {
    const reply = await m.ask({ kind: "readOrganizations" });
    const list = (reply.ok as { organizations?: Organization[] } | undefined)?.organizations;
    if (list && list.length > 0) {
      setOrganizations(list);
      setOrgState((o) => (list.some((x) => x.id === o) ? o : list[0]!.id));
    }
  }, []);

  /** Meet the PC (state changes only after the first answer). */
  const open = useCallback(async () => {
    let m: Meeting;
    try {
      m = await Meeting.open({
        paired: current.current.paired,
        keys: current.current.keys,
        ...(make ? { make } : {}),
      });
    } catch (e) {
      // The relay no longer takes this phone's pass: the PC removed it.
      if (
        (e instanceof RelayRefused && e.code === "bad_pass") ||
        (e instanceof LineEnded && e.why === "notListed")
      ) {
        setStatus({ kind: "notListed" });
      } else if (e instanceof RelayRefused || e instanceof LineEnded)
        setStatus({ kind: "offline" });
      // The meeting itself failed: the PC does not know this phone's key any more.
      else setStatus({ kind: "notListed" });
      return;
    }
    m.onEnd((e) => {
      setMeeting((now) => (now === m ? null : now));
      setStatus((s) =>
        s.kind === "notListed" || e.why === "notListed"
          ? { kind: "notListed" }
          : { kind: "offline" },
      );
    });
    m.onEvent((event) => {
      if (event.kind !== "signedOut") return;
      if (event.why === "removed") {
        void keep.forget().then(onForgotten);
        return;
      }
      setStatus({ kind: "signIn" });
    });
    setMeeting(m);
    if (m.welcome.signedIn) {
      setStatus({ kind: "ready" });
      await loadOrganizations(m).catch(() => undefined);
    } else {
      setStatus({ kind: "signIn" });
    }
  }, [keep, loadOrganizations, make, onForgotten]);

  const connect = useCallback(() => {
    setStatus({ kind: "connecting" });
    void open();
  }, [open]);

  // The first meeting, once the page is on screen (a timer, so a page shown twice in a row by
  // React's strict mode still meets the PC once).
  useEffect(() => {
    const t = setTimeout(() => void open(), 0);
    return () => clearTimeout(t);
  }, [open]);

  // A request whose outcome is unknown: ask what happened once the PC is back.
  useEffect(() => {
    if (status.kind !== "ready" || !meeting || !unknown) return;
    let live = true;
    meeting
      .ask({ kind: "outcome", of: unknown })
      .then((reply) => {
        if (!live) return;
        setUnknown(null);
        const known = (reply.ok as { known?: boolean } | undefined)?.known;
        setNote(
          known
            ? "Your PC got your last request before the connection dropped."
            : "Your PC did not get your last request. Try it again if you still want it.",
        );
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [status.kind, meeting, unknown]);

  const signIn = useCallback(async () => {
    const m = meeting;
    const challenge = m?.welcome.challenge;
    if (!m || !challenge) {
      connect();
      return;
    }
    const answer = await answerChallenge(challenge, current.current.paired.credential);
    const reply = await m.ask({ kind: "signIn", answer });
    if (reply.ok) {
      const pass = (reply.ok as { pass?: string }).pass;
      if (pass) {
        const next = { ...current.current, paired: { ...current.current.paired, pass } };
        current.current = next;
        await keep.save(next);
      }
      setStatus({ kind: "ready" });
      await loadOrganizations(m).catch(() => undefined);
      return;
    }
    // A refused check uses the meeting's one challenge: the next try is a new meeting.
    const problem =
      reply.refused?.message ?? reply.failed ?? "Your PC could not check that it is you.";
    m.close();
    setStatus({ kind: "signIn", problem });
    if (reply.refused?.why === "paused") return;
    void open();
  }, [connect, keep, loadOrganizations, meeting, open]);

  const ask = useCallback(
    async (a: PhoneAsk, again = false) => {
      if (!meeting) throw new LineEnded("closed");
      try {
        return await meeting.ask(a, { again });
      } catch (e) {
        if ((e instanceof Lost || e instanceof NoAnswer) && !a.kind.startsWith("read")) {
          setUnknown(e.request);
        }
        throw e;
      }
    },
    [meeting],
  );

  const leave = useCallback(
    async (how: "signOut" | "remove") => {
      if (meeting) {
        await meeting
          .ask(how === "signOut" ? { kind: "signOut" } : { kind: "removeThisPhone" })
          .catch(() => undefined);
        meeting.close();
      }
      if (how === "remove") {
        await keep.forget();
        onForgotten();
      } else {
        connect();
      }
    },
    [connect, keep, meeting, onForgotten],
  );

  return (
    <SessionContext.Provider
      value={{
        status,
        kept,
        meeting,
        organizations,
        org,
        setOrg,
        unknown,
        note,
        clearNote: () => setNote(null),
        connect,
        signIn,
        leave,
        ask,
      }}
    >
      {children}
    </SessionContext.Provider>
  );
}
