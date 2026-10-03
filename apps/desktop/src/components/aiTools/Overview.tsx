import type { ReactNode } from "react";
import type {
  AgentRuntimeInfo,
  AiToolsPage,
  AiToolState,
  AiToolUsage,
  InstallState,
  PlanWindow,
  RoutingSnapshot,
  ToolInfo,
} from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { INSTALL_LABEL } from "../../agents/format";
import { cancelAiToolUpdate, checkAiTool, clearUsageLimit, updateAiTool } from "../../api/commands";
import { useRun } from "../../guard/useRun";
import { when } from "../../pages/words";
import { until } from "../../routing/format";
import { useTerminalIfAny } from "../../terminal/useTerminal";
import { Refusal } from "../models/shared";
import type { Go } from "../views";
import type { KeyCard } from "./AiToolCard";
import { movesToKey, OPENROUTER } from "./keyFor";
import { PaidKey } from "./PaidKey";
import { SignIn } from "./SignIn";
import { useToolUsage } from "./useAiTools";
import {
  atLimit,
  countsNothing,
  isNewerVersion,
  keyLimitWords,
  MOVING,
  planUsed,
  planWindowName,
  resetWhen,
  usageBetween,
  usageLine,
  usingWords,
  versionNotice,
  type UsageWindow,
} from "./words";
import { systemWords } from "../../system/words";

type Apply = (page: AiToolsPage) => void;

/**
 * Claude Code reports what is left of your plan in the messages Plenipo reads during a task; the
 * others that report it (Codex) do when Plenipo checks them (ADR-060 §3).
 */
const PLAN_DURING_A_TASK: ReadonlySet<string> = new Set(["claude-code"]);

/**
 * A card's Overview (Phase 19): sign-in, version and update, how it is paid for, the usage limit,
 * what is left of the plan, and this week's tokens. A paid AI tool (Phase 16 Wave 3, ADR-085)
 * shows its key instead of a plan; a subscription AI tool shows a key box for paying per use
 * instead, saving the key of the paid AI tool in `keyCard` (2026-09-30).
 */
export function Overview({
  info,
  tool,
  route,
  checking,
  usage,
  onApply,
  onRouting,
  go,
  keyCard,
  keyWeek,
}: {
  info: AgentRuntimeInfo;
  /** The page's part for this tool; `undefined` while it loads. */
  tool: AiToolState | undefined;
  /** The Router's view of the tool (its usage limit); `undefined` while it loads. */
  route: ToolInfo | undefined;
  checking: boolean;
  usage: { usage: AiToolUsage | null; window: UsageWindow | null; error: string | null };
  onApply: Apply;
  onRouting: (snapshot: RoutingSnapshot) => void;
  /** Opens another page (Settings → Switches or Spending caps). */
  go?: Go | undefined;
  /** A subscription AI tool's key box: the paid AI tool whose key it saves. */
  keyCard?: KeyCard | undefined;
  /** This week's work with that key, as a second line (Phase 25, item 2.1). */
  keyWeek?: ReactNode;
}) {
  const install = info.installation.state;
  const paid = tool?.payment === "paidKey";
  const installed = install === "installed" ? (info.installation.version ?? "unknown") : null;
  // Not working, or installed in a way Plenipo can't use: Plenipo's reason, where it gave one.
  const problem =
    install === "broken" || install === "unsupported" ? info.installation.detail : null;
  const notice = versionNotice(info.installation.version, info.checkedVersion);
  // The subscription's name, as the tool's sign-in check says it ("Claude subscription (max)").
  const plan =
    info.auth.state === "subscription" || info.auth.state === "unverified"
      ? info.auth.method
      : null;
  return (
    <dl className="kv ai-tool__facts">
      <dt>{paid ? "Key check" : "Sign-in"}</dt>
      <dd>
        {!tool ? (
          <span className="muted">Loading…</span>
        ) : paid ? (
          <KeyCheck info={info} tool={tool} checking={checking} />
        ) : (
          <SignIn info={info} checking={checking} />
        )}
      </dd>
      <dt>Version</dt>
      <dd>
        {!tool ? (
          <span className="muted">Loading…</span>
        ) : tool.builtIn ? (
          <div>Comes with Plenipo {info.checkedVersion}</div>
        ) : (
          <div>
            {installed ? `Installed ${installed}` : INSTALL_LABEL[install]} · Checked by Plenipo{" "}
            {info.checkedVersion}
          </div>
        )}
        {problem && <p className="muted">{problem}</p>}
        {notice && <p className="muted">{notice}</p>}
      </dd>
      <dt>Update</dt>
      <dd>
        {tool?.builtIn ? (
          <>{info.label} comes with Plenipo: it is updated when Plenipo is.</>
        ) : (
          <UpdateInfo info={info} tool={tool} onApply={onApply} />
        )}
      </dd>
      <dt>How it is paid for</dt>
      <dd>
        {!tool ? (
          <span className="muted">Loading…</span>
        ) : paid ? (
          // A new instance for each saved key: nothing typed for the last one stays.
          <PaidKey
            key={tool.paidKey?.id ?? "no key"}
            info={info}
            tool={tool}
            onApply={onApply}
            go={go}
          />
        ) : (
          <>
            <div>
              Subscription
              {plan ? ` (${plan})` : ""}
            </div>
            {/* A subscription AI tool never takes a paid key itself (ADR-085 §5). */}
            <p className="muted">{info.label} always uses your subscription.</p>
            {keyCard && (
              <PayPerUse label={info.label} keyCard={keyCard} onApply={onApply} go={go} />
            )}
          </>
        )}
      </dd>
      <dt>Usage limit</dt>
      <dd>
        <UsageLimit route={route} onRouting={onRouting} keyCard={keyCard} />
      </dd>
      <dt>Left of your plan</dt>
      <dd>
        {paid ? (
          <div className="ai-tool__block">
            <p>
              No plan: {info.label} is paid per use. A spending limit is up to you, in Spending
              caps.
            </p>
            {/* The key's own limit, where the service reports it (Phase 25, item 4.3). */}
            {tool?.plan?.keyLimit && <p>{keyLimitWords(tool.plan.keyLimit)}</p>}
            {go && (
              <div className="ai-tool__buttons">
                <Button
                  size="sm"
                  variant="quiet"
                  onClick={() => go({ view: "settings", id: "spending" })}
                >
                  Spending caps
                </Button>
              </div>
            )}
          </div>
        ) : (
          <PlanLeft info={info} tool={tool} onApply={onApply} />
        )}
      </dd>
      <dt>This week</dt>
      <dd>
        <WeekLine label={info.label} usage={usage} />
        {keyWeek}
      </dd>
    </dl>
  );
}

/**
 * A subscription AI tool's key box (2026-09-30): paying per use with your own key instead. The key
 * is the paid AI tool's (the same AI company's, or OpenRouter's), the same one as on that tool's
 * card; work on it runs on that paid AI tool, priced and listed under Spending caps. The
 * subscription AI tool itself never gets the key.
 */
function PayPerUse({
  label,
  keyCard,
  onApply,
  go,
}: {
  label: string;
  keyCard: KeyCard;
  onApply: Apply;
  go?: Go | undefined;
}) {
  const company = keyCard.info.label;
  return (
    <div className="ai-tool__block pay-per-use" role="group" aria-label="Pay per use instead">
      <p>
        <strong>Or pay per use with your own key.</strong>{" "}
        {keyCard.info.id === OPENROUTER
          ? `${label} has no key of its own for paying per use; an OpenRouter key reaches the same kinds of models, and many more.`
          : `Your ${company} key goes here.`}{" "}
        {keyCard.info.id === OPENROUTER
          ? "It is the same key as on the OpenRouter card under Paid per use with your key. "
          : ""}
        Work on it runs on Plenipo&apos;s {company} AI tool and is priced and listed under Spending
        caps; {label} itself keeps using your subscription.
      </p>
      {keyCard.tool.paidKey && (
        <KeyCheck
          info={keyCard.info}
          tool={keyCard.tool}
          checking={keyCard.tool.checking || keyCard.info.auth.state === "checking"}
        />
      )}
      {/* A new instance for each saved key, as on the paid AI tool's own card. */}
      <PaidKey
        key={keyCard.tool.paidKey?.id ?? "no key"}
        info={keyCard.info}
        tool={keyCard.tool}
        onApply={onApply}
        go={go}
        place={`${label}'s card`}
      />
    </div>
  );
}

/**
 * A paid AI tool's check (ADR-085): whether its key works, or why it is not in use. Nothing to
 * sign in to: the key is added below.
 */
function KeyCheck({
  info,
  tool,
  checking,
}: {
  info: AgentRuntimeInfo;
  tool: AiToolState;
  checking: boolean;
}) {
  if (checking) return <>Checking…</>;
  // Switched off since the last check: not in use, whatever it said.
  if (info.ready && tool.paidBlocked === null) {
    return <>Your key works{info.checkedAt ? ` (checked at ${when(info.checkedAt)})` : ""}.</>;
  }
  // Switched off: the notice below says so once.
  const why = tool.paidBlocked ? null : info.auth.detail;
  return (
    <div className="ai-tool__block">
      <div>{tool.paidKey ? "Key not in use" : "No key yet"}</div>
      {why && <p className="muted ai-tool__why">{why}</p>}
    </div>
  );
}

/** Where the tool's update is, its newest version, and Update / Cancel (ADR-059). */
function UpdateInfo({
  info,
  tool,
  onApply,
}: {
  info: AgentRuntimeInfo;
  tool: AiToolState | undefined;
  onApply: Apply;
}) {
  const terminal = useTerminalIfAny();
  const { pending, error, run } = useRun<AiToolsPage>(onApply);
  if (!tool) return <span className="muted">Loading…</span>;
  const label = info.label;
  const u = tool.update;
  const installed = info.installation.state === "installed" ? info.installation.version : null;
  const moving = MOVING.has(u.state);
  const newer =
    tool.newest !== null && installed !== null && isNewerVersion(tool.newest, installed);
  const looked = tool.newestCheckedAt ? ` (looked at ${when(tool.newestCheckedAt)})` : "";
  const update = (to: string | null) => (
    <Button
      size="sm"
      variant="primary"
      disabled={pending || installed === null}
      aria-label={to ? `Update ${label} to ${to}` : `Update ${label}`}
      onClick={() => void run(() => updateAiTool(info.id))}
    >
      {to ? `Update to ${to}` : "Update"}
    </Button>
  );

  let state: ReactNode = null;
  switch (u.state) {
    case "waiting":
      state = (
        <p role="status">
          Waiting:{" "}
          {u.tasksUsing > 0
            ? `${usingWords(u.tasksUsing, label)}. Plenipo updates it when ${
                u.tasksUsing === 1 ? "it finishes" : "they finish"
              }.`
            : `Plenipo updates ${label} when it is free.`}{" "}
          <Button
            size="sm"
            variant="quiet"
            disabled={pending}
            aria-label={`Cancel ${label}'s update`}
            onClick={() => void run(() => cancelAiToolUpdate(info.id))}
          >
            Cancel
          </Button>
        </p>
      );
      break;
    case "updating":
      state = <p role="status">Updating…</p>;
      break;
    case "checking":
      state = <p role="status">Checking the new version…</p>;
      break;
    case "updated":
      state = (
        <p>
          Updated to {u.to ?? "a new version"}
          {u.automatic ? " by itself" : ""}
          {u.at ? ` at ${when(u.at)}` : ""}.
        </p>
      );
      break;
    case "upToDate":
      state = <p>Up to date.</p>;
      break;
    case "byHand":
      state = (
        <div>
          <p>{u.message ?? tool.updateByHand}</p>
          {terminal && (
            <Button size="sm" icon="terminal" onClick={terminal.openHere}>
              Open a terminal
            </Button>
          )}
          <p className="muted">You type the command there; Plenipo never types it for you.</p>
        </div>
      );
      break;
    case "failed":
      state =
        u.oldStillWorks === false ? null : (
          <div>
            <p>
              The update didn&apos;t finish — your old version ({u.from ?? installed ?? "unknown"})
              still works.
            </p>
            {u.message && <p className="muted">{u.message}</p>}
          </div>
        );
      break;
    case "idle":
      break;
  }

  const stopped =
    tool.outOfService ?? (u.state === "failed" && u.oldStillWorks === false ? u.message : null);
  let newest: ReactNode = null;
  if (!moving) {
    if (installed === null) {
      newest = <p className="muted">{nothingToUpdate(label, info.installation.state)}</p>;
    } else if (!tool.canUpdate) {
      newest = (
        <>
          {tool.newest && (
            <p>
              {newer ? `Newest version: ${tool.newest}` : "Up to date"}
              {looked}.
            </p>
          )}
          <p className="muted">
            {label} updates itself: when a new version is ready, use {label}&apos;s icon in{" "}
            {systemWords().waitsIn} to restart it.
          </p>
        </>
      );
    } else if (tool.newestFrom === "updateChecks") {
      newest = (
        <>
          <p className="muted">
            {label}&apos;s newest version is not published in a list Plenipo can read. Press Update:{" "}
            {label} checks and installs a new version if there is one.
          </p>
          {update(null)}
        </>
      );
    } else if (tool.newest === null) {
      newest = (
        <p className="muted">
          {tool.newestProblem
            ? `Plenipo couldn't find ${label}'s newest version: ${tool.newestProblem}`
            : "Plenipo hasn't looked for a new version yet."}
        </p>
      );
    } else {
      newest = (
        <>
          {newer ? (
            <p>
              Newest version: {tool.newest}
              {looked}.
            </p>
          ) : (
            u.state !== "upToDate" && u.state !== "updated" && <p>Up to date{looked}.</p>
          )}
          {tool.newestProblem && (
            <p className="muted">The last look didn&apos;t work: {tool.newestProblem}</p>
          )}
          {newer && update(tool.newest)}
        </>
      );
    }
  }

  return (
    <div className="ai-tool__block">
      {state}
      {stopped && (
        <div role="status">
          <p>Plenipo is not giving {label} tasks for now.</p>
          <p className="muted">{stopped}</p>
          <Button
            size="sm"
            disabled={pending}
            aria-label={`Check ${label} again`}
            onClick={() => void run(() => checkAiTool(info.id))}
          >
            Check again
          </Button>
        </div>
      )}
      {newest}
      <Refusal error={error} />
    </div>
  );
}

/** Why there is no version to update: still checking, not installed, or not working. */
function nothingToUpdate(label: string, install: InstallState): string {
  switch (install) {
    case "checking":
      return "Checking…";
    case "notInstalled":
      return `Nothing to update: ${label} is not installed.`;
    case "installed":
      return `Plenipo can't tell which version of ${label} is installed.`;
    case "unsupported":
    case "broken":
      return `Plenipo can't update ${label} now (${INSTALL_LABEL[install].toLowerCase()}).`;
  }
}

/**
 * The usage limit and when it resets, from the Router, with Try again now (ADR-060 §2), and
 * where its work goes meanwhile: the same models on its company's key, when the key can take them
 * (Phase 25, item 4.4; ADR-254).
 */
function UsageLimit({
  route,
  onRouting,
  keyCard,
}: {
  route: ToolInfo | undefined;
  onRouting: (snapshot: RoutingSnapshot) => void;
  keyCard?: KeyCard | undefined;
}) {
  const { pending, error, run } = useRun<RoutingSnapshot>(onRouting);
  if (!route) return <span className="muted">Loading…</span>;
  const limit = route.usageLimit;
  if (!limit) return <>No usage limit reached</>;
  const onKey =
    movesToKey(route.runtimeId) &&
    keyCard !== undefined &&
    keyCard.tool.paidKey !== null &&
    keyCard.info.ready &&
    keyCard.tool.paidBlocked === null;
  return (
    <div className="ai-tool__block">
      <div>
        Usage limit reached —{" "}
        {limit.resetsAt
          ? `resets at ${when(limit.until)} (${until(limit.until)})`
          : `Plenipo tries it again at ${when(limit.until)} (${until(limit.until)})`}
      </div>
      {onKey && keyCard && (
        <div>
          Its work moves to your {keyCard.info.label} key while it waits (paid per use, within your
          spending caps).
        </div>
      )}
      <div className="ai-tool__buttons">
        <Button
          size="sm"
          variant="quiet"
          disabled={pending}
          aria-label={`Try ${route.label} again now`}
          onClick={() => void run(() => clearUsageLimit(route.runtimeId))}
        >
          Try again now
        </Button>
      </div>
      <Refusal error={error} />
    </div>
  );
}

const NO_WINDOW: PlanWindow = { minutes: null, usedPercent: null, resetsAt: null };

/**
 * How much of the plan is left, only as the tool officially reported it (ADR-060 §3). "Limit
 * reached" is said once: on the window at its limit, or above them all when the tool said only
 * that the plan is limited.
 */
function PlanLeft({
  info,
  tool,
  onApply,
}: {
  info: AgentRuntimeInfo;
  tool: AiToolState | undefined;
  onApply: Apply;
}) {
  const label = info.label;
  const { pending, error, run } = useRun<AiToolsPage>(onApply);
  if (!tool) return <span className="muted">Loading…</span>;
  if (!tool.reportsPlanLeft) return <>{label} doesn&apos;t report how much of your plan is left.</>;
  // Asked now, for an AI tool Plenipo can ask (Phase 25, item 1.2); Claude Code tells it only
  // during a task.
  const ask = PLAN_DURING_A_TASK.has(info.id) ? null : (
    <div className="ai-tool__buttons">
      <Button
        size="sm"
        variant="quiet"
        disabled={pending || !info.ready}
        aria-label={`Check ${label}'s plan now`}
        onClick={() => void run(() => checkAiTool(info.id))}
      >
        {pending ? "Checking…" : "Check plan"}
      </Button>
      <Refusal error={error} />
    </div>
  );
  const plan = tool.plan;
  if (!plan) {
    return PLAN_DURING_A_TASK.has(info.id) ? (
      <>{label} reports this during a task; nothing reported yet.</>
    ) : (
      <div className="ai-tool__block">
        <span>
          {label} hasn&apos;t reported it yet. Plenipo asks when it checks {label}.
        </span>
        {ask}
      </div>
    );
  }
  const windows = plan.windows.length > 0 ? plan.windows : [NO_WINDOW];
  const limitedAbove = plan.limited && !windows.some((w) => atLimit(w, plan));
  return (
    <div className="ai-tool__block">
      {limitedAbove && <div>Limit reached</div>}
      <ul className="ai-tool__list" aria-label={`Left of your plan with ${label}`}>
        {windows.map((w, i) => {
          const name = planWindowName(w.minutes, w.models);
          return (
            <li key={`${w.minutes ?? "plan"}-${w.models ?? ""}-${i}`}>
              {name && <strong>{name}: </strong>}
              {planUsed(w, plan)}
              {w.resetsAt ? `, resets ${resetWhen(w.resetsAt)}` : ""}
            </li>
          );
        })}
      </ul>
      <span className="muted">
        Reported by {label} at {when(plan.reportedAt)}
        {plan.plan ? ` · your plan: ${plan.plan}` : ""}
      </span>
      {ask}
    </div>
  );
}

/**
 * This week's work with a subscription AI tool's key, as a second line under its own (Phase 25,
 * item 2.1): read only while the card shows it.
 */
export function KeyWeekLine({
  keyCard,
  revision,
  today,
}: {
  keyCard: KeyCard;
  revision: number;
  today: number;
}) {
  const usage = useToolUsage(keyCard.info.id, revision, today);
  return (
    <div>
      <span className="muted">With your key: </span>
      <WeekLine label={keyCard.info.label} usage={usage} />
    </div>
  );
}

/** This week's tokens in one line. */
function WeekLine({
  label,
  usage,
}: {
  label: string;
  usage: { usage: AiToolUsage | null; window: UsageWindow | null; error: string | null };
}) {
  if (!usage.usage || !usage.window) {
    return <span className="muted">{usage.error ?? "Loading…"}</span>;
  }
  const { total } = usageBetween(usage.usage, usage.window.thisWeek, usage.window.tomorrow);
  if (total.tasks === 0) return <>No tasks this week yet</>;
  const counts = !countsNothing(usage.usage);
  return (
    <>
      {usageLine(total, counts)}
      {!counts && <span className="muted"> ({label} doesn&apos;t report token counts)</span>}
    </>
  );
}
