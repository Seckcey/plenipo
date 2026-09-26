import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "**/dist/**",
      "**/node_modules/**",
      "**/target/**",
      "**/coverage/**",
      "apps/desktop/src-tauri/**",
      "packages/types/src/generated/**",
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
    // E2E specs run in Node, but browser.execute() callbacks run inside the webview.
    files: ["tests/e2e/**/*.mjs"],
    languageOptions: { globals: { ...globals.node, ...globals.browser } },
  },
);
