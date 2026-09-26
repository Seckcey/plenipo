import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";

import { frontendReady, getAppInfo, PlenipoCommandError, toCommandError } from "./commands";

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
