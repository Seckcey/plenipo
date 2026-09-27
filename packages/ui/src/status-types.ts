/** The status ramp (tokens `ok`, `warn`, `error`, `offline`, `pending`). */
export type Status = "ok" | "warn" | "error" | "offline" | "pending";

export const STATUSES: readonly Status[] = ["ok", "warn", "error", "offline", "pending"];
