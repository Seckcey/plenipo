import type { GithubRepository } from "@plenipo/types";

/** The most repositories the list shows at once; typing narrows it. */
export const SHOWN = 50;

/**
 * The address Plenipo keeps for a picked repository (ADR-204): always built from its checked
 * owner and name, never copied from GitHub's text.
 */
export const addressOf = (r: GithubRepository) => `https://github.com/${r.owner}/${r.name}`;

/** "updated today", "updated 3 days ago", "updated 2 months ago". */
export function updatedWords(updatedAt: string | undefined, now = Date.now()): string | null {
  if (!updatedAt) return null;
  const at = Date.parse(updatedAt);
  if (Number.isNaN(at)) return null;
  const days = Math.max(0, Math.floor((now - at) / 86_400_000));
  if (days === 0) return "updated today";
  if (days === 1) return "updated yesterday";
  if (days < 60) return `updated ${days} days ago`;
  const months = Math.floor(days / 30);
  if (months < 24) return `updated ${months} months ago`;
  return `updated ${Math.floor(months / 12)} years ago`;
}

/** The repositories matching what is typed: by owner and name, or description. */
export function matching(list: GithubRepository[], typed: string): GithubRepository[] {
  const words = typed
    .trim()
    .toLowerCase()
    .replace(/^https?:\/\/(www\.)?github\.com\//, "")
    .replace(/\.git$/, "")
    .split(/\s+/)
    .filter(Boolean);
  if (words.length === 0) return list;
  return list.filter((r) => {
    const text = `${r.owner}/${r.name} ${r.description ?? ""}`.toLowerCase();
    return words.every((w) => text.includes(w));
  });
}
