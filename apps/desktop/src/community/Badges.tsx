import { badgeReason, badgeWords } from "./peopleWords";

/**
 * A person's badges as words (Phase 24, ADR-169 §3), on a card, on the leaderboard, and in your own
 * points. Each has its one-line reason: as a `title` for the mouse, and as words only a screen
 * reader says (a `title` alone is not read out reliably). A badge this copy of Plenipo does not
 * know is left out, and a badge named twice shows once. No badges, no list.
 */
export function BadgeList({
  badges,
  label = "Badges",
  className = "people-card__badges",
}: {
  badges: readonly string[];
  /** What the list is called for a screen reader. */
  label?: string;
  className?: string;
}) {
  const shown = [
    ...new Map(
      badges.flatMap((badge) => {
        const words = badgeWords(badge);
        return words ? [[words, badgeReason(badge) ?? ""] as const] : [];
      }),
    ),
  ];
  if (shown.length === 0) return null;
  return (
    <ul className={className} aria-label={label}>
      {shown.map(([words, reason]) => (
        <li key={words} title={reason}>
          <span>{words}</span>
          <span className="visually-hidden">. {reason}</span>
        </li>
      ))}
    </ul>
  );
}
