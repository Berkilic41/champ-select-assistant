// Performans baseline'ı (Faz A8) — iki ölçüm + kalıcı kayıt:
//   1. Cold start: CSA_SMOKE=1 Electron boot'unun duvar saati süresi
//      (spawn → CSA_SMOKE satırı; DB+migrations+WASM+pencere dahil).
//   2. Öneri gecikmesi: GERÇEK csa-core WASM'ına parity fixture'ıyla N çağrı
//      → p50/p95. IPC köprüsü DAHİL DEĞİL (motor payı; <500ms bütçesinin
//      baskın kalemi) — dürüst not olarak kayda düşülür.
//
// RAM ölçümü ayrı: uygulama açıkken
//   pwsh scripts/benchmark/measure-process.ps1 -Name csa-desktop
//
// Kullanım: node scripts/benchmark/baseline.mjs [--runs <n>]
// Çıktı: scripts/benchmark/baseline-<tarih>.json + konsol tablosu.

import { spawn } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const runs = Number(
  process.argv.includes("--runs")
    ? process.argv[process.argv.indexOf("--runs") + 1]
    : "50",
);

// ── 1. Öneri gecikmesi (gerçek WASM, IPC'siz) ────────────────────────────────
const core = await import(
  new URL("file://" + join(ROOT, "core", "pkg", "csa_core.js")).href
).then((m) => m.default ?? m);
const fixture = readFileSync(
  join(ROOT, "core", "tests", "fixtures", "json_api_recommendations_input.json"),
  "utf8",
);

// Isınma (WASM JIT + KB lazy-load fixture dışında kalmasın).
for (let i = 0; i < 5; i++) core.recommendations_json(fixture);

const samples = [];
for (let i = 0; i < runs; i++) {
  const t0 = performance.now();
  core.recommendations_json(fixture);
  samples.push(performance.now() - t0);
}
samples.sort((a, b) => a - b);
const pct = (p) => samples[Math.min(samples.length - 1, Math.floor((p / 100) * samples.length))];
const recLatency = {
  n: runs,
  p50_ms: Number(pct(50).toFixed(2)),
  p95_ms: Number(pct(95).toFixed(2)),
  max_ms: Number(samples[samples.length - 1].toFixed(2)),
  note: "motor-yalnız (WASM çağrısı); IPC + DB okuma payı dahil değil",
};

// ── 2. Cold start (CSA_SMOKE boot) ───────────────────────────────────────────
function measureColdStart() {
  return new Promise((resolve) => {
    const t0 = performance.now();
    const child = spawn("pnpm", ["exec", "electron", "."], {
      cwd: join(ROOT, "desktop"),
      env: { ...process.env, CSA_SMOKE: "1" },
      shell: true,
    });
    let out = "";
    const finish = (ms, status) => resolve({ ms, status });
    const timer = setTimeout(() => {
      child.kill();
      finish(null, "timeout-60s");
    }, 60_000);
    child.stdout.on("data", (d) => {
      out += String(d);
      if (out.includes("CSA_SMOKE")) {
        clearTimeout(timer);
        const ms = Math.round(performance.now() - t0);
        child.on("exit", () => finish(ms, "ok"));
      }
    });
    child.on("error", () => {
      clearTimeout(timer);
      finish(null, "spawn-error");
    });
  });
}

const cold = await measureColdStart();

// ── Kayıt + tablo ─────────────────────────────────────────────────────────────
const result = {
  measured_at: new Date().toISOString(),
  cold_start_ms: cold.ms,
  cold_start_status: cold.status,
  rec_latency: recLatency,
};
const stamp = result.measured_at.slice(0, 10);
const outPath = join(ROOT, "scripts", "benchmark", `baseline-${stamp}.json`);
writeFileSync(outPath, JSON.stringify(result, null, 2) + "\n", "utf8");

console.log("== Performans baseline ==");
console.log(`Cold start (CSA_SMOKE boot) : ${cold.ms ?? "-"} ms (${cold.status})`);
console.log(
  `Öneri gecikmesi (motor)     : p50 ${recLatency.p50_ms} ms · p95 ${recLatency.p95_ms} ms · max ${recLatency.max_ms} ms (n=${runs})`,
);
console.log(`Kayıt: ${outPath}`);
console.log(
  "RAM için (uygulama açıkken): pwsh scripts/benchmark/measure-process.ps1 -Name csa-desktop",
);
