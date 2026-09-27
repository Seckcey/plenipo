/**
 * Plenipo's brand, from the owner's approved kit (docs/brand/pip-brand-kit): the P, the logo with
 * Pip on the n, and Pip, Plenipo's robot. Every color comes from the brand tokens, so the logo
 * follows the theme without a second image.
 */

import { P_RAILS, PIP_IMAGES, WORDMARK_LETTERS, type PipPose } from "./brand-data";
import { cx } from "./util";

export type PipSize = "sm" | "md" | "lg";

function Rails() {
  return (
    <g fill="none" strokeWidth={7} strokeLinecap="round" strokeLinejoin="round">
      {P_RAILS.map((r) => (
        <path key={r.rail} d={r.d} className={`ui-brand__rail ui-brand__rail--${r.rail}`} />
      ))}
    </g>
  );
}

/**
 * The P alone: Plenipo's mark where the whole logo would be too small (the left strip).
 * Decorative unless it has a `label`.
 */
export function PlenipoMark({
  size = 28,
  label,
  className,
}: {
  size?: number;
  label?: string | undefined;
  className?: string | undefined;
}) {
  return (
    <svg
      className={cx("ui-brand-mark", className)}
      width={size}
      height={size}
      viewBox="0 0 100 100"
      focusable="false"
      {...(label ? { role: "img", "aria-label": label } : { "aria-hidden": true })}
    >
      <g transform="translate(-0.5 -3)">
        <Rails />
      </g>
    </svg>
  );
}

const LOGO_BOX = { horizontal: [749, 298], square: [800, 800] } as const;

/**
 * The logo. `horizontal`: the P as the capital, then "lenipo", with Pip at his laptop on the n.
 * `square`: Pip at his laptop on a large P, no name. Sized by `height`; the width follows.
 */
export function PlenipoLogo({
  variant = "horizontal",
  height = 40,
  label = "Plenipo",
  className,
}: {
  variant?: "horizontal" | "square";
  height?: number;
  label?: string;
  className?: string | undefined;
}) {
  const [w, h] = LOGO_BOX[variant];
  return (
    <svg
      className={cx("ui-logo", `ui-logo--${variant}`, className)}
      viewBox={`0 0 ${w} ${h}`}
      width={Math.round((height * w) / h)}
      height={height}
      role="img"
      aria-label={label}
      focusable="false"
    >
      {variant === "square" ? (
        <>
          <g transform="translate(94.75 196.75) scale(5.5)">
            <Rails />
          </g>
          <image x={273} y={28} width={325} height={325} href={PIP_IMAGES.coding} />
        </>
      ) : (
        <>
          <g transform="translate(-8.6024 2.8434) scale(2.3614)">
            <Rails />
          </g>
          <g transform="translate(200.8012 226)">
            {WORDMARK_LETTERS.map((l) => (
              <path
                key={l.letter}
                className={l.letter === "o" ? "ui-brand__o" : "ui-brand__ink"}
                transform={`translate(${l.x} 0) scale(0.18 -0.18)`}
                d={l.d}
              />
            ))}
          </g>
          <image x={329} y={32} width={132} height={132} href={PIP_IMAGES.coding} />
        </>
      )}
    </svg>
  );
}

/**
 * Pip, Plenipo's robot, in one of his 15 poses. Decorative unless it has a `label`: the words
 * next to Pip carry the meaning.
 */
export function Pip({
  pose,
  size = "md",
  label,
  className,
}: {
  pose: PipPose;
  size?: PipSize;
  label?: string | undefined;
  className?: string | undefined;
}) {
  return (
    <img
      className={cx("ui-pip", `ui-pip--${size}`, className)}
      src={PIP_IMAGES[pose]}
      alt={label ?? ""}
      draggable={false}
      data-pip={pose}
    />
  );
}
