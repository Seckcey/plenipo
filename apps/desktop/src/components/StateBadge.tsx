import type { ExecutionRecord } from "@plenipo/types";
import { StatusPill } from "@plenipo/ui";

import { outcomeText } from "../runtime/format";
import { EXECUTION_TONE } from "./tones";

export function StateBadge({ record }: { record: ExecutionRecord }) {
  return <StatusPill status={EXECUTION_TONE[record.state]} label={outcomeText(record)} />;
}
