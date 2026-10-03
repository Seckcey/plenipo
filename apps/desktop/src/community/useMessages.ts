import { useCallback, useEffect, useRef, useState } from "react";
import type { ConversationSummary } from "@plenipo/types";

import { communityConversations, toCommandError } from "../api/commands";
import { subscribeCommunityMessages } from "../api/events";

/**
 * Your conversations, kept live (Phase 24, ADR-164): read when shown, and again whenever messages
 * arrive or change. `changes` counts those changes, for the open conversation to hear.
 *
 * Reading the list tells nothing to anyone, so this is safe to do for each change. Reading an
 * open conversation does (it marks its messages seen, and tells every window so the unseen counts
 * move), which is why the open conversation does not read again for each change (`useConversation`).
 */
export function useMessages(): {
  list: ConversationSummary[] | null;
  error: string | null;
  /** When the list was last read (`Date.now()`), so the open conversation can tell what is newer. */
  listAt: number;
  reload: () => void;
  changes: number;
} {
  const [list, setList] = useState<ConversationSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [listAt, setListAt] = useState(0);
  const [changes, setChanges] = useState(0);
  const alive = useRef(true);
  // An answer for a question that was since asked again is dropped, so the list never goes back.
  const turn = useRef(0);
  const reload = useCallback(() => {
    const mine = ++turn.current;
    communityConversations()
      .then((next) => {
        if (!alive.current || mine !== turn.current) return;
        setList(next);
        setListAt(Date.now());
        setError(null);
      })
      .catch((reason: unknown) => {
        if (alive.current && mine === turn.current) setError(toCommandError(reason).message);
      });
  }, []);
  useEffect(() => {
    alive.current = true;
    let stop: (() => void) | null = null;
    void subscribeCommunityMessages(() => {
      if (alive.current) setChanges((n) => n + 1);
      reload();
    })
      .then((s) => {
        if (alive.current) stop = s;
        else s();
      })
      .catch(() => undefined)
      .finally(reload);
    return () => {
      alive.current = false;
      stop?.();
    };
  }, [reload]);
  return { list, error, listAt, reload, changes };
}

/** The unseen messages in all conversations, for the number on the Messages tab. */
export function unseenCount(list: readonly ConversationSummary[] | null): number {
  return (list ?? []).reduce((sum, c) => sum + c.unseen, 0);
}
