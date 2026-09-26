import { createContext } from "react";

import type { RuntimeState } from "./store";

export interface RuntimeContextValue {
  state: RuntimeState;
  start: (profileId: string) => Promise<string>;
  cancel: (executionId: string) => Promise<void>;
  loadOutput: (executionId: string) => Promise<void>;
  reload: () => Promise<void>;
}

export const RuntimeContext = createContext<RuntimeContextValue | null>(null);
