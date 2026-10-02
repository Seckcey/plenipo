import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { PhoneAsk, PhoneChanged, PhoneReply } from "@plenipo/types";

import type { Meeting } from "./line/phone";
import type { Kept } from "./keep";

/** Where the phone stands with its PC. */
export type Status =
  | { kind: "connecting" }
  /** The PC is off, or not connected to the relay: nothing was changed. */
  | { kind: "offline" }
  /** The PC no longer knows this phone (removed, or the pass ran out). */
  | { kind: "notListed" }
  | { kind: "signIn"; problem?: string }
  | { kind: "ready" };

export interface Organization {
  id: string;
  name: string;
}

export interface Session {
  status: Status;
  kept: Kept;
  meeting: Meeting | null;
  organizations: Organization[];
  org: string;
  setOrg: (id: string) => void;
  /** A request sent before the connection dropped, whose outcome is not known yet. */
  unknown: string | null;
  /** What the PC said about that request once it was back. */
  note: string | null;
  clearNote: () => void;
  connect: () => void;
  signIn: () => Promise<void>;
  /** Sign out, or remove this phone: either way the page goes back to its start. */
  leave: (how: "signOut" | "remove") => Promise<void>;
  /** Ask the PC, tracking a lost connection. */
  ask: (ask: PhoneAsk, again?: boolean) => Promise<PhoneReply>;
}

export const SessionContext = createContext<Session | null>(null);

export function useSession(): Session {
  const s = useContext(SessionContext);
  if (!s) throw new Error("useSession outside SessionProvider");
  return s;
}

/**
 * What a page read from the PC (`request`; `null`: nothing to read), kept fresh when the PC says
 * that page changed (`changes`).
 */
export function useRead<T>(
  request: PhoneAsk | null,
  changes: PhoneChanged[] = [],
): { data: T | null; error: string | null; reload: () => void } {
  const { meeting, ask, org } = useSession();
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const live = useRef(true);
  const key = request ? JSON.stringify(request) : null;
  const watched = changes.join(",");
  const read = useCallback(
    (again: boolean) => {
      if (!key) return;
      ask(JSON.parse(key) as PhoneAsk, again)
        .then((reply) => {
          if (!live.current) return;
          if (reply.ok !== undefined) {
            setData(reply.ok as T);
            setError(null);
          } else {
            setError(reply.refused?.message ?? reply.failed ?? "Your PC could not answer.");
          }
        })
        .catch(() => {
          if (live.current) setError("Your PC can't be reached. Nothing was changed.");
        });
    },
    [ask, key],
  );
  useEffect(() => {
    live.current = true;
    read(false);
    return () => {
      live.current = false;
    };
  }, [read]);
  useEffect(() => {
    if (!meeting || !watched) return;
    const wanted = watched.split(",");
    return meeting.onEvent((event) => {
      if (event.kind !== "changed" || !wanted.includes(event.what)) return;
      if (event.org && event.org !== org) return;
      read(true);
    });
  }, [meeting, org, read, watched]);
  return { data, error, reload: () => read(true) };
}
