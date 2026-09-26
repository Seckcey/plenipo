// Plain words for permissions, approvals, and Guard's events (Phase 7).
import type {
  ApprovalStatus,
  Capability,
  Level,
  PermissionSet,
  SensitiveRule,
} from "@plenipo/types";

/** Every capability, in the registry's order, with its plain name. */
export const CAPABILITY_LABEL: Record<Capability, string> = {
  "filesystem.read": "Read files",
  "filesystem.write": "Change files",
  "shell.exec": "Run programs",
  "powershell.exec": "Run PowerShell scripts",
  "git.read": "Read git history",
  "git.write": "Save to git",
  "github.read": "Read GitHub",
  "github.write": "Change GitHub",
  "ssh.connect": "Connect to servers",
  "browser.navigate": "Visit websites",
  "browser.automate": "Use websites",
  "computer.observe": "See the screen",
  "computer.control": "Use the mouse and keyboard",
  "mcp.invoke": "Use add-on tools",
  "network.local": "Reach local services",
  "process.manage": "Manage running programs",
};

export const CAPABILITIES = Object.keys(CAPABILITY_LABEL) as Capability[];

export const LEVELS: Level[] = ["allowed", "ask", "blocked"];

export const LEVEL_LABEL: Record<Level, string> = {
  allowed: "Allowed",
  ask: "Ask me",
  blocked: "Blocked",
};

export const SENSITIVE_RULE_LABEL: Record<SensitiveRule, string> = {
  ask: "Ask me",
  block: "Blocked",
};

export const APPROVAL_STATUS_LABEL: Record<ApprovalStatus, string> = {
  pending: "Waiting",
  approved: "Approved",
  rejected: "Not approved",
  expired: "Expired",
};

export function capabilityLabel(id: string): string {
  return id in CAPABILITY_LABEL ? CAPABILITY_LABEL[id as Capability] : id;
}

export function levelOf(set: PermissionSet | undefined, c: Capability): Level {
  return set?.levels[c] ?? "blocked";
}

/** "Read files, Change files · asks before: Run PowerShell scripts" — what a set allows. */
export function setSummary(set: PermissionSet): string {
  const allowed = CAPABILITIES.filter((c) => levelOf(set, c) === "allowed").map(
    (c) => CAPABILITY_LABEL[c],
  );
  const ask = CAPABILITIES.filter((c) => levelOf(set, c) === "ask").map((c) => CAPABILITY_LABEL[c]);
  const parts: string[] = [];
  if (allowed.length > 0) parts.push(allowed.join(", "));
  if (ask.length > 0) parts.push(`asks you before: ${ask.join(", ")}`);
  return parts.length > 0 ? parts.join(" · ") : "Nothing (conversation only)";
}

/** "8 min left", "40 s left", or "expired". */
export function timeLeft(expiresAt: number | null, now: number): string {
  if (expiresAt === null) return "";
  const s = Math.round((expiresAt - now) / 1000);
  if (s <= 0) return "time is up";
  if (s < 90) return `${s} s left`;
  return `${Math.round(s / 60)} min left`;
}
