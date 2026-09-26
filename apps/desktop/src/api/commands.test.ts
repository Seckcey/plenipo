import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";

import {
  frontendReady,
  getAppInfo,
  getLiaisonOverview,
  getTaskHandoffs,
  getTaskTree,
  PlenipoCommandError,
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
});
