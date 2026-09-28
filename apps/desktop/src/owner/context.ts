import { createContext, useContext } from "react";
import type { OwnerProfile, OwnerProfileInput } from "@plenipo/types";

/** Your tile (Phase 18, ADR-056): your picture, status, mood, and message. */
export interface OwnerApi {
  /** As kept in the Ledger; `null` until it is loaded. */
  profile: OwnerProfile | null;
  /** Keep a change; resolves with your tile as kept, or refuses with the reason. */
  save: (input: OwnerProfileInput) => Promise<OwnerProfile>;
}

export const OwnerContext = createContext<OwnerApi | null>(null);

/** Without an `OwnerProvider` (a view shown on its own): nothing to show, and nothing to keep. */
const WITHOUT_PROVIDER: OwnerApi = {
  profile: null,
  save: () =>
    Promise.reject(new Error("Your picture, status, mood, and message can't change here.")),
};

/** Your tile, from `OwnerProvider`. Never throws: without one, there is no profile yet. */
export function useOwnerProfile(): OwnerApi {
  return useContext(OwnerContext) ?? WITHOUT_PROVIDER;
}
