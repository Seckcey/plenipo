// Guard / capability broker DTO fixtures for tests.
import type {
  ApprovalQueue,
  ApprovalView,
  Capability,
  CapabilityInfo,
  PermissionSet,
  PermissionsSnapshot,
} from "@plenipo/types";

import { CAPABILITIES, CAPABILITY_LABEL } from "../guard/format";

export const T0 = Date.UTC(2026, 8, 26, 15, 0, 0);

const set = (
  id: string,
  name: string,
  levels: PermissionSet["levels"],
  builtIn = true,
): PermissionSet => ({ id, name, description: `${name} work.`, levels, builtIn });

const LATER: Capability[] = [
  "github.read",
  "github.write",
  "ssh.connect",
  "browser.navigate",
  "browser.automate",
  "computer.observe",
  "computer.control",
  "mcp.invoke",
  "network.local",
  "process.manage",
];

export function samplePermissions(patch: Partial<PermissionsSnapshot> = {}): PermissionsSnapshot {
  const capabilities: CapabilityInfo[] = CAPABILITIES.map((id) => ({
    id,
    label: CAPABILITY_LABEL[id],
    description: `${CAPABILITY_LABEL[id]} in the project folder.`,
    tools: !LATER.includes(id),
    ...(LATER.includes(id) ? { arrives: "Phase 10" } : {}),
  }));
  return {
    settings: {
      capabilities,
      sets: [
        set("read-only", "Read only", { "filesystem.read": "allowed", "git.read": "allowed" }),
        set("developer", "Developer", {
          "filesystem.read": "allowed",
          "filesystem.write": "allowed",
          "shell.exec": "allowed",
          "powershell.exec": "ask",
          "git.read": "allowed",
          "git.write": "allowed",
        }),
        set("docs", "Docs", { "filesystem.read": "allowed" }, false),
      ],
      roles: [
        { roleId: "role-dev", roleName: "Senior Developer", fullTime: false, setId: "developer" },
        { roleId: "role-rev", roleName: "Code Reviewer", fullTime: false },
      ],
      departments: [{ id: "dept-1", name: "Development" }],
      projects: [
        { id: "proj-1", name: "Website", folder: "D:\\projects\\website", setId: "read-only" },
        {
          id: "proj-2",
          name: "Mobile",
          problem: "It has no folder, so its workers cannot use files, programs, or git.",
        },
      ],
      commands: { approved: ["cargo test *", "npm test *"], ask: [], blocked: ["curl *"] },
      blockedFiles: [".env", "*.pem"],
      sensitive: [
        {
          kind: "outbound",
          label: "Sending or publishing outside this computer",
          examples: "git push, npm publish",
          rule: "ask",
        },
        { kind: "dns", label: "Changing DNS", examples: "aws route53", rule: "block" },
      ],
      options: { approvalMinutes: 10 },
      secrets: [
        {
          id: "secret-1",
          name: "GitHub token",
          envVar: "GH_TOKEN",
          programs: ["gh"],
          createdAt: T0,
          updatedAt: T0,
        },
      ],
    },
    vault: {
      available: true,
      label: "Windows Credential Manager",
      stored: ["secret-1"],
    },
    tools: { running: true, detail: "Ready: workers with permissions get Plenipo's tools." },
    grants: [
      {
        grantId: "grant-1",
        taskId: "task-7",
        sessionId: "session-7",
        step: 1,
        positionId: "pos-dev",
        worker: "Backend Developer",
        role: "Senior Developer",
        project: "Website",
        folder: "D:\\projects\\website",
        permissions: [
          { capability: "filesystem.read", label: "Read files", level: "allowed" },
          { capability: "powershell.exec", label: "Run PowerShell scripts", level: "ask" },
        ],
        openedAt: T0 - 60_000,
        revoked: false,
        used: 3,
        blocked: 1,
        asked: 1,
      },
    ],
    blocked: [
      {
        at: T0 - 30_000,
        taskId: "task-7",
        worker: "Backend Developer",
        capabilityLabel: "Read files",
        summary: "read ../outside.txt",
        reason: "Blocked: ../outside.txt is outside the project folder.",
      },
    ],
    notices: [],
    ...patch,
  };
}

export function approval(patch: Partial<ApprovalView> = {}): ApprovalView {
  return {
    id: "approval-1",
    taskId: "task-7",
    status: "pending",
    requestedAt: T0 - 60_000,
    expiresAt: T0 + 9 * 60_000,
    resolvedAt: null,
    worker: "Backend Developer",
    role: "Senior Developer",
    project: "Website",
    folder: "D:\\projects\\website",
    capability: "git.write",
    capabilityLabel: "Save to git",
    summary: "git push origin",
    detail: "git push origin",
    reason:
      "Git push origin needs your approval: it sends commits to a server (Sending or publishing outside this computer).",
    risk: "external",
    riskLabel: "Reaches outside this computer",
    sensitive: "outbound",
    sensitiveLabel: "Sending or publishing outside this computer",
    grantId: "grant-1",
    sessionId: "session-7",
    waiting: true,
    ...patch,
  };
}

export function sampleQueue(patch: Partial<ApprovalQueue> = {}): ApprovalQueue {
  return {
    pending: [approval()],
    recent: [
      approval({
        id: "approval-0",
        status: "rejected",
        summary: "run npm publish",
        resolvedAt: T0 - 120_000,
        resolvedBy: "owner",
        note: "Refused by you.",
        waiting: false,
      }),
    ],
    ...patch,
  };
}
