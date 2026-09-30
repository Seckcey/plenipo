import { useState } from "react";
import type {
  AgentRuntimeInfo,
  AiToolsPage,
  AiToolState,
  RoutingSnapshot,
  ToolInfo,
} from "@plenipo/types";
import { Panel, StatusPill, Tabs, type Status } from "@plenipo/ui";

import { runtimeStatus } from "../../agents/format";
import { useNow } from "../../runtime/useNow";
import { useTerminalIfAny } from "../../terminal/useTerminal";
import { PILL_TONE } from "../tones";
import type { Go } from "../views";
import { ModelsTab } from "./ModelsTab";
import { Overview } from "./Overview";
import { UsageTab } from "./UsageTab";
import { useToolUsage } from "./useAiTools";
import { midnight, MOVING } from "./words";

type CardTab = "overview" | "usage" | "models";

/**
 * A check that started this long before the sign-in tab's end was seen still counts as the check
 * after it (Plenipo starts it the moment the program ends).
 */
const CHECK_SLACK_MS = 2000;
/** "Checking…" gives up after this long without an answer. */
const CHECK_GIVE_UP_MS = 120_000;

/** The card's pill: checking, updating, given no tasks, or ready and why not. */
function cardStatus(
  info: AgentRuntimeInfo,
  tool: AiToolState | undefined,
  checking: boolean,
): { status: Status; label: string } {
  if (checking) return { status: "offline", label: "Checking…" };
  if (tool?.outOfService) return { status: "error", label: "No tasks for now" };
  if (tool && MOVING.has(tool.update.state) && tool.update.state !== "waiting") {
    return { status: "pending", label: "Updating…" };
  }
  // Until the page's own part arrives, a card cannot tell a paid AI tool from another.
  if (!tool && !info.ready) return { status: "offline", label: "Loading…" };
  // A paid AI tool is not signed in to: it has a key in use, or not (ADR-085). Paid keys
  // switched off (or no business cap) count even before the next check says so.
  if (tool?.payment === "paidKey" && (!info.ready || tool.paidBlocked !== null)) {
    return { status: PILL_TONE.warn, label: tool.paidKey ? "Key not in use" : "No key yet" };
  }
  const s = runtimeStatus(info);
  return { status: PILL_TONE[s.tone], label: s.text };
}

/**
 * One AI tool's card on the AI tools page (Phase 19): its AI company and state, then Overview
 * (sign-in, version and update, how it is paid for, limits, plan left, this week), Usage, and
 * Models.
 */
export function AiToolCard({
  info,
  tool,
  route,
  usageRevision,
  onApply,
  onRouting,
  go,
}: {
  info: AgentRuntimeInfo;
  tool: AiToolState | undefined;
  route: ToolInfo | undefined;
  usageRevision: number;
  onApply: (page: AiToolsPage) => void;
  onRouting: (snapshot: RoutingSnapshot) => void;
  /** Opens another page (a paid AI tool's Settings links). */
  go?: Go | undefined;
}) {
  const [tab, setTab] = useState<CardTab>("overview");
  const terminal = useTerminalIfAny();
  const now = useNow(5000);
  // Read again after midnight too: "Today" and "This week" move on.
  const usage = useToolUsage(info.id, usageRevision, midnight(now));
  // After its sign-in tab's program ended, Plenipo checks the tool again by itself (ADR-058 §4):
  // "Checking…" until that check's answer arrives.
  const ended = terminal?.signInEnded[info.id];
  const afterSignIn =
    ended !== undefined &&
    (info.checkedAt ?? 0) < ended - CHECK_SLACK_MS &&
    now - ended < CHECK_GIVE_UP_MS;
  const checking =
    afterSignIn ||
    tool?.checking === true ||
    info.auth.state === "checking" ||
    info.installation.state === "checking";
  const status = cardStatus(info, tool, checking);
  const prefix = `ai-tool-${info.id}`;

  return (
    <Panel
      id={prefix}
      title={info.label}
      className="ai-tool"
      actions={<StatusPill status={status.status} label={status.label} />}
    >
      <div className="card__meta">AI company: {info.providerLabel}</div>
      <Tabs<CardTab>
        label={`${info.label}: what to show`}
        value={tab}
        onChange={setTab}
        idPrefix={prefix}
        tabs={[
          { value: "overview", label: "Overview" },
          { value: "usage", label: "Usage" },
          { value: "models", label: "Models" },
        ]}
      />
      <div
        role="tabpanel"
        id={`${prefix}-panel-${tab}`}
        aria-labelledby={`${prefix}-tab-${tab}`}
        className="ai-tool__panel"
      >
        {tab === "overview" && (
          <Overview
            info={info}
            tool={tool}
            route={route}
            checking={checking}
            usage={usage}
            onApply={onApply}
            onRouting={onRouting}
            go={go}
          />
        )}
        {tab === "usage" && <UsageTab label={info.label} usage={usage} />}
        {tab === "models" && <ModelsTab info={info} tool={tool} route={route} onApply={onApply} />}
      </div>
    </Panel>
  );
}
