// V1 — app-launch + IPC smoke (E2E). The unit suite stubs Electron entirely
// (tests/electron-stub.ts → ipcMain.handle = noop), so NOTHING exercises the real
// app boot: window creation, preload/contextBridge, IPC dispatch, renderer mount.
// This launches the ACTUAL built Electron app with CSA_SMOKE=1 and asserts the
// `CSA_SMOKE {...}` status line it prints — proving Electron 40 boots, the WASM
// core loads, migrations run, and a renderer→main→core IPC roundtrip works.
//
// Isolation: a throwaway --user-data-dir so the real csa-electron.db is untouched.
// Prereq: `electron-vite build` + `build:wasm` (the test:e2e script + CI do this).

import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

// No electron alias in vitest.e2e.config.ts → require("electron") returns the
// path to the real Electron binary (its Node entry exports the executable path).
// eslint-disable-next-line @typescript-eslint/no-require-imports
const electronPath = require("electron") as unknown as string;

const MAIN = resolve(__dirname, "..", "..", "out", "main", "index.js");

interface SmokeStatus {
  engine: string;
  db: { migrated: number; error?: string };
  lcu: string;
  ipc_ok: boolean;
}

let userDataDir: string | undefined;
afterEach(() => {
  if (userDataDir) rmSync(userDataDir, { recursive: true, force: true });
  userDataDir = undefined;
});

function launchSmoke(): { status: SmokeStatus; exitCode: number | null; raw: string } {
  if (!existsSync(MAIN)) {
    throw new Error(
      `out/main/index.js yok — önce build gerekli (pnpm --filter csa-desktop build:wasm && build). Beklenen: ${MAIN}`,
    );
  }
  userDataDir = mkdtempSync(join(tmpdir(), "csa-e2e-"));
  // CRITICAL: strip ELECTRON_RUN_AS_NODE. If it leaks into the env (the build
  // tooling / vitest can set it), the spawned Electron runs as plain Node and
  // `require("electron")` returns the binary path instead of the API → app is
  // undefined and the real main never boots. Removing it forces true GUI mode.
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    CSA_SMOKE: "1",
    ELECTRON_DISABLE_SECURITY_WARNINGS: "1",
  };
  delete env.ELECTRON_RUN_AS_NODE;
  const res = spawnSync(
    electronPath,
    [MAIN, `--user-data-dir=${userDataDir}`, "--no-sandbox"],
    { encoding: "utf8", timeout: 45_000, env },
  );
  const raw = `${res.stdout ?? ""}\n${res.stderr ?? ""}`;
  const line = raw.split(/\r?\n/).find((l) => l.includes("CSA_SMOKE "));
  if (!line) {
    throw new Error(
      `CSA_SMOKE satırı çıkmadı (signal=${res.signal}, status=${res.status}). Çıktı:\n${raw.slice(-2000)}`,
    );
  }
  const json = line.slice(line.indexOf("CSA_SMOKE ") + "CSA_SMOKE ".length).trim();
  return { status: JSON.parse(json) as SmokeStatus, exitCode: res.status, raw };
}

describe("app-launch smoke (E2E)", () => {
  it("boots Electron, loads the WASM core, migrates the DB, and serves an IPC roundtrip", () => {
    const { status, exitCode } = launchSmoke();

    // Çekirdek motor (core.wasm) yüklendi → recommendations/coaching mümkün.
    expect(status.engine).toBe("ready");
    // Migration'lar koştu (boş ama geçerli şema) — DB boot hatası yok.
    expect(status.db.error).toBeUndefined();
    expect(status.db.migrated).toBeGreaterThan(0);
    // Renderer→main→core gerçek IPC roundtrip (preload contextBridge + "cmd"
    // dispatcher + get_settings + DB) çalıştı.
    expect(status.ipc_ok).toBe(true);
    // app.quit() temiz çıkış (zombie process yok).
    expect(exitCode).toBe(0);
  });
});
