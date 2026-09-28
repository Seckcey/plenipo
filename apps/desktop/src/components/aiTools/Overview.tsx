import type { ReactNode } from "react";
import type {
  AgentRuntimeInfo,
  AiToolsPage,
  AiToolState,
  AiToolUsage,
  PlanWindow,
  RoutingSnapshot,
  ToolInfo,
} from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { cancelAiToolUpdate, checkAiTool, clearUsageLimit, updateAiTool } from "../../api/commands";
import { useRun } from "../../guard/useRun";
import { when } from "../../pages/words";
import { until } from "../../routing/format";
import { useTerminalIfAny } from "../../terminal/useTerminal";
import { Refusal } from "../models/shared";
import { Toggle } from "../SwitchSettings";
import { SignIn } from "./SignIn";
import {
  compareVersions,
  countsNothing,
  MOVING,
  planLeft,
  planWindowName,
  usageBetween,
  usageLine,
  usingWords,
  versionNotice,
  type UsageWindow,
} from "./words";

type Apply = (page: AiToolsPage) => void;

/**
 * A card's Overview (Phase 19): sign-in, version and update, how it is paid for, the usage limit,
 * what is left of the plan, and this week's tokens.
 */
export function Overview({
  info,
  tool,
  route,
  checking,
  usage,
  onApply,
  onRouting,
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
}) {
  const installed =
    info.installation.state === "installed" ? (info.installation.version ?? "unknown") : null;
  const notice = versionNotice(info.installation.version, info.checkedVersion);
  // The subscription's name, as the tool's sign-in check says it ("Claude subscription (max)").
  const plan =
    info.auth.state === "subscription" || info.auth.state === "unverified"
      ? info.auth.method
      : null;
  return (
    <dl className="kv ai-tool__facts">
      <dt>Sign-in</dt>
      <dd>
        <SignIn info={info} checking={checking} />
      </dd>
      <dt>Version</dt>
      <dd>
        <div>
          {installed ? `Installed ${installed}` : "Not installed"} · Checked by Plenipo{" "}
          {info.checkedVersion}
        </div>
        {notice && <p className="muted">{notice}</p>}
      </dd>
      <dt>Update</dt>
      <dd>
        <UpdateInfo info={info} tool={tool} onApply={onApply} />
      </dd>
      <dt>How it is paid for</dt>
      <dd>
        <div>
          {tool?.payment === "paidKey" ? "Paid AI key" : "Subscription"}
          {plan ? ` (${plan})` : ""}
        </div>
        {/* Locked until spending caps exist (Phase 16): it never asks for a paid key. */}
        <Toggle
          label="Paid AI key (pay per use)"
          hint="Comes with spending caps in a later version."
          checked={tool?.payment === "paidKey"}
          disabled
          onChange={() => undefined}
        />
      </dd>
      <dt>Usage limit</dt>
      <dd>
        <UsageLimit route={route} onRouting={onRouting} />
      </dd>
      <dt>Left of your plan</dt>
      <dd>
        <PlanLeft label={info.label} tool={tool} />
      </dd>
      <dt>This week</dt>
      <dd>
        <WeekLine label={info.label} usage={usage} />
      </dd>
    </dl>
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
    tool.newest !== null && installed !== null && compareVersions(tool.newest, installed) === 1;
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
      newest = <p className="muted">Nothing to update: {label} is not installed.</p>;
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
            {label} updates itself: when a new version is ready, use {label}&apos;s icon in the tray
            to restart it.
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

/** The usage limit and when it resets, from the Router, with Try again now (ADR-060 §2). */
function UsageLimit({
  route,
  onRouting,
}: {
  route: ToolInfo | undefined;
  onRouting: (snapshot: RoutingSnapshot) => void;
}) {
  const { pending, error, run } = useRun<RoutingSnapshot>(onRouting);
  if (!route) return <span className="muted">Loading…</span>;
  const limit = route.usageLimit;
  if (!limit) return <>No usage limit reached</>;
  return (
    <div className="ai-tool__block">
      <div>
        Usage limit reached —{" "}
        {limit.resetsAt
          ? `resets at ${when(limit.until)} (${until(limit.until)})`
          : `Plenipo tries it again at ${when(limit.until)} (${until(limit.until)})`}
      </div>
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

/** How much of the plan is left, only as the tool officially reported it (ADR-060 §3). */
function PlanLeft({ label, tool }: { label: string; tool: AiToolState | undefined }) {
  if (!tool) return <span className="muted">Loading…</span>;
  if (!tool.reportsPlanLeft) return <>{label} doesn&apos;t report how much of your plan is left.</>;
  const plan = tool.plan;
  if (!plan) return <>{label} reports this during a task; nothing reported yet.</>;
  const windows = plan.windows.length > 0 ? plan.windows : [NO_WINDOW];
  return (
    <div className="ai-tool__block">
      <ul className="ai-tool__list" aria-label={`Left of your plan with ${label}`}>
        {windows.map((w, i) => (
          <li key={`${w.minutes ?? "plan"}-${i}`}>
            <strong>{planWindowName(w.minutes)}:</strong> {planLeft(w, plan)}
            {w.resetsAt ? ` · resets at ${when(w.resetsAt)}` : ""}
          </li>
        ))}
      </ul>
      <span className="muted">
        Reported by {label} at {when(plan.reportedAt)}
        {plan.plan ? ` · your plan: ${plan.plan}` : ""}
      </span>
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
