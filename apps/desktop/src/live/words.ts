/**
 * The live conversation in plain words (Phase 25, item 3.1): each step a worker takes ("Reading
 * index.ts", "Running `npm test`"), how far along it is ("Step 3 of 7 · 4 min · 12 steps · 3,210
 * tokens"), and what it is doing now, for a tile's one line.
 */
import type { AgentActivity, PlanStep, PositionInfo, TaskState } from "@plenipo/types";

/** Work a live conversation can show: started, and not finished. */
const LIVE: ReadonlySet<TaskState> = new Set(["running", "blocked", "awaitingApproval"]);

/** A position's work going on now, with its conversation (up to 3: a page shows each). */
export function liveWork(p: PositionInfo): {
  taskId: string;
  sessionId: string;
  objective: string;
  startedAt: number | null;
  state: TaskState;
}[] {
  if (p.staffing === "persistent") {
    const t = p.currentTask;
    const sessionId = t?.sessionId ?? p.agent?.sessionId ?? null;
    return t && sessionId && LIVE.has(t.state)
      ? [
          {
            taskId: t.id,
            sessionId,
            objective: t.objective,
            startedAt: t.startedAt,
            state: t.state,
          },
        ]
      : [];
  }
  return p.workers
    .flatMap((w) =>
      w.sessionId && LIVE.has(w.state)
        ? [
            {
              taskId: w.taskId,
              sessionId: w.sessionId,
              objective: w.objective,
              startedAt: w.startedAt,
              state: w.state,
            },
          ]
        : [],
    )
    .slice(0, 3);
}

/** A file's name from a path ("src/app/index.ts" → "index.ts"); anything else as it is. */
function nameOf(summary: string): string {
  const s = summary.trim();
  if (!s || /\s/.test(s)) return s;
  const parts = s.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? s;
}

/** The tool's own name, without an add-on's prefix ("plenipo/run_command" → "run_command"). */
function toolName(tool: string): string {
  const t = tool.toLowerCase().trim();
  const slash = t.lastIndexOf("/");
  const named = slash >= 0 ? t.slice(slash + 1) : t;
  return named.replace(/^mcp__[a-z0-9_-]+__/, "");
}

const READ = new Set(["read", "read_file", "view", "open_file", "cat", "notebookread"]);
const WRITE = new Set(["write", "write_file", "create_file", "create"]);
const CHANGE = new Set([
  "edit",
  "edit_file",
  "multiedit",
  "str_replace",
  "str_replace_editor",
  "apply_patch",
  "file change",
  "notebookedit",
]);
const RUN = new Set([
  "bash",
  "shell",
  "run_command",
  "exec",
  "execute",
  "command",
  "command_execution",
  "terminal",
  "run_terminal_cmd",
]);
const SEARCH = new Set(["grep", "search", "search_files", "rg", "find"]);
const FIND = new Set(["glob", "ls", "list", "list_dir", "list_files"]);
const OPEN = new Set(["webfetch", "fetch", "browse", "open_page", "navigate", "open_url"]);
const WEB = new Set(["websearch", "web search", "web_search", "search_web"]);
const HELPER = new Set(["task", "agent", "subagent"]);

/** One step in plain words. */
export function stepWords(tool: string, summary: string): string {
  const t = toolName(tool);
  const s = summary.trim();
  const name = nameOf(s);
  if (READ.has(t)) return name ? `Reading ${name}` : "Reading a file";
  if (WRITE.has(t)) return name ? `Writing ${name}` : "Writing a file";
  if (CHANGE.has(t)) return name ? `Changing ${name}` : "Changing a file";
  if (RUN.has(t)) return s ? `Running \`${s}\`` : "Running a command";
  if (SEARCH.has(t)) return s ? `Searching for ${s}` : "Searching the files";
  if (FIND.has(t)) return s ? `Looking at ${s}` : "Looking at the files";
  if (OPEN.has(t)) return s ? `Opening ${s}` : "Opening a web page";
  if (WEB.has(t)) return s ? `Searching the web for ${s}` : "Searching the web";
  if (HELPER.has(t)) return s ? `Asking a helper: ${s}` : "Asking a helper";
  return s ? `${tool}: ${s}` : `Using ${tool}`;
}

/** How far along a worker is, from its activity. */
export interface LiveProgress {
  /** Steps of its plan done, and in all (`null`: no plan). */
  done: number | null;
  total: number | null;
  /** The plan's step it is on. */
  current: string | null;
  /** Steps taken (tools used). */
  steps: number;
  /** Tokens used so far, when its AI tool says. */
  tokens: number | null;
}

/** The latest plan in the activity. */
export function latestPlan(activity: readonly AgentActivity[]): PlanStep[] | null {
  for (let i = activity.length - 1; i >= 0; i--) {
    const e = activity[i]?.event;
    if (e?.type === "plan") return e.steps;
  }
  return null;
}

export function liveProgress(activity: readonly AgentActivity[]): LiveProgress {
  const plan = latestPlan(activity);
  let steps = 0;
  let tokens: number | null = null;
  for (const a of activity) {
    if (a.event.type === "toolUse") steps++;
    if (a.event.type === "usage") tokens = a.event.usage.inputTokens + a.event.usage.outputTokens;
  }
  const current =
    plan?.find((s) => s.status === "inProgress")?.text ??
    plan?.find((s) => s.status === "pending")?.text ??
    null;
  return {
    done: plan ? plan.filter((s) => s.status === "done").length : null,
    total: plan ? plan.length : null,
    current,
    steps,
    tokens,
  };
}

/** "Step 3 of 7 · 4 min · 12 steps · 3,210 tokens", leaving out what isn't known. */
export function progressWords(p: LiveProgress, startedAt: number | null, now: number): string {
  const parts: string[] = [];
  if (p.total !== null && p.done !== null && p.total > 0) {
    parts.push(
      p.done >= p.total ? `All ${p.total} steps done` : `Step ${p.done + 1} of ${p.total}`,
    );
  }
  if (startedAt !== null) {
    const minutes = Math.floor(Math.max(0, now - startedAt) / 60_000);
    parts.push(minutes < 1 ? "under a minute" : `${minutes} min`);
  }
  if (p.steps > 0) parts.push(`${p.steps} ${p.steps === 1 ? "step" : "steps"}`);
  if (p.tokens !== null && p.tokens > 0) parts.push(`${p.tokens.toLocaleString("en-US")} tokens`);
  return parts.join(" · ");
}

/**
 * A step Claude Code has just started and not yet finished writing ("Running a command"), when
 * it is the latest thing it did; `null` once the step is complete or anything came after it.
 */
export function startingStep(activity: readonly AgentActivity[]): string | null {
  for (let i = activity.length - 1; i >= 0; i--) {
    const e = activity[i]?.event;
    if (!e || e.type === "usage") continue;
    if (e.type === "status" && e.phase === "starting") return e.text;
    if (e.type === "status") continue;
    return null;
  }
  return null;
}

/** What it is doing now, in a few words, for a tile: the last step, or writing its answer. */
export function nowWords(activity: readonly AgentActivity[]): string | null {
  for (let i = activity.length - 1; i >= 0; i--) {
    const e = activity[i]?.event;
    if (!e) continue;
    switch (e.type) {
      case "toolUse":
        return stepWords(e.tool, e.summary);
      case "textDelta":
      case "message":
        return "Writing its answer";
      case "reasoning":
        return "Thinking";
      case "status":
        // A step starting, or a wait in its own words: for the AI company, or no word from the
        // AI tool for a long time (the owner's report, 2026-10-05: not "Writing" forever).
        if (e.phase === "starting" || e.phase === "waiting") return e.text;
        break;
      case "plan": {
        const p = liveProgress(activity);
        if (p.current) return p.current;
        break;
      }
      default:
        break;
    }
  }
  return null;
}

/**
 * What a tile's line says for work in `state` now: its wait, while it waits, or what it is doing.
 * A lead waiting for its team no longer says "Writing its answer" from its last words (the owner's
 * report, 2026-10-05).
 */
export function nowFor(state: TaskState, activity: readonly AgentActivity[]): string | null {
  if (state === "blocked") return "Waiting for its team";
  if (state === "awaitingApproval") return "Waiting for your approval";
  return nowWords(activity);
}
