/** OpenRouter's paid AI tool: its key reaches hundreds of models from many AI companies. */
export const OPENROUTER = "openrouter";

/**
 * The paid AI tool whose key a subscription AI tool's card offers (the owner's direction,
 * 2026-09-30): the same AI company's own key, or OpenRouter's for an AI tool whose company sells
 * no per-use key of its own (Ollama, GitHub Copilot). One key in two places: saved on either
 * card, it is the same key, kept once in the Vault (ADR-085 §5). The subscription AI tool itself
 * never gets the key.
 */
const KEY_FOR: Readonly<Record<string, string>> = {
  "claude-code": "anthropic-key",
  codex: "openai-key",
  grok: "xai-key",
  kimi: "moonshot-key",
  antigravity: "google-key",
};

/** The paid AI tool whose key `runtimeId`'s card offers. */
export function keyToolFor(runtimeId: string): string {
  return KEY_FOR[runtimeId] ?? OPENROUTER;
}

/**
 * The subscription AI tool whose card a company's key card is folded into (Phase 25, item 2.1):
 * Anthropic's into Claude Code's, and so on. `null`: it keeps its own card (OpenRouter, and the
 * AI companies with no subscription AI tool in Plenipo).
 */
export function foldedInto(paidId: string): string | null {
  return Object.entries(KEY_FOR).find(([, key]) => key === paidId)?.[0] ?? null;
}

/**
 * Subscription AI tools whose models are linked to the same models on their company's key, so
 * that while the plan is out the work runs on the key (Phase 25, item 4.4; ADR-204).
 */
const MOVES_TO_KEY: ReadonlySet<string> = new Set(["claude-code", "codex"]);

/** Whether `runtimeId`'s work moves to its company's key while its plan is out. */
export function movesToKey(runtimeId: string): boolean {
  return MOVES_TO_KEY.has(runtimeId);
}
