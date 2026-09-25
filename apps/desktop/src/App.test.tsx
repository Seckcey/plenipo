import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import * as commands from "./api/commands";

vi.mock("./api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, getAppInfo: vi.fn(), frontendReady: vi.fn() };
});

const getAppInfo = vi.mocked(commands.getAppInfo);
const frontendReady = vi.mocked(commands.frontendReady);

describe("App shell", () => {
  beforeEach(() => {
    frontendReady.mockResolvedValue(undefined);
  });

  it("renders the branded shell and runtime details", async () => {
    getAppInfo.mockResolvedValue({
      name: "Plenipo",
      version: "0.1.0",
      buildProfile: "release",
      os: "windows",
      arch: "x86_64",
    });

    render(<App />);

    expect(screen.getByText("Plenipo")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(/AI workforce/i);
    expect(await screen.findByText("Connected")).toBeInTheDocument();
    expect(screen.getByLabelText("Application version")).toHaveTextContent("v0.1.0");
    expect(screen.getByText("windows / x86_64")).toBeInTheDocument();
    expect(screen.getByText("None configured")).toBeInTheDocument();
    expect(frontendReady).toHaveBeenCalledTimes(1);
  });

  it("shows an error and does not report ready when Core is unavailable", async () => {
    getAppInfo.mockRejectedValue(new commands.PlenipoCommandError("internal", "IPC down"));

    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("IPC down");
    expect(frontendReady).not.toHaveBeenCalled();
  });

  it("requires no credentials to render", async () => {
    getAppInfo.mockResolvedValue({
      name: "Plenipo",
      version: "0.1.0",
      buildProfile: "debug",
      os: "linux",
      arch: "x86_64",
    });
    render(<App />);
    await screen.findByText("Connected");
    expect(screen.queryByLabelText(/api key|password|token/i)).not.toBeInTheDocument();
  });
});
