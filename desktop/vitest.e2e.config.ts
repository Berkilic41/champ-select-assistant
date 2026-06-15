import { defineConfig } from "vitest/config";

// E2E config — SEPARATE from vitest.config.ts on purpose. The unit config aliases
// `electron` to a Node stub; the E2E test must instead spawn the REAL Electron
// binary (require("electron") returns its path), so this config has NO alias and
// only picks up tests/e2e/**/*.e2e.ts. Run via `pnpm test:e2e` (builds first).
// Electron launch + renderer load is slow → generous timeouts, single fork.
export default defineConfig({
  test: {
    environment: "node",
    include: ["tests/e2e/**/*.e2e.ts"],
    testTimeout: 60_000,
    hookTimeout: 60_000,
    pool: "forks",
    fileParallelism: false,
  },
});
