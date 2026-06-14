import { defineConfig } from "vitest/config";

// Worker unit tests run in plain Node (mocked fetch + fake D1); they exercise
// pure helpers, the HTTP surface, auth gating, backoff, and per-region fairness.
export default defineConfig({
  test: {
    include: ["test/**/*.test.ts"],
    environment: "node",
  },
});
