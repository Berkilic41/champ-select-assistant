// Sprint 2B — match-outcome label (öneri → pick → win/loss).
//
// Akış (tamamı host/ana-süreçte, renderer'a bağlı değil):
//   1. get_recommendations her koştuğunda host son öneri listesini RecsCache'e yazar.
//   2. Champ-select'te oyuncu bir şampiyon commit edince (local_player.champion_id>0),
//      gameflow InProgress'e geçtiğinde pending bir recommendation_outcomes satırı
//      yazılır (rec_rank/rec_score öneri listesinden; liste-dışı pick'te NULL).
//   3. Oyun bitince (gameflow EndOfGame) maç geçmişi çekilir; resolve, o pick'i
//      `matches` tablosundaki gerçek sonuca (win/loss + match_id) bağlar.
//
// Bu satırlar YEREL ML eğitim etiketidir — sunucuya GİTMEZ (rıza gerektirmez).
// Determinist motor hâlâ tek doğruluk kaynağı; bu yalnız sonradan-etiketleme katmanı.

import type { DatabaseSync } from "node:sqlite";

import { championKeyById } from "../repos";

/** core draft_brain::DRAFT_BRAIN_RULES_VERSION paritesi (player.ts ile aynı sabit). */
const DRAFT_BRAIN_RULES_VERSION = "draft-brain-rules-v2";

// `matches.played_at = floor(gameCreation/1000)` = oyun başlangıcı (≈ InProgress'te
// yazılan pick.created_at). Pencere geniş tutulur (clock skew + stat işleme payı);
// puuid+champion+queue + EN YAKIN maç + "henüz bağlanmamış" kısıtı yanlış eşleşmeyi
// önler.
const RESOLVE_WINDOW_BEFORE_SECS = 2 * 3600;
const RESOLVE_WINDOW_AFTER_SECS = 6 * 3600;
/** Aynı (puuid,champion,queue) bekleyen pick'i bu pencerede tekrar yazma. */
const PICK_DEDUP_WINDOW_SECS = 6 * 3600;

/** Bir champ-select seansında commit edilen pick'in bağlamı. */
export interface PickContext {
  champion_id: number;
  queue_id: number;
  position: string;
  /** 3C: pick anındaki müttefik şampiyon id'leri (kendisi hariç) — co-pick
   *  combo geçmişi için (display-augment; scoring'e dokunmaz). */
  allies?: number[];
}

/** RecsCache içindeki tek bir önerinin sıralı görünümü. */
/** ModelPack lineer skorunu besleyen per-signal özellikler (0..1). FAZ 3 / 3B:
 *  öğrenilen ModelPack'in eğitim feature vektörü — pick anında saklanır. core
 *  apply_model_score'un kullandığı 7 sinyalle birebir. */
export interface ModelFeatures {
  comfort: number;
  matchup: number;
  team_counter: number;
  synergy: number;
  meta: number;
  role_fit: number;
  risk: number;
}

export interface CachedRec {
  champion_id: number;
  champion_key: string;
  score: number;
  rank: number;
  /** 3B: per-signal feature vektörü (öğrenilen ModelPack eğitimi için). */
  features: ModelFeatures;
}

/**
 * Host'un son hesapladığı öneri listesini tutar; `get_recommendations` her
 * çağrıldığında tazelenir. "Kilitlenen şampiyon öneri listesinin neresindeydi"
 * sorusunu yanıtlar — recommendation→pick linkage'in kaynağı. Yeni bir
 * champ-select başlayınca temizlenir (bayat seans verisi bir sonrakine sızmasın).
 */
export class RecsCache {
  private puuid = "";
  private ranked: CachedRec[] = [];

  /** Ham öneri objelerinden (core Recommendation) sıralı cache kur. Geçersiz
   *  giriş atlanır; rank geçerli öneriler arasında boşluksuz 1..N verilir. */
  set(puuid: string, recs: readonly unknown[]): void {
    this.puuid = puuid;
    const num = (o: Record<string, unknown>, key: string): number => {
      const v = Number(o?.[key]);
      return Number.isFinite(v) ? v : 0;
    };
    const out: CachedRec[] = [];
    for (const r of recs) {
      const o = r as Record<string, unknown>;
      const id = Number(o?.champion_id);
      if (!Number.isFinite(id) || id <= 0) continue;
      out.push({
        champion_id: id,
        champion_key:
          typeof o?.champion_key === "string" ? o.champion_key : String(id),
        score: Number(o?.total_score ?? 0),
        rank: out.length + 1,
        features: {
          comfort: num(o, "comfort_score"),
          matchup: num(o, "matchup_score"),
          team_counter: num(o, "team_counter_score"),
          synergy: num(o, "synergy_score"),
          meta: num(o, "meta_score"),
          role_fit: num(o, "role_fit_score"),
          risk: num(o, "risk_score"),
        },
      });
    }
    this.ranked = out;
  }

  clear(): void {
    this.puuid = "";
    this.ranked = [];
  }

  puuidValue(): string {
    return this.puuid;
  }

  isEmpty(): boolean {
    return this.ranked.length === 0;
  }

  /** Pick'in öneri listesindeki yeri; liste-dışı pick'te null. */
  lookup(championId: number): CachedRec | null {
    return this.ranked.find((r) => r.champion_id === championId) ?? null;
  }

  snapshot(): CachedRec[] {
    return this.ranked.map((r) => ({ ...r }));
  }
}

/**
 * Commit edilen pick için pending bir outcome satırı yaz.
 * ÖN KOŞUL: bu seansta öneri gösterilmiş olmalı (RecsCache dolu) — yoksa null
 * döner (etiketlenecek öneri yok; "öneri→pick→sonuç" zinciri kurulamaz).
 * Aynı (puuid,champion,queue) bekleyen satır son 6 saatte varsa tekrar yazılmaz.
 * @returns yeni satır id'si; atlandıysa null.
 */
export function recordRecommendationPick(
  db: DatabaseSync,
  recs: RecsCache,
  pick: PickContext,
  nowSecs: number = Math.floor(Date.now() / 1000),
): number | null {
  if (recs.isEmpty()) return null;
  const puuid = recs.puuidValue();
  if (!puuid) return null;
  if (!Number.isFinite(pick.champion_id) || pick.champion_id <= 0) return null;

  const hit = recs.lookup(pick.champion_id);
  const championKey =
    hit?.champion_key ??
    championKeyById(db, pick.champion_id) ??
    String(pick.champion_id);

  const dup = db
    .prepare(
      `SELECT id FROM recommendation_outcomes
        WHERE puuid = ? AND champion_id = ? AND queue_id = ?
          AND resolved_at IS NULL AND created_at >= ?`,
    )
    .get(puuid, pick.champion_id, pick.queue_id, nowSecs - PICK_DEDUP_WINDOW_SECS);
  if (dup) return null;

  const contextJson = JSON.stringify({
    recommended: recs.snapshot().map((r) => ({
      champion_id: r.champion_id,
      rank: r.rank,
      score: r.score,
    })),
    picked: pick.champion_id,
    // 3B: picked şampiyonun per-signal feature vektörü (öğrenilen ModelPack
    // eğitim örneği). Liste-dışı pick'te (hit yok) null → o satır eğitime girmez.
    features: hit?.features ?? null,
    // 3C: pick anındaki müttefik id'leri (co-pick combo geçmişi agregasyonu için).
    allies: pick.allies ?? [],
  });

  const result = db
    .prepare(
      `INSERT INTO recommendation_outcomes
         (puuid, champion_id, champion_key, queue_id, position, rec_rank, rec_score,
          model_version, context_json, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      puuid,
      pick.champion_id,
      championKey,
      pick.queue_id,
      pick.position || null,
      hit?.rank ?? null,
      hit?.score ?? null,
      DRAFT_BRAIN_RULES_VERSION,
      contextJson,
      nowSecs,
    );
  return Number(result.lastInsertRowid);
}

export interface ResolveResult {
  /** Bu çağrıda gerçek maça bağlanan pick sayısı. */
  resolved: number;
  /** Hâlâ bekleyen (eşleşen maç henüz ingest olmamış) pick sayısı. */
  pending: number;
}

/**
 * Bekleyen (resolved_at IS NULL) tüm pick'leri `matches` tablosuna bağla: aynı
 * puuid+champion+queue, played_at pencere içinde, henüz başka bir outcome'a
 * bağlanmamış EN YAKIN maç → win/loss + match_id + resolved_at yaz.
 * Idempotent + tamamen DB-only → her maç ingestion'ından sonra güvenle çağrılır.
 */
export function resolveRecommendationOutcomes(
  db: DatabaseSync,
  nowSecs: number = Math.floor(Date.now() / 1000),
): ResolveResult {
  const pending = db
    .prepare(
      `SELECT id, puuid, champion_id, queue_id, created_at
         FROM recommendation_outcomes WHERE resolved_at IS NULL`,
    )
    .all() as unknown as {
    id: number;
    puuid: string;
    champion_id: number;
    queue_id: number;
    created_at: number;
  }[];

  // "Henüz bağlanmamış" alt-sorgu döngü içinde her UPDATE'ten sonra güncel görülür
  // (node:sqlite senkron) → aynı maç iki pick'e bağlanamaz.
  const findMatch = db.prepare(
    `SELECT m.match_id AS match_id, m.win AS win
       FROM matches m
      WHERE m.puuid = ? AND m.champion_id = ? AND m.queue_id = ?
        AND m.played_at >= ? AND m.played_at <= ?
        AND m.match_id NOT IN (
          SELECT match_id FROM recommendation_outcomes WHERE match_id IS NOT NULL
        )
      ORDER BY ABS(m.played_at - ?) ASC
      LIMIT 1`,
  );
  const resolveRow = db.prepare(
    `UPDATE recommendation_outcomes
        SET match_id = ?, win = ?, resolved_at = ? WHERE id = ?`,
  );

  let resolved = 0;
  for (const row of pending) {
    const match = findMatch.get(
      row.puuid,
      row.champion_id,
      row.queue_id,
      row.created_at - RESOLVE_WINDOW_BEFORE_SECS,
      row.created_at + RESOLVE_WINDOW_AFTER_SECS,
      row.created_at,
    ) as { match_id: string; win: number } | undefined;
    if (!match) continue;
    resolveRow.run(match.match_id, match.win, nowSecs, row.id);
    resolved += 1;
  }
  return { resolved, pending: pending.length - resolved };
}

const IN_GAME_PHASES = new Set(["GameStart", "InProgress"]);
const POSTGAME_PHASES = new Set(["WaitingForStats", "PreEndOfGame", "EndOfGame"]);

interface ParsedTeamSlot {
  champion_id?: number;
  assigned_position?: string;
}
interface ParsedSession {
  queue_id?: number;
  local_player?: ParsedTeamSlot;
  my_team?: ParsedTeamSlot[];
}

export interface OutcomeTrackerDeps {
  /** DB lazily (index.ts'te boot sonrası atanır); yoksa no-op. */
  getDb: () => DatabaseSync | undefined;
  recs: RecsCache;
  /** Oyun-sonu maç çekimi (best-effort; içinde resolve da koşmalı). */
  syncMatches: () => Promise<void>;
}

/**
 * Champ-select + gameflow event akışından öneri→pick→sonuç köprüsünü kurar.
 *   - yeni ChampSelect : seansı sıfırla + RecsCache temizle.
 *   - champ-select session: commit edilen (champion_id>0) son pick'i hatırla.
 *   - InProgress       : pending outcome satırı yaz (seans başına bir kez).
 *   - EndOfGame        : maç geçmişini çek → resolve (player-data sync'i de self-heal eder).
 */
export class OutcomeTracker {
  private lastCommitted: PickContext | null = null;
  private pickRecorded = false;
  private postgameHandled = false;

  constructor(private readonly deps: OutcomeTrackerDeps) {}

  /** Parse edilmiş ChampSelectState (snake_case) veya null (seans bitti). */
  onChampSelectSession(state: unknown | null): void {
    if (state === null || typeof state !== "object") return;
    const s = state as ParsedSession;
    const slot = s.local_player;
    if (!slot) return;
    const championId = Number(slot.champion_id);
    if (!Number.isFinite(championId) || championId <= 0) return; // commit yok (hover/ban)
    // 3C: müttefik şampiyon id'leri (commit edilenler, kendisi hariç).
    const allies = (Array.isArray(s.my_team) ? s.my_team : [])
      .map((t) => Number(t?.champion_id))
      .filter((id) => Number.isFinite(id) && id > 0 && id !== championId);
    this.lastCommitted = {
      champion_id: championId,
      queue_id: Number(s.queue_id ?? 0),
      position:
        typeof slot.assigned_position === "string" ? slot.assigned_position : "",
      allies,
    };
  }

  onGameflowPhase(phase: string): void {
    if (phase === "ChampSelect") {
      this.lastCommitted = null;
      this.pickRecorded = false;
      this.postgameHandled = false;
      this.deps.recs.clear();
      return;
    }
    if (IN_GAME_PHASES.has(phase)) {
      this.postgameHandled = false;
      if (this.pickRecorded || !this.lastCommitted) return;
      const db = this.deps.getDb();
      if (!db) return;
      try {
        recordRecommendationPick(db, this.deps.recs, this.lastCommitted);
      } catch (err) {
        console.warn("recordRecommendationPick hatası:", (err as Error).message);
      }
      this.pickRecorded = true;
      return;
    }
    if (POSTGAME_PHASES.has(phase)) {
      if (this.postgameHandled) return;
      this.postgameHandled = true;
      void this.deps.syncMatches().catch((err) => {
        console.warn("oyun-sonu maç sync hatası:", (err as Error).message);
      });
    }
  }
}
