import { composite, contrast, parseColor } from "./color";
import { renderTokensCss, renderTokensJson } from "./render-tokens";
import { COLOR_ROLES, CONTRAST_PAIRS, PALETTES, THEMES, type ColorToken } from "./tokens";

describe("generated token files", () => {
  // `pnpm tokens` rewrites them (vitest -u); CI refuses a stale copy.
  it("tokens.css matches tokens.ts", async () => {
    await expect(renderTokensCss()).toMatchFileSnapshot("./generated/tokens.css");
  });

  it("tokens.json matches tokens.ts", async () => {
    await expect(renderTokensJson()).toMatchFileSnapshot("./generated/tokens.json");
  });
});

describe("palettes", () => {
  it.each(THEMES)("%s defines every color token with a real color", (theme) => {
    const palette = PALETTES[theme];
    expect(Object.keys(palette).sort()).toEqual(Object.keys(COLOR_ROLES).sort());
    for (const [name, value] of Object.entries(palette)) {
      expect(() => parseColor(value), `${theme} ${name}`).not.toThrow();
    }
  });
});

describe("contrast (WCAG AA)", () => {
  /** A token's color, laid over `over` (default: surface) when it is translucent. */
  const opaque = (theme: (typeof THEMES)[number], token: ColorToken, over?: ColorToken) => {
    const color = parseColor(PALETTES[theme][token]);
    if (color.a >= 1) return color;
    return composite(color, parseColor(PALETTES[theme][over ?? "surface"]));
  };

  for (const theme of THEMES) {
    it.each(CONTRAST_PAIRS.map((p) => [`${p.fg} on ${p.bg}`, p] as const))(
      `${theme}: %s`,
      (_label, pair) => {
        const bg = opaque(theme, pair.bg, pair.over);
        const fg = parseColor(PALETTES[theme][pair.fg]);
        const ratio = contrast(fg.a < 1 ? composite(fg, bg) : fg, bg);
        expect(
          ratio,
          `${theme}: ${pair.fg} on ${pair.bg} is ${ratio.toFixed(2)}`,
        ).toBeGreaterThanOrEqual(pair.min);
      },
    );
  }

  it("checks every text token against every surface", () => {
    const texts = CONTRAST_PAIRS.filter((p) => p.min === 4.5 && p.bg === "surface").map(
      (p) => p.fg,
    );
    expect(texts).toEqual(
      expect.arrayContaining(["text-primary", "text-secondary", "text-muted", "accent"]),
    );
  });
});

describe("color math", () => {
  it("matches known WCAG ratios", () => {
    expect(contrast(parseColor("#000"), parseColor("#fff"))).toBeCloseTo(21, 5);
    expect(contrast(parseColor("#777777"), parseColor("#ffffff"))).toBeCloseTo(4.48, 2);
  });

  it("lays a translucent color over a background", () => {
    const c = composite(parseColor("rgba(255, 255, 255, 0.5)"), parseColor("#000000"));
    expect(c.r).toBeCloseTo(127.5);
  });
});
