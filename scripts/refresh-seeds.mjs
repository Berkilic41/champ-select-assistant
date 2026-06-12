// Seed tazeleme aracı (Faz A3) — yerel DB'deki son u.gg sync'inden seed
// ADAYLARI üretir. Mevcut seed dosyalarının ÜZERİNE YAZMAZ: çıktılar
// *.candidate.json'dur; küratör diff özetini inceleyip uygun bulursa adayı
// elle seed dosyasının yerine koyar (release checklist adımı).
//
// Kullanım:
//   node scripts/refresh-seeds.mjs [--db <yol>] [--min-games <n>]
//
// Varsayılan DB: %APPDATA%/csa-desktop/csa-electron.db (salt-okunur açılır).
// Kaynak dürüstlüğü: aday satırlar source="u_gg" taşır (manual_seed DEĞİL) —
// import sonrası canlı u.gg sync'i aynı conflict anahtarıyla üstüne yazar.

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SEEDS_DIR = join(ROOT, "desktop", "resources", "seeds");

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
}

const dbPath = arg(
  "db",
  join(process.env.APPDATA ?? "", "csa-desktop", "csa-electron.db"),
);
const minGames = Number(arg("min-games", "1000"));

if (!existsSync(dbPath)) {
  console.error(`DB bulunamadı: ${dbPath} (önce uygulamayı çalıştırıp sync bekleyin)`);
  process.exit(1);
}

const db = new DatabaseSync(dbPath, { readOnly: true });

// Baskın u_gg patch'i (en çok satırı olan) — karışık patch'leri adaya sokma.
const patchRow = db
  .prepare(
    `SELECT patch_version AS p, COUNT(*) AS c FROM builds
     WHERE source = 'u_gg' GROUP BY patch_version ORDER BY c DESC LIMIT 1`,
  )
  .get();
if (!patchRow) {
  console.error("DB'de u_gg build satırı yok — u.gg sync'i henüz koşmamış.");
  process.exit(1);
}
const patch = String(patchRow.p);

const jsonArr = (s) => {
  try {
    const v = JSON.parse(String(s ?? "[]"));
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
};

// ── builds adayı ──────────────────────────────────────────────────────────────
const buildRows = db
  .prepare(
    `SELECT champion_id, position, patch_version, item_ids, rune_ids, win_rate,
            pick_rate, skill_order, summoner_spells, secondary_runes, stat_shards,
            games
     FROM builds
     WHERE source = 'u_gg' AND patch_version = ? AND games >= ?
     ORDER BY champion_id, position`,
  )
  .all(patch, minGames);

const buildCandidates = buildRows.map((r) => ({
  champion_id: Number(r.champion_id),
  position: String(r.position),
  patch_version: String(r.patch_version),
  item_ids: jsonArr(r.item_ids),
  rune_ids: jsonArr(r.rune_ids),
  win_rate: Number(r.win_rate),
  pick_rate: Number(r.pick_rate),
  source: "u_gg",
  opponent_archetype: null,
  skill_order: r.skill_order === null ? null : String(r.skill_order),
  summoner_spells: jsonArr(r.summoner_spells),
  secondary_runes: jsonArr(r.secondary_runes),
  stat_shards: jsonArr(r.stat_shards),
}));

// ── matchup adayı ─────────────────────────────────────────────────────────────
const matchupRows = db
  .prepare(
    `SELECT champion_id, opponent_id, position, games, wins, win_rate, patch_version
     FROM champion_matchups
     WHERE source = 'u_gg' AND patch_version = ? AND games >= ?
     ORDER BY champion_id, opponent_id`,
  )
  .all(patch, minGames);

const matchupCandidates = matchupRows.map((r) => ({
  champion_id: Number(r.champion_id),
  opponent_id: Number(r.opponent_id),
  position: String(r.position),
  games: Number(r.games),
  wins: Number(r.wins),
  win_rate: Number(r.win_rate),
  patch_version: String(r.patch_version),
  source: "u_gg",
}));

db.close();

// ── küratör diff özeti ────────────────────────────────────────────────────────
function summarize(name, currentPath, candidates, keyFn) {
  let current = [];
  try {
    current = JSON.parse(readFileSync(currentPath, "utf8"));
  } catch {
    /* seed yoksa boş kabul */
  }
  const curKeys = new Set(current.map(keyFn));
  const candKeys = new Set(candidates.map(keyFn));
  const added = [...candKeys].filter((k) => !curKeys.has(k)).length;
  const removed = [...curKeys].filter((k) => !candKeys.has(k)).length;
  const curPatches = [...new Set(current.map((e) => e.patch_version))].join(",");
  console.log(
    `${name}: mevcut ${current.length} satır (patch ${curPatches || "-"}) → aday ` +
      `${candidates.length} satır (patch ${patch}) | +${added} yeni anahtar, ` +
      `-${removed} kalkan anahtar`,
  );
}

const buildsOut = join(SEEDS_DIR, "builds_seed.candidate.json");
const matchupsOut = join(SEEDS_DIR, "meta", "matchup_seed.candidate.json");
writeFileSync(buildsOut, JSON.stringify(buildCandidates, null, 1) + "\n", "utf8");
writeFileSync(matchupsOut, JSON.stringify(matchupCandidates, null, 1) + "\n", "utf8");

summarize(
  "builds",
  join(SEEDS_DIR, "builds_seed.json"),
  buildCandidates,
  (e) => `${e.champion_id}:${e.position}:${e.opponent_archetype ?? ""}`,
);
summarize(
  "matchups",
  join(SEEDS_DIR, "meta", "matchup_seed.json"),
  matchupCandidates,
  (e) => `${e.champion_id}:${e.opponent_id}:${e.position}`,
);
console.log(`Adaylar yazıldı:\n  ${buildsOut}\n  ${matchupsOut}`);
console.log(
  "Küratör adımı: adayları incele; uygunsa seed dosyasının yerine koy " +
    "(candidate dosyaları repoya COMMIT'LENMEZ).",
);
