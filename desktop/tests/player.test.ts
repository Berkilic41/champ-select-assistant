// Player komutları — gerçek migration'lı DB ile getLearningProgress (Epic #4).

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { afterEach, describe, expect, it } from "vitest";

import { getLearningProgress } from "../src/main/commands/player";
import { openDatabase, runMigrations } from "../src/main/db";

const MIGRATIONS_DIR = join(__dirname, "..", "resources", "migrations");
const PUUID = "learn-puuid";

let dir: string | undefined;
let openDb: DatabaseSync | undefined;
afterEach(() => {
  try {
    openDb?.close();
  } catch {
    /* zaten kapalı */
  }
  openDb = undefined;
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function seededDb(): DatabaseSync {
  dir = mkdtempSync(join(tmpdir(), "csa-player-"));
  const db = openDatabase(join(dir, "app.db"));
  runMigrations(db, MIGRATIONS_DIR);
  openDb = db;

  db.prepare(
    "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES (?, 'L', 'TR', 'tr1', 0)",
  ).run(PUUID);
  const champ = db.prepare(
    "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (?, ?, ?, 't', 0)",
  );
  champ.run(86, "Garen", "Garen");
  champ.run(64, "LeeSin", "Lee Sin");
  champ.run(1, "Annie", "Annie");
  champ.run(99, "Lux", "Lux");
  return db;
}

const now = (): number => Math.floor(Date.now() / 1000);

describe("getLearningProgress (Epic #4 — öğrenme hedefi ilerlemesi)", () => {
  it("returns mastery gain only for 'learning'-marked champions, 0-gain included, sorted desc", () => {
    const db = seededDb();
    const t = now();
    const pref = db.prepare(
      "INSERT INTO user_preferences (puuid, champion_id, preference, created_at) VALUES (?, ?, ?, 0)",
    );
    pref.run(PUUID, 86, "learning"); // Garen — gain 500
    pref.run(PUUID, 64, "learning"); // LeeSin — gain 0 (hareket yok)
    pref.run(PUUID, 1, "never"); // Annie — hariç

    const snap = db.prepare(
      "INSERT INTO mastery_snapshots (puuid, champion_id, mastery_points, mastery_level, snapshot_at) VALUES (?, ?, ?, ?, ?)",
    );
    snap.run(PUUID, 86, 1000, 5, t - 86_400);
    snap.run(PUUID, 86, 1500, 6, t);
    snap.run(PUUID, 64, 2000, 7, t - 86_400);
    snap.run(PUUID, 64, 2000, 7, t);
    snap.run(PUUID, 1, 2500, 4, t - 86_400); // 'never' → hariç
    snap.run(PUUID, 1, 3000, 5, t);
    snap.run(PUUID, 99, 100, 2, t); // tercihsiz → hariç

    // Maç sonuçları (games/WR): Garen pencere-içi 4 maç (3 galibiyet) + 1 pencere-dışı (hariç).
    const m = db.prepare(
      "INSERT INTO matches (match_id, puuid, champion_id, position, win, kills, deaths, assists, duration_secs, queue_id, played_at) VALUES (?, ?, ?, 'MIDDLE', ?, 5, 2, 7, 1800, 420, ?)",
    );
    m.run("M1", PUUID, 86, 1, t - 1_000);
    m.run("M2", PUUID, 86, 1, t - 2_000);
    m.run("M3", PUUID, 86, 1, t - 3_000);
    m.run("M4", PUUID, 86, 0, t - 4_000);
    m.run("M5", PUUID, 86, 1, t - 40 * 86_400); // pencere-dışı → sayılmaz
    // LeeSin: hiç maç yok → games 0. Annie (1, 'never'): listede yok zaten.

    const res = getLearningProgress(db, PUUID, 30);
    expect(res).toHaveLength(2);
    // gain DESC: Garen (500) önce, LeeSin (0) sonra.
    expect(res[0].champion_key).toBe("Garen");
    expect(res[0].points_gained).toBe(500);
    expect(res[0].current_level).toBe(6);
    // Maç istatistiği: pencere-içi 4 maç, 3 galibiyet (pencere-dışı M5 hariç).
    expect(res[0].games_played).toBe(4);
    expect(res[0].wins).toBe(3);
    expect(res[1].champion_key).toBe("LeeSin");
    expect(res[1].points_gained).toBe(0);
    // Maçı olmayan hedef → games/wins 0 (gizlenmez, dürüst 0).
    expect(res[1].games_played).toBe(0);
    expect(res[1].wins).toBe(0);
    // 'never' ve tercihsiz şampiyonlar listede yok.
    expect(res.map((r) => r.champion_id)).not.toContain(1);
    expect(res.map((r) => r.champion_id)).not.toContain(99);
  });

  it("returns empty when there are no learning preferences", () => {
    const db = seededDb();
    expect(getLearningProgress(db, PUUID, 30)).toHaveLength(0);
  });
});
