import { useCallback, useEffect, useRef, useState } from "react";
import type { OrgSnapshot, TilePlace } from "@plenipo/types";

import { placeTiles, tidyUp, toCommandError } from "../api/commands";
import { mergePlaces } from "./arrange";

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
      setLocal({ places: next, savedAt: null });
      try {
        await work();
        if (seq.current === mine) setLocal({ places: next, savedAt: Date.now() });
        return true;
      } catch (reason) {
        if (seq.current === mine) setLocal(null);
        onError(toCommandError(reason).message);
        return false;
      }
    },
    [onError],
  );

  /** Save these spots (a moved tile, and its team where needed). */
  const place = useCallback(
    (changes: TilePlace[]) =>
      changes.length === 0
        ? Promise.resolve(true)
        : save(mergePlaces(current.current, changes), () => placeTiles(changes)),
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
