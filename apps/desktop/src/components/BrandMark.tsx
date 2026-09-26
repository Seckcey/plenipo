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
      <rect width="64" height="64" rx="14" fill="#0B1220" />
      <path
        d="M22 48V16h13a10 10 0 0 1 0 20H22"
        fill="none"
        stroke="#2E8BFF"
        strokeWidth="6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="44" cy="46" r="4" fill="#2E8BFF" />
    </svg>
  );
}
