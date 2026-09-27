/**
 * Plenipo's own line icons: simple strokes on a 24-unit grid, drawn in the current text color.
 * No UniFi (or other) artwork (ADR-029 §5).
 */

import { ICONS, type IconName, type Shape } from "./icon-data";

export type { IconName } from "./icon-data";

export function Icon({
  name,
  size = 18,
  label,
  className,
}: {
  name: IconName;
  size?: number;
  /** Spoken name; without it the icon is decoration (hidden from screen readers). */
  label?: string;
  className?: string;
}) {
  const shape: Shape = ICONS[name];
  return (
    <svg
      className={className ? `ui-icon ${className}` : "ui-icon"}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={name === "more" ? 3 : 1.7}
      strokeLinecap="round"
      strokeLinejoin="round"
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      focusable="false"
    >
      <path d={shape.d} />
      {shape.circles?.map(([cx, cy, r]) => (
        <circle key={`${cx},${cy}`} cx={cx} cy={cy} r={r} />
      ))}
    </svg>
  );
}
