import { useContext } from "react";

import { RuntimeContext, type RuntimeContextValue } from "./context";

export function useRuntime(): RuntimeContextValue {
  const value = useContext(RuntimeContext);
  if (!value) throw new Error("useRuntime must be used inside <RuntimeProvider>");
  return value;
}
