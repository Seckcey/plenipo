import { useCallback, useState } from "react";

import { DEFAULT_THEME, THEMES, type ThemeName } from "./tokens";

/** Where the theme is remembered on this computer. */
export const THEME_KEY = "plenipo.theme";

const isTheme = (v: unknown): v is ThemeName => THEMES.includes(v as ThemeName);

/** The remembered theme, or the default (dark). */
export function readTheme(): ThemeName {
  try {
    const saved = localStorage.getItem(THEME_KEY);
    return isTheme(saved) ? saved : DEFAULT_THEME;
  } catch {
    return DEFAULT_THEME;
  }
}

/** Show `theme` everywhere (the tokens switch on `<html data-theme>`). */
export function applyTheme(theme: ThemeName): void {
  document.documentElement.dataset.theme = theme;
}

/** The current theme and a way to change it; the choice is remembered. */
export function useTheme(): [ThemeName, (theme: ThemeName) => void] {
  const [theme, setTheme] = useState<ThemeName>(readTheme);
  const choose = useCallback((next: ThemeName) => {
    setTheme(next);
    applyTheme(next);
    try {
      localStorage.setItem(THEME_KEY, next);
    } catch {
      // Storage unavailable: the choice lasts until the window closes.
    }
  }, []);
  return [theme, choose];
}
