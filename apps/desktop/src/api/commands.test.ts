import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";

import {
  cancelAiToolUpdate,
  checkAiTool,
  checkAiToolVersions,
  getAiTools,
  getAiToolUsage,
  setAiToolPayment,
  setAiToolsAutoUpdate,
  getSpending,
  removePaidKey,
  removeSpendingCap,
  savePaidKey,
  setSpendingCap,
  updateAiTool,
  archiveDepartment,
  bringBack,
  deleteForGood,
  hireFromWorkforce,
  previewDeleteForGood,
  setAgentLearning,
  setModelRule,
  assignPermissions,
  frontendReady,
  resolveApproval,
  saveSecret,
  getAppInfo,
  getLiaisonOverview,
  getTaskHandoffs,
  getTaskTree,
  PlenipoCommandError,
  setOrganizationTitles,
  startAgentSession,
  toCommandError,
} from "./commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockedInvoke = vi.mocked(invoke);

describe("command client", () => {
  it("calls get_app_info and returns the DTO", async () => {
    const info = {
      name: "Plenipo",
      version: "0.1.0",
      buildProfile: "debug",
      os: "windows",
      arch: "x86_64",
    };
    mockedInvoke.mockResolvedValueOnce(info);
    await expect(getAppInfo()).resolves.toEqual(info);
    expect(mockedInvoke).toHaveBeenCalledWith("get_app_info", undefined);
  });

  it("calls frontend_ready", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    await frontendReady();
    expect(mockedInvoke).toHaveBeenCalledWith("frontend_ready", undefined);
  });

  it("starts sessions without handoffs unless asked", async () => {
    mockedInvoke.mockResolvedValue({});
    await startAgentSession("codex", "Write a parser", " ");
    expect(mockedInvoke).toHaveBeenLastCalledWith("start_agent_session", {
      runtimeId: "codex",
      objective: "Write a parser",
      model: null,
      handoffs: false,
    });
    await startAgentSession("claude-code", "Plan it", undefined, true);
    expect(mockedInvoke).toHaveBeenLastCalledWith("start_agent_session", {
      runtimeId: "claude-code",
      objective: "Plan it",
      model: null,
      handoffs: true,
    });
  });

  it("calls the Liaison queries with the task ID", async () => {
    mockedInvoke.mockResolvedValue({});
    await getTaskHandoffs("t-1");
    expect(mockedInvoke).toHaveBeenLastCalledWith("get_task_handoffs", { taskId: "t-1" });
    await getTaskTree("t-2");
    expect(mockedInvoke).toHaveBeenLastCalledWith("get_task_tree", { taskId: "t-2" });
    await getLiaisonOverview();
    expect(mockedInvoke).toHaveBeenLastCalledWith("get_liaison_overview", undefined);
  });

  it("sends the owner's control over workers to its commands (Phase 17)", async () => {
    mockedInvoke.mockResolvedValue({});
    await setModelRule(
      { layer: "department", id: "d-1" },
      {
        models: [],
        efforts: {},
        effort: "high",
        neverCompanies: [],
      },
    );
    expect(mockedInvoke).toHaveBeenLastCalledWith("set_model_rule", {
      target: { layer: "department", id: "d-1" },
      rule: { models: [], efforts: {}, effort: "high", neverCompanies: [] },
    });
    await setAgentLearning("p-1", null);
    expect(mockedInvoke).toHaveBeenLastCalledWith("set_agent_learning", {
      positionId: "p-1",
      learns: null,
    });
    await archiveDepartment("d-1");
    expect(mockedInvoke).toHaveBeenLastCalledWith("archive_department", { departmentId: "d-1" });
    await bringBack("position", "p-1");
    expect(mockedInvoke).toHaveBeenLastCalledWith("bring_back_position", { positionId: "p-1" });
    await bringBack("project", "pr-1");
    expect(mockedInvoke).toHaveBeenLastCalledWith("bring_back_project", { projectId: "pr-1" });
    await bringBack("department", "d-1");
    expect(mockedInvoke).toHaveBeenLastCalledWith("bring_back_department", {
      departmentId: "d-1",
    });
    await previewDeleteForGood("project", "pr-1");
    expect(mockedInvoke).toHaveBeenLastCalledWith("preview_delete_for_good", {
      kind: "project",
      id: "pr-1",
    });
    await deleteForGood("project", "pr-1", ["p-2"]);
    expect(mockedInvoke).toHaveBeenLastCalledWith("delete_for_good", {
      kind: "project",
      id: "pr-1",
      save: ["p-2"],
    });
    await hireFromWorkforce("w-1", null, "  ");
    expect(mockedInvoke).toHaveBeenLastCalledWith("hire_from_workforce", {
      savedId: "w-1",
      reportsTo: null,
      title: null,
    });
  });

  it("sets the organization's titles", async () => {
    mockedInvoke.mockResolvedValue({});
    await setOrganizationTitles("navy");
    expect(mockedInvoke).toHaveBeenLastCalledWith("set_organization_titles", { titles: "navy" });
  });

  it("converts a structured backend error", async () => {
    mockedInvoke.mockRejectedValueOnce({ kind: "invalidInput", message: "nope" });
    const err = await getAppInfo().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(PlenipoCommandError);
    expect(err).toMatchObject({ kind: "invalidInput", message: "nope" });
  });

  it("converts unstructured failures to internal errors", () => {
    expect(toCommandError("IPC down")).toMatchObject({ kind: "internal", message: "IPC down" });
    expect(toCommandError(new Error("boom"))).toMatchObject({ kind: "internal", message: "boom" });
    expect(toCommandError(42)).toMatchObject({ kind: "internal", message: "Unknown error" });
    expect(toCommandError({ kind: "shellExec", message: "x" })).toMatchObject({ kind: "internal" });
  });

  it("sends permissions, approvals, and secrets by name only", async () => {
    mockedInvoke.mockResolvedValue({});
    await assignPermissions("role", "r-1", null);
    expect(mockedInvoke).toHaveBeenLastCalledWith("assign_permissions", {
      target: "role",
      id: "r-1",
    });
    await assignPermissions("department", "d-1", "read-only");
    expect(mockedInvoke).toHaveBeenLastCalledWith("assign_permissions", {
      target: "department",
      id: "d-1",
      setId: "read-only",
    });
    await resolveApproval("a-1", false);
    expect(mockedInvoke).toHaveBeenLastCalledWith("resolve_approval", {
      approvalId: "a-1",
      approve: false,
    });
    await saveSecret({ name: "Token", programs: [], value: "v" });
    expect(mockedInvoke).toHaveBeenLastCalledWith("save_secret", {
      input: { name: "Token", programs: [], value: "v" },
    });
  });

  it("sends the AI tools page's commands by tool ID (Phase 19)", async () => {
    mockedInvoke.mockResolvedValue({});
    await getAiTools();
    expect(mockedInvoke).toHaveBeenLastCalledWith("get_ai_tools", undefined);
    await checkAiTool("codex");
    expect(mockedInvoke).toHaveBeenLastCalledWith("check_ai_tool", { runtimeId: "codex" });
    await checkAiToolVersions();
    expect(mockedInvoke).toHaveBeenLastCalledWith("check_ai_tool_versions", undefined);
    await getAiToolUsage("kimi", [1, 2, 3]);
    expect(mockedInvoke).toHaveBeenLastCalledWith("get_ai_tool_usage", {
      runtimeId: "kimi",
      dayStarts: [1, 2, 3],
    });
    await updateAiTool("grok");
    expect(mockedInvoke).toHaveBeenLastCalledWith("update_ai_tool", { runtimeId: "grok" });
    await cancelAiToolUpdate("grok");
    expect(mockedInvoke).toHaveBeenLastCalledWith("cancel_ai_tool_update", { runtimeId: "grok" });
    await setAiToolsAutoUpdate(true);
    expect(mockedInvoke).toHaveBeenLastCalledWith("set_ai_tools_auto_update", { on: true });
    await setAiToolPayment("codex", "subscription");
    expect(mockedInvoke).toHaveBeenLastCalledWith("set_ai_tool_payment", {
      runtimeId: "codex",
      method: "subscription",
    });
  });

  it("sends the spending commands their arguments (Phase 16 Wave 3)", async () => {
    await getSpending();
    expect(mockedInvoke).toHaveBeenLastCalledWith("get_spending", undefined);
    await setSpendingCap({ kind: "department", id: "d1" }, 25_000_000);
    expect(mockedInvoke).toHaveBeenLastCalledWith("set_spending_cap", {
      covers: { kind: "department", id: "d1" },
      monthlyMicros: 25_000_000,
    });
    await removeSpendingCap("cap-1");
    expect(mockedInvoke).toHaveBeenLastCalledWith("remove_spending_cap", { capId: "cap-1" });
  });

  it("sends a paid key only to its own command (Phase 16 Wave 3)", async () => {
    await savePaidKey("openrouter", "Office key", "sk-or-v1-made-up");
    expect(mockedInvoke).toHaveBeenLastCalledWith("save_paid_key", {
      runtimeId: "openrouter",
      name: "Office key",
      key: "sk-or-v1-made-up",
    });
    await removePaidKey("openrouter");
    expect(mockedInvoke).toHaveBeenLastCalledWith("remove_paid_key", { runtimeId: "openrouter" });
  });
});
