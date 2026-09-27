import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import globals from "globals";
import tseslint from "typescript-eslint";

import { COLOR_KEYS, TS_NAMED, TS_RAW } from "./scripts/colors.mjs";

const RAW_COLOR = "Raw color: use a design token, e.g. var(--ui-accent) (ADR-030).";

export default tseslint.config(
  {
    ignores: [
      "**/dist/**",
      "**/node_modules/**",
      "**/target/**",
      "**/coverage/**",
      "apps/desktop/src-tauri/**",
      "packages/types/src/generated/**",
      // Archived, approved generator is retained byte for byte with its kit manifest.
      "assets/source/build-kit.cjs",
      // The owner's approved brand kit, kept exactly as delivered.
      "docs/brand/pip-brand-kit/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommendedTypeChecked,
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
      "@typescript-eslint/consistent-type-imports": "error",
      // All IPC goes through the typed client in src/api/commands.ts.
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "@tauri-apps/api/core",
              importNames: ["invoke"],
              message: "Use the typed wrappers in src/api/commands.ts instead of raw invoke().",
            },
            {
              name: "@tauri-apps/api/event",
              message: "Subscribe through src/api/events.ts instead of calling listen() directly.",
            },
          ],
        },
      ],
    },
  },
  {
    // No raw colors in feature code: every color is a design token (ADR-030 §3). Colors are
    // written only in packages/ui/src/tokens.ts; scripts/check-colors.mjs checks the CSS.
    files: ["apps/desktop/src/**/*.{ts,tsx}", "packages/ui/src/**/*.{ts,tsx}"],
    ignores: ["packages/ui/src/tokens.ts", "**/*.test.{ts,tsx}"],
    rules: {
      "no-restricted-syntax": [
        "error",
        // Hex colors (also %23… in a data: image) and color functions, in any string.
        { selector: `Literal[value=${TS_RAW}]`, message: RAW_COLOR },
        { selector: `TemplateElement[value.raw=${TS_RAW}]`, message: RAW_COLOR },
        // Named colors, where a color goes: a style property or an SVG color attribute.
        {
          selector: `Property[key.name=${COLOR_KEYS}] > Literal[value=${TS_NAMED}]`,
          message: RAW_COLOR,
        },
        {
          selector: `Property[key.name=${COLOR_KEYS}] > TemplateLiteral > TemplateElement[value.raw=${TS_NAMED}]`,
          message: RAW_COLOR,
        },
        {
          selector: `JSXAttribute[name.name=${COLOR_KEYS}] > Literal[value=${TS_NAMED}]`,
          message: RAW_COLOR,
        },
      ],
    },
  },
  {
    files: [
      "apps/desktop/src/api/commands.ts",
      "apps/desktop/src/api/events.ts",
      "**/*.test.{ts,tsx}",
    ],
    rules: { "no-restricted-imports": "off" },
  },
  {
    files: ["**/*.{js,mjs}"],
    ...tseslint.configs.disableTypeChecked,
    languageOptions: { globals: globals.node },
  },
  {
    // Scripts Plenipo runs inside web pages of its browser (Phase 10).
    files: ["crates/**/*.js"],
    languageOptions: { globals: globals.browser },
  },
  {
    // E2E specs run in Node, but browser.execute() callbacks run inside the webview.
    files: ["tests/e2e/**/*.mjs"],
    languageOptions: { globals: { ...globals.node, ...globals.browser } },
  },
);
