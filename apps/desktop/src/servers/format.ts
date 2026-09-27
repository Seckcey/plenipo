// Plain words for servers (Phase 11, ADR-025).
import type { CommandClass, Environment, ServerApproval, SignIn } from "@plenipo/types";

export const ENVIRONMENTS: Environment[] = ["development", "staging", "production"];

export const ENVIRONMENT_LABEL: Record<Environment, string> = {
  development: "Test",
  staging: "Staging",
  production: "Production",
};

export const ENVIRONMENT_HINT: Record<Environment, string> = {
  development: "For building and trying things out (development). Nothing customers rely on.",
  staging: "A copy of the live system, for checking changes before they go live.",
  production:
    "The live system your customers use. Every command asks you first, and deleting, wiping, or shutting down is blocked unless you turn it on.",
};

export const SIGN_IN_LABEL: Record<SignIn, string> = {
  key: "A private key",
  password: "A password",
  agent: "My SSH agent",
};

/** How Plenipo signs in, as part of a sentence ("with a private key"). */
export const SIGN_IN_PHRASE: Record<SignIn, string> = {
  key: "a private key",
  password: "a password",
  agent: "your SSH agent",
};

export const APPROVAL_LABEL: Record<ServerApproval, string> = {
  every: "Ask me before every command",
  changes: "Ask me before anything that is not looking around",
  allowed: "Don't ask for the kinds of commands I allow",
};

/** When the owner is asked on a server, in a line. */
export function approvalWords(environment: Environment, approval: ServerApproval): string {
  if (environment === "production") return "Before every command (production)";
  switch (approval) {
    case "every":
      return "Before every command";
    case "changes":
      return "Before anything that is not looking around";
    case "allowed":
      return "Only before deleting, running as administrator, and other sensitive actions";
  }
}

/** The kinds a new server allows (production starts without changes or destroying). */
export function defaultClasses(environment: Environment): CommandClass[] {
  return environment === "production" ? ["look", "services"] : ["look", "services", "change"];
}

/** One entry per line: trimmed, blank lines dropped. */
export function lines(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
}
