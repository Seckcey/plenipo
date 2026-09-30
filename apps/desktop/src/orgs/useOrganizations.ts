import { useCallback, useEffect, useState } from "react";
import type { OrgListing } from "@plenipo/types";

import { getOrganizations, toCommandError } from "../api/commands";
import { subscribeOrganizations } from "../api/events";

/** Your organizations, kept current when one is made, renamed, opened, or archived. */
export function useOrganizations() {
  const [listing, setListing] = useState<OrgListing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const reload = useCallback(async () => {
    try {
      setListing(await getOrganizations());
      setError(null);
    } catch (reason) {
      setError(toCommandError(reason).message);
    }
  }, []);
  useEffect(() => {
    let stop: (() => void) | null = null;
    let gone = false;
    const load = () =>
      getOrganizations().then(
        (next) => {
          if (gone) return;
          setListing(next);
          setError(null);
        },
        (reason: unknown) => {
          if (!gone) setError(toCommandError(reason).message);
        },
      );
    void load();
    void subscribeOrganizations(() => void load())
      .then((unlisten) => {
        if (gone) unlisten();
        else stop = unlisten;
      })
      .catch(() => undefined);
    return () => {
      gone = true;
      stop?.();
    };
  }, [reload]);
  return { listing, error, reload, apply: setListing };
}
