import { useCallback, useEffect, useRef, useState } from "react";
import type { OrgSnapshot, TilePlace } from "@plenipo/types";

import { placeTiles, tidyUp, toCommandError } from "../api/commands";
import { MAX_PLACES, mergePlaces } from "./arrange";

interface Local {
  places: TilePlace[];
  /** When the Ledger finished saving them (`null`: not yet). */
  savedAt: number | null;
}

/**
 * The tiles placed by hand (ADR-053 §3–§5): the organization's own list, with this window's
 * latest changes on top until the organization is read again after they were saved (saving a
 * spot records no event, so nothing else reloads it). A spot the Ledger refuses goes back.
 */
export function usePlaces(snapshot: OrgSnapshot | null, onError: (message: string) => void) {
  const [local, setLocal] = useState<Local | null>(null);
  const seq = useRef(0);
  const fromSnapshot = snapshot?.places ?? [];
  const places =
    local && (local.savedAt === null || local.savedAt >= (snapshot?.generatedAt ?? 0))
      ? local.places
      : fromSnapshot;
  const current = useRef(places);
  useEffect(() => {
    current.current = places;
  });

  const save = useCallback(
    async (next: TilePlace[], work: () => Promise<unknown>): Promise<boolean> => {
      const mine = ++seq.current;
      // What was on the canvas before: a refused save goes back to it (not to an older reading
      // of the organization, which may not have the spots saved since).
      const before = current.current;
      setLocal({ places: next, savedAt: null });
      try {
        await work();
        if (seq.current === mine) setLocal({ places: next, savedAt: Date.now() });
        return true;
      } catch (reason) {
        if (seq.current === mine) setLocal({ places: before, savedAt: Date.now() });
        onError(toCommandError(reason).message);
        return false;
      }
    },
    [onError],
  );

  /** Save these spots (a moved tile, and its team where needed; Undo can bring back many, so
   * they go in steps of at most 500, as the Ledger takes them). */
  const place = useCallback(
    (changes: TilePlace[]) =>
      changes.length === 0
        ? Promise.resolve(true)
        : save(mergePlaces(current.current, changes), async () => {
            for (let i = 0; i < changes.length; i += MAX_PLACES) {
              await placeTiles(changes.slice(i, i + MAX_PLACES));
            }
          }),
    [save],
  );

  /** Tidy up: forget every spot; resolves with them, for Undo (`null` when it failed). */
  const tidy = useCallback(async (): Promise<TilePlace[] | null> => {
    let removed: TilePlace[] = [];
    const ok = await save([], async () => {
      removed = await tidyUp();
    });
    return ok ? removed : null;
  }, [save]);

  return { places, place, tidy };
}
