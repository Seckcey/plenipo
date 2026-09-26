import type { ExecutionRecord } from "@plenipo/types";

import { outcomeText } from "../runtime/format";

export function StateBadge({ record }: { record: ExecutionRecord }) {
  return <span className={`badge badge--${record.state}`}>{outcomeText(record)}</span>;
}
