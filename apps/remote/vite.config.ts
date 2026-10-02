/// <reference types="vitest/config" />
import react from "@vitejs/plugin-react";
import { build, defineConfig, type Plugin } from "vite";

/**
 * Where the page reaches 8 West's relay (ADR-146): Plenipo's own name for it, or, for the end-to-end
 * tests only, a stand-in on this computer. Built in; never a setting.
 */
const RELAY =
  process.env.PLENIPO_RELAY_PHONE?.trim() || "wss://relay.getplenipo.com/plenipo/v1/phone";

/** The page's version (the root package.json, as every package's is). */
const VERSION = process.env.npm_package_version ?? "0.0.0";

/**
 * The page's rules (a Content Security Policy, ADR-146 §3): only its own files, and only the relay
 * for connections; no code made at run time, no outside scripts, fonts, or counters. Added to the
 * built page only (the development server needs its own scripts).
 */
function contentSecurityPolicy(): Plugin {
  const relay = new URL(RELAY);
  const policy = [
    "default-src 'self'",
    "script-src 'self'",
    "style-src 'self'",
    "img-src 'self' data: blob:",
    `connect-src ${relay.protocol}//${relay.host}`,
    "media-src 'self' blob:",
    "worker-src 'self'",
    "manifest-src 'self'",
    "font-src 'self'",
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'none'",
  ].join("; ");
  return {
    name: "plenipo-remote-csp",
    apply: "build",
    transformIndexHtml(html) {
      return html.replace(
        "<!-- CSP -->",
        `<meta http-equiv="Content-Security-Policy" content="${policy}" />`,
      );
    },
  };
}

const DEFINE = {
  __PLENIPO_RELAY__: JSON.stringify(RELAY),
  __PLENIPO_VERSION__: JSON.stringify(VERSION),
};

/**
 * The page's background part (`sw.js`, part 14C): it shows notices when the page is closed. Built
 * on its own, as one plain script with no imports, after the page is built.
 */
function serviceWorker(): Plugin {
  let outDir = "dist";
  let root = process.cwd();
  return {
    name: "plenipo-remote-sw",
    apply: "build",
    configResolved(config) {
      outDir = config.build.outDir;
      root = config.root;
    },
    async closeBundle() {
      await build({
        configFile: false,
        root,
        logLevel: "warn",
        define: DEFINE,
        build: {
          outDir,
          emptyOutDir: false,
          target: "es2022",
          sourcemap: false,
          copyPublicDir: false,
          lib: {
            entry: "src/sw.ts",
            formats: ["iife"],
            name: "plenipoNotices",
            fileName: () => "sw.js",
          },
        },
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), contentSecurityPolicy(), serviceWorker()],
  clearScreen: false,
  define: DEFINE,
  server: {
    port: 8771,
    strictPort: true,
    host: "localhost",
  },
  build: {
    target: "es2022",
    sourcemap: false,
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    restoreMocks: true,
    testTimeout: 15_000,
  },
});
