import { useState } from "react";
import type {
  AgentRuntimeInfo,
  AiToolsPage,
  AiToolState,
  RoutingSnapshot,
  ToolInfo,
} from "@plenipo/types";
import { Disclosure, Tabs, type Status } from "@plenipo/ui";

import { runtimeStatus } from "../../agents/format";
import { useNow } from "../../runtime/useNow";
import { useTerminalIfAny } from "../../terminal/useTerminal";
import { PILL_TONE } from "../tones";
import type { Go } from "../views";
import { ModelsTab } from "./ModelsTab";
import { QuickSignIn } from "./SignIn";
import { KeyWeekLine, Overview } from "./Overview";
import { UsageTab } from "./UsageTab";
import { useToolUsage } from "./useAiTools";
import { midnight, MOVING } from "./words";

type CardTab = "overview" | "usage" | "models";

/** The paid AI tool whose key a subscription AI tool's card offers, with its check and page part. */
export type KeyCard = { info: AgentRuntimeInfo; tool: AiToolState };

/**
 * A check that started this long before the sign-in tab's end was seen still counts as the check
 * after it (Plenipo starts it the moment the program ends).
 */
const CHECK_SLACK_MS = 2000;
/** "Checking…" gives up after this long without an answer. */
const CHECK_GIVE_UP_MS = 120_000;

/** Whether a subscription card's key works: saved, checked, and paid keys on (ADR-085). */
function keyWorks(keyCard: KeyCard | undefined): boolean {
  return (
    keyCard !== undefined &&
    keyCard.tool.paidKey !== null &&
    keyCard.info.ready &&
    keyCard.tool.paidBlocked === null
  );
}

/**
 * The card's pill: checking, updating, given no tasks, or ready and why not. A subscription AI
 * tool's card is green when its subscription or its company's key works, and says which (Phase
 * 25, item 1.3).
 */
function cardStatus(
  info: AgentRuntimeInfo,
  tool: AiToolState | undefined,
  checking: boolean,
  keyCard?: KeyCard,
): { status: Status; label: string } {
  if (checking) return { status: "offline", label: "Checking…" };
  if (tool?.outOfService) return { status: "error", label: "No tasks for now" };
  if (tool && MOVING.has(tool.update.state) && tool.update.state !== "waiting") {
    return { status: "pending", label: "Updating…" };
  }
  // Until the page's own part arrives, a card cannot tell a paid AI tool from another.
  if (!tool && !info.ready) return { status: "offline", label: "Loading…" };
  // A paid AI tool is not signed in to: it has a key in use, or not (ADR-085). Paid keys
  // switched off count even before the next check says so.
  if (tool?.payment === "paidKey" && (!info.ready || tool.paidBlocked !== null)) {
    return { status: PILL_TONE.warn, label: tool.paidKey ? "Key not in use" : "No key yet" };
  }
  if (tool?.payment !== "paidKey") {
    const key = keyWorks(keyCard);
    if (info.ready && key) {
      return { status: PILL_TONE.ok, label: "Subscription and API key connected" };
    }
    if (info.ready) return { status: PILL_TONE.ok, label: "Subscription connected" };
    if (key) return { status: PILL_TONE.ok, label: "API key connected" };
  }
  const s = runtimeStatus(info);
  return { status: PILL_TONE[s.tone], label: s.text };
}

/**
 * Whether a card needs you, so it opens by itself (Phase 25, item 2.1): an AI tool that is
 * installed but can't take work (not signed in, a sign-in Plenipo can't use), one given no tasks,
 * or one installed in a way Plenipo can't use. A paid AI tool's key is up to you, so it never does.
 */
function needsYou(info: AgentRuntimeInfo, tool: AiToolState | undefined, checking: boolean) {
  if (checking || !tool || tool.payment === "paidKey") return false;
  const install = info.installation.state;
  if (install === "broken" || install === "unsupported" || tool.outOfService) return true;
  return install === "installed" && !info.ready;
}

/** One line under a card's name (Phase 25, item 2.1): its AI company, plan, and saved key. */
function cardSummary(info: AgentRuntimeInfo, tool: AiToolState | undefined, keyCard?: KeyCard) {
  const signedIn = info.auth.state === "subscription" || info.auth.state === "unverified";
  const plan = tool?.plan?.plan ?? (signedIn ? info.auth.method : null);
  return [
    `AI company: ${info.providerLabel}`,
    plan,
    keyCard?.tool.paidKey ? `Your ${keyCard.info.label} key: ${keyCard.tool.paidKey.name}` : null,
  ]
    .filter((x): x is string => !!x)
    .join(" · ");
}

/**
 * One AI tool's card on the AI tools page (Phase 19): its AI company and state, then Overview
 * (sign-in, version and update, how it is paid for, limits, plan left, this week), Usage, and
 * Models. It starts closed, showing its light, one line, and Sign in or Reconnect (Phase 25, item
 * 2.1); it opens by itself when it needs you or a link points at it, and remembers being opened.
 */
export function AiToolCard({
  info,
  tool,
  route,
  usageRevision,
  onApply,
  onRouting,
  go,
  keyCard,
  focused = false,
}: {
  info: AgentRuntimeInfo;
  tool: AiToolState | undefined;
  route: ToolInfo | undefined;
  usageRevision: number;
  onApply: (page: AiToolsPage) => void;
  onRouting: (snapshot: RoutingSnapshot) => void;
  /** Opens another page (a paid AI tool's Settings links). */
  go?: Go | undefined;
  /** A subscription AI tool's key box: the paid AI tool whose key it saves. */
  keyCard?: KeyCard | undefined;
  /** A link points at this card: it opens. */
  focused?: boolean;
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
  const status = cardStatus(info, tool, checking, keyCard);
  const prefix = `ai-tool-${info.id}`;

  const paid = tool?.payment === "paidKey";
  return (
    <Disclosure
      title={info.label}
      headingId={prefix}
      className="ai-tool"
      status={status}
      summary={cardSummary(info, tool, keyCard)}
      actions={(open) => (open || paid ? null : <QuickSignIn info={info} checking={checking} />)}
      openWhen={focused || needsYou(info, tool, checking)}
      rememberAs={`ai-tool:${info.id}`}
    >
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
            keyCard={keyCard}
            keyWeek={
              keyCard?.tool.paidKey ? (
                <KeyWeekLine keyCard={keyCard} revision={usageRevision} today={midnight(now)} />
              ) : null
            }
          />
        )}
        {tab === "usage" && <UsageTab label={info.label} usage={usage} />}
        {tab === "models" && <ModelsTab info={info} tool={tool} route={route} onApply={onApply} />}
      </div>
    </Disclosure>
  );
}
