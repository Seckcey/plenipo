import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test-setup.ts"],
    restoreMocks: true,
    // Slow Windows CI runners can take several times longer than a laptop.
    testTimeout: 15_000,
  },
});
