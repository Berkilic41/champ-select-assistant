# Data Pipeline — Ingestion Contract + Last-Good Policy (Audit)

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis) · Kapsam: aggregation çıktısını **DB-upsert-ready canonical
> row**'lara çeviren saf kontrat + **last-good cache promotion/rollback** saf politikası. Hot UI/command
> dosyalarına dokunulmadı. Network/DB/Riot yok. Veri uydurma yok. Codex sonra raw fetch + upsert + cache'e bağlar.
> Tamamlayıcı: [match-v5-aggregation-core.md](match-v5-aggregation-core.md).

## 1. Modül
`recommendation/ingestion_contract.rs` (saf, `#[allow(dead_code)]` bağlanana dek).

## 2. Canonical rows (DB-upsert-ready, ts-rs)
`to_canonical_rows(&AggregationResult, region) -> CanonicalRowSet`. Superset — Codex client (SQLite)
veya backend (Postgres) hedefine map'ler:
- `CanonicalRateRow { region, patch, champion_id, position, win_rate, pick_rate, ban_rate, sample_size, source, confidence }`
- `CanonicalMatchupRow { region, patch, champion_id, opponent_id, position, games, wins, win_rate, sample_size, source, confidence }`
- `CanonicalBuildRow { region, patch, champion_id, position, item_ids[], rune_ids[], summoner_spells[], games, win_rate, pick_rate, sample_size, source, confidence }`
- `CanonicalRowSet { region, rates, matchups, builds }`

Kurallar:
- `source`/`patch`/`confidence`/`sample_size` agregadan **korunur**.
- `ban_rate = 0.0` (basitleştirilmiş Match-V5 input'ta ban yok → dürüst; ileride `teams.bans` ile genişler).
- Build `pick_rate` aynı champ/pos/patch'in **rate satırından join** edilir.
- `item_ids` = core + situational (öncelik sırası).
- **Şema notu:** client `champion_rates` UNIQUE(champ,pos,source); `champion_matchups`/`builds`'te confidence
  kolonu yok (Codex upsert'te confidence'ı taşıyabilir/atlayabilir). `region` client'ta yok, backend'de var.

## 3. Last-good cache policy (saf)
`decide_cache_promotion(candidate, current_good?) -> CachePromotionDecision { action, promoted, reason }`.
`action` ∈ `promote` | `keep_current` | `reject`. Öncelik:
1. **Yüksek riskli kaynak** → cache'in üstüne **asla promote edilmez**: cache varsa `keep_current`, yoksa `reject`.
2. **Boş/yetersiz aday** (coverage 0 veya sample 0) → cache varsa `keep_current`, yoksa `reject`.
3. Cache yok + aday geçerli → `promote` (ilk iyi).
4. **Regresyon koruması:** aday kapsaması mevcut cache'ten `0.10`'dan fazla düşük → `keep_current`.
5. Aday bayat, cache taze → `keep_current`.
6. Aksi halde → `promote`.

> No fabrication: kullanılamaz aday + cache yok = `reject` (insufficient), sahte promote değil.

## 4. Test matrisi (12 — hepsi geçti)
**Canonical/fixture:** raw ranked Match-V5 → aggregate → canonical (source/patch/confidence/region korunur,
ban_rate 0) · ARAM skip (boş canonical) · **patch isolation** (14.11 + 14.10 → ayrı satırlar) · build item +
join'lenmiş pick_rate · **emitted token vocabulary lock** (confidence∈{high,medium,low}, source, cache action).
**Last-good policy:** ilk-iyi promote · daha-iyi/eşit promote · high-risk keep_current · high-risk-no-cache
reject · coverage regresyon keep_current · bayat aday keep_current · boş aday keep_current/reject.

## 5. ts-rs + contract
5 DTO üretildi (CanonicalRateRow/MatchupRow/BuildRow/RowSet + CachePromotionDecision; hepsi number/string/
bool/array — **bigint yok**). TS contract + drift guard: `src/types/ingestion-contract.contract.test.ts`
(canonical shape'ler exhaustive `keyof` + cache action token vocabulary'si kilitli).

Baseline: cargo test **444** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **179/45**.

## 6. Codex'e (sonraki büyük iş — actual fetch/upsert)
1. **Riot Match-V5 fetcher** (key/proxy hazır olunca) → `MatchV5` listesi.
2. `aggregate_matches` → `to_canonical_rows(result, region)` → `CanonicalRowSet`.
3. **SQLite/backend upsert:** rates → `champion_rates` (UNIQUE champ,pos,source), matchups →
   `champion_matchups`, builds → `builds` (item_ids/rune_ids TEXT'e serialize). `cached_at` = now.
4. **Last-good cache:** fetch sonrası `decide_cache_promotion` ile promote/rollback; `promoted=false` ise
   eski cache korunur (pipeline `use_last_good_cache` action'ı bu davranışa bağlanır).
5. Opsiyonel **background scheduler:** pipeline `actions`'ı periyodik/manuel tetikler. **Champ-select'te network yok.**

> Sıra: Claude saf aggregation + canonical kontrat + cache policy'yi kurdu; Codex raw fetch + upsert + cache
> promote/rollback + scheduler'a bağlar.
