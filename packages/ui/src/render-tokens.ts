/** Renders the token source (`tokens.ts`) as CSS custom properties and as JSON. */

import {
  COLOR_ROLES,
  DEFAULT_THEME,
  EASING,
  ELEVATION,
  FONT_FAMILY,
  FONT_SIZE,
  FONT_WEIGHT,
  LINE_HEIGHT,
  MOTION,
  PALETTES,
  RADIUS,
  SIZE,
  SPACE,
  THEMES,
  cssVar,
  type ColorToken,
  type ThemeName,
} from "./tokens";

const HEADER =
  "Generated from packages/ui/src/tokens.ts by `pnpm tokens`. Do not edit by hand (ADR-029).";

const px = (n: number) => (n === 0 ? "0" : `${n}px`);

/** Theme-independent properties: type, spacing, radius, sizes, motion. */
function scaleProperties(): [string, string][] {
  return [
    ...Object.entries(FONT_FAMILY).map(([k, v]) => [cssVar(`font-${k}`), v] as [string, string]),
    ...Object.entries(FONT_SIZE).map(([k, v]) => [cssVar(`font-${k}`), px(v)] as [string, string]),
    ...Object.entries(FONT_WEIGHT).map(
      ([k, v]) => [cssVar(`weight-${k}`), String(v)] as [string, string],
    ),
    ...Object.entries(LINE_HEIGHT).map(
      ([k, v]) => [cssVar(`leading-${k}`), String(v)] as [string, string],
    ),
    ...Object.entries(SPACE).map(([k, v]) => [cssVar(`space-${k}`), px(v)] as [string, string]),
    ...Object.entries(RADIUS).map(([k, v]) => [cssVar(`radius-${k}`), px(v)] as [string, string]),
    ...Object.entries(SIZE).map(([k, v]) => [cssVar(k), px(v)] as [string, string]),
    ...Object.entries(MOTION).map(
      ([k, v]) => [cssVar(`motion-${k}`), `${v}ms`] as [string, string],
    ),
    [cssVar("ease"), EASING],
  ];
}

function colorProperties(theme: ThemeName): [string, string][] {
  const palette = PALETTES[theme];
  const colors = (Object.keys(COLOR_ROLES) as ColorToken[]).map(
    (name) => [cssVar(name), palette[name]] as [string, string],
  );
  const shadows = Object.entries(ELEVATION).map(
    ([k, geometry]) =>
      [cssVar(`elevation-${k}`), `${geometry} ${palette.shadow}`] as [string, string],
  );
  return [["color-scheme", theme], ...colors, ...shadows];
}

const block = (selector: string, props: [string, string][]) =>
  `${selector} {\n${props.map(([k, v]) => `  ${k}: ${v};`).join("\n")}\n}\n`;

/**
 * `tokens.css`: the default theme on `:root`, and each theme on any `[data-theme]` element,
 * so `<html data-theme>` switches the app and the Gallery can show both themes side by side.
 */
export function renderTokensCss(): string {
  const parts = [
    `/* ${HEADER} */\n`,
    block(":root", [...scaleProperties(), ...colorProperties(DEFAULT_THEME)]),
    ...THEMES.map((theme) => block(`[data-theme="${theme}"]`, colorProperties(theme))),
  ];
  return parts.join("\n");
}

/** `tokens.json`: every value, for the Rust side and exports. */
export function renderTokensJson(): string {
  return `${JSON.stringify(
    {
      $comment: HEADER,
      defaultTheme: DEFAULT_THEME,
      colors: PALETTES,
      roles: COLOR_ROLES,
      fontFamily: FONT_FAMILY,
      fontSize: FONT_SIZE,
      fontWeight: FONT_WEIGHT,
      lineHeight: LINE_HEIGHT,
      space: SPACE,
      radius: RADIUS,
      size: SIZE,
      elevation: ELEVATION,
      motion: MOTION,
      easing: EASING,
    },
    null,
    2,
  )}\n`;
}
