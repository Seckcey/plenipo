// Typed client for the Tauri command boundary.
// This is the ONLY module that calls `invoke`. Components import these functions,
// never `@tauri-apps/api/core` directly (enforced by ESLint).

import { invoke } from "@tauri-apps/api/core";
import type { AppInfo, CommandError, CommandErrorKind } from "@plenipo/types";

/** Error thrown by every command wrapper. Mirrors the Rust `CommandError` DTO. */
export class PlenipoCommandError extends Error {
  readonly kind: CommandErrorKind;

  constructor(kind: CommandErrorKind, message: string) {
    super(message);
    this.name = "PlenipoCommandError";
    this.kind = kind;
  }
}

function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (v.kind === "invalidInput" || v.kind === "internal") && typeof v.message === "string";
}

/** Normalize anything a failed `invoke` rejects with into a PlenipoCommandError. */
export function toCommandError(reason: unknown): PlenipoCommandError {
  if (reason instanceof PlenipoCommandError) return reason;
  if (isCommandError(reason)) return new PlenipoCommandError(reason.kind, reason.message);
  const message =
    reason instanceof Error
      ? reason.message
      : typeof reason === "string"
        ? reason
        : "Unknown error";
  return new PlenipoCommandError("internal", message);
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (reason) {
    throw toCommandError(reason);
  }
}

export function getAppInfo(): Promise<AppInfo> {
  return call<AppInfo>("get_app_info");
}

/** Signals that the shell rendered. Used by the launch smoke test; a no-op otherwise. */
export function frontendReady(): Promise<void> {
  return call<void>("frontend_ready");
}
