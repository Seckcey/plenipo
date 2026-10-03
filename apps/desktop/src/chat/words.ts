/**
 * The plain words a conversation uses for what an agent does (ADR-010, ADR-200): its tools in
 * everyday words, a run of tool calls in one sentence, and how long something has taken.
 */

/** What kind of thing a tool does, for the sentence that sums up a run of them. */
export type ToolKind =
  | "read"
  | "list"
  | "search"
  | "write"
  | "edit"
  | "move"
  | "delete"
  | "run"
  | "script"
  | "git"
  | "github"
  | "web"
  | "screen"
  | "server"
  | "other";

const KINDS: Record<string, ToolKind> = {
  read_file: "read",
  list_directory: "list",
  search_text: "search",
  write_file: "write",
  edit_file: "edit",
  move_path: "move",
  delete_path: "delete",
  run_command: "run",
  run_powershell: "script",
  git_status: "git",
  git_diff: "git",
  git_log: "git",
  git_add: "git",
  git_commit: "git",
  git_branch: "git",
  git_push: "git",
  github_pr_list: "github",
  github_pr_view: "github",
  github_pr_checks: "github",
  github_issue_view: "github",
  github_pr_create: "github",
};

/** Claude Code names Plenipo's tools `mcp__plenipo__write_file`; the others use the bare name. */
export function toolName(raw: string): string {
  const parts = raw.split("__");
  return parts.length >= 3 && parts[0] === "mcp" ? parts.slice(2).join("__") : raw;
}

export function toolKind(raw: string): ToolKind {
  const name = toolName(raw);
  const known = KINDS[name];
  if (known) return known;
  if (name.startsWith("browser_")) return "web";
  if (name.startsWith("screen_")) return "screen";
  if (name.startsWith("ssh_")) return "server";
  return "other";
}

/** A tool as a short phrase for its own row: "Saving a file", "Running a program". */
export function toolPhrase(raw: string, running: boolean): string {
  const kind = toolKind(raw);
  const now = running;
  switch (kind) {
    case "read":
      return now ? "Reading a file" : "Read a file";
    case "list":
      return now ? "Looking at a folder" : "Looked at a folder";
    case "search":
      return now ? "Searching the files" : "Searched the files";
    case "write":
      return now ? "Saving a file" : "Saved a file";
    case "edit":
      return now ? "Changing a file" : "Changed a file";
    case "move":
      return now ? "Moving a file" : "Moved a file";
    case "delete":
      return now ? "Deleting a file" : "Deleted a file";
    case "run":
      return now ? "Running a program" : "Ran a program";
    case "script":
      return now ? "Running a script" : "Ran a script";
    case "git":
      return now ? "Using git" : "Used git";
    case "github":
      return now ? "Checking GitHub" : "Checked GitHub";
    case "web":
      return now ? "Using a web page" : "Used a web page";
    case "screen":
      return now ? "Using the screen" : "Used the screen";
    case "server":
      return now ? "Using a server" : "Used a server";
    case "other":
      return now ? `Using ${toolName(raw)}` : `Used ${toolName(raw)}`;
  }
}

/** A file name from a tool's one-line summary, when the summary is just a path. */
export function fileOf(summary: string): string | null {
  const text = summary.trim();
  if (text === "" || text.length > 240 || /\s{2,}/.test(text)) return null;
  // A path has no spaces around a program name and its arguments: take what looks like a path.
  const first = text.split(/\s+/)[0] ?? "";
  if (!/[\\/]|\.[A-Za-z0-9]{1,8}$/.test(first)) return null;
  return first;
}

/** The last part of a path, for a short label. */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

interface Call {
  tool: string;
  summary: string;
}

/**
 * One sentence for a run of tool calls, in the order the kinds first appear: "Read 2 files,
 * searched the files, saved clear-temp.ps1". One file by name; more than one by count.
 */
export function describeCalls(calls: readonly Call[], past: boolean): string {
  const order: ToolKind[] = [];
  const groups = new Map<ToolKind, Call[]>();
  for (const call of calls) {
    const kind = toolKind(call.tool);
    const group = groups.get(kind);
    if (group) group.push(call);
    else {
      groups.set(kind, [call]);
      order.push(kind);
    }
  }
  const phrases = order.map((kind) => phrase(kind, groups.get(kind) ?? [], past));
  const text = phrases.join(", ");
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function phrase(kind: ToolKind, calls: readonly Call[], past: boolean): string {
  const n = calls.length;
  const file = n === 1 ? fileOf(calls[0]?.summary ?? "") : null;
  const name = file ? baseName(file) : null;
  const many = (word: string, plural: string) => (n === 1 ? word : `${n} ${plural}`);
  switch (kind) {
    case "read":
      return `${past ? "read" : "reading"} ${name ?? many("a file", "files")}`;
    case "list":
      return `${past ? "looked at" : "looking at"} ${many("a folder", "folders")}`;
    case "search":
      return n === 1
        ? past
          ? "searched the files"
          : "searching the files"
        : `${past ? "searched" : "searching"} the files ${n} times`;
    case "write":
      return `${past ? "saved" : "saving"} ${name ?? many("a file", "files")}`;
    case "edit":
      return `${past ? "changed" : "changing"} ${name ?? many("a file", "files")}`;
    case "move":
      return `${past ? "moved" : "moving"} ${many("a file", "files")}`;
    case "delete":
      return `${past ? "deleted" : "deleting"} ${name ?? many("a file", "files")}`;
    case "run":
      return `${past ? "ran" : "running"} ${many("a program", "programs")}`;
    case "script":
      return `${past ? "ran" : "running"} ${many("a script", "scripts")}`;
    case "git":
      return `${past ? "used" : "using"} git${n === 1 ? "" : ` ${n} times`}`;
    case "github":
      return `${past ? "checked" : "checking"} GitHub`;
    case "web":
      return `${past ? "used" : "using"} ${many("a web page", "web pages")}`;
    case "screen":
      return `${past ? "used" : "using"} the screen`;
    case "server":
      return `${past ? "used" : "using"} ${many("a server", "servers")}`;
    case "other": {
      const names = [...new Set(calls.map((c) => toolName(c.tool)))];
      return `${past ? "used" : "using"} ${names.slice(0, 2).join(" and ")}${names.length > 2 ? " and more" : ""}`;
    }
  }
}

/** Seconds as the owner reads them: "8 s", "2m 18s", "1h 05m". */
export function elapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  if (total < 60) return `${total} s`;
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  if (minutes < 60) return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${String(minutes % 60).padStart(2, "0")}m`;
}

/** "Thought for 6 s", or "Thought for a moment" when it took under a second. */
export function thoughtFor(ms: number): string {
  return ms < 1000 ? "Thought for a moment" : `Thought for ${elapsed(ms)}`;
}
