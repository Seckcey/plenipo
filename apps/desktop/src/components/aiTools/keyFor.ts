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
