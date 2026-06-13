import { resolve } from "node:path";

import { defineConfig } from "vitest/config";

// vitest does NOT auto-load electron.vite.config.ts (that drives electron-vite
// builds, not tests). This config scopes desktop tests to tests/ and aliases
// `electron` to a Node stub so main-process imports resolve without an Electron
// runtime (CI is plain Node). The WASM core (../core/pkg) must be built first
// via `pnpm build:wasm` — engine.test.ts loads it directly.
export default defineConfig({
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
    alias: {
      electron: resolve(__dirname, "tests/electron-stub.ts"),
    },
  },
});
