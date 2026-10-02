/** "9 minutes", "1 minute", "less than a minute". */
export function minutesLeft(endsAt: number, now: number): string {
  const ms = endsAt - now;
  if (ms < 60_000) return "less than a minute";
  const m = Math.floor(ms / 60_000);
  return m === 1 ? "1 minute" : `${m} minutes`;
}
