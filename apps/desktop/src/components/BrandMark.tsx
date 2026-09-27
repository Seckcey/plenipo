/** Plenipo monogram. Decorative; the wordmark next to it carries the name. */
export function BrandMark({ size = 28 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      aria-hidden="true"
      focusable="false"
      className="brand-mark"
    >
      <rect width="64" height="64" rx="14" className="brand-mark__tile" />
      <path
        d="M22 48V16h13a10 10 0 0 1 0 20H22"
        className="brand-mark__line"
        strokeWidth="6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="44" cy="46" r="4" className="brand-mark__dot" />
    </svg>
  );
}
