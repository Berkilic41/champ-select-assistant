# Match Discovery Planner Core v1 — Audit

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis) · Kapsam: "hangi oyuncu-hash crawl edilsin ve
> hangi match id aday havuzuna girsin?" kararını veren **saf motor** — Match-V5 veri hacmini aktif
> oyuncunun 20 maçının ötesine, katılımcı PUUID'lerinden keşifle büyütür. DB/network/Riot/command/UI YOK.
> **PII-safe by construction:** modül yalnız `puuid_hash` görür/döndürür (hash↔ham-PUUID map runtime'da).
> Veri uydurma yok (seed yoksa crawl yok, aday yoksa yeni id yok). `match_fetch_planner`'ın **upstream**'i —
> duplicate değil. Tamamlayıcı: [match-fetch-planner-core.md](match-fetch-planner-core.md) ·
> [coverage-expansion-policy-core.md](coverage-expansion-policy-core.md).

## 1. Modül
`recommendation/match_discovery_planner.rs` (saf, `#[allow(dead_code)]` crawl/fetch-history wiring'e bağlanana dek).
Giriş: `plan_match_discovery(&MatchDiscoveryInput) -> MatchDiscoveryPlan`.

## 2. DTO'lar
**Input (Rust-only — timestamp'ları i64 taşır, export edilmez):**
`DiscoverySeed` (puuid_hash/region/source/seen_at/contribution_count) ·
`CrawledPlayerRecord` (puuid_hash/region/last_crawled_at/crawl_count) ·
`DiscoveredMatchCandidate` (match_id/region/source_puuid_hash/discovered_at) ·
`KnownMatchRecord` (match_id/region/status) ·
`MatchDiscoveryInput` (now/champ_select_active/crawl_budget/max_breadth/per_player_match_cap/seeds/
crawled_players/candidate_matches/known_matches).

**Output (ts-rs, bigint yok — yalnız `*_hash`, ham PUUID yok):**
`PlayerCrawlDecision` (puuid_hash/region/decision/reason/priority) ·
`MatchDiscoveryDecision` (match_id/region/decision/reason) ·
`MatchDiscoveryPlan` (to_crawl/new_match_ids/player_decisions/match_decisions/selected_crawl_count/
new_match_count/skipped_count).

## 3. Token vocabulary (sabit `pub const`)
- **PLAYER_DECISIONS [6]:** `crawl` · `skip_already_crawled` · `skip_champ_select` · `skip_budget` ·
  `skip_breadth_full` · `skip_invalid`
- **MATCH_DECISIONS [4]:** `new` · `skip_known` · `skip_invalid` · `skip_player_cap`

## 4. Karar mantığı
- **Champ-select guard:** `champ_select_active=true` → tüm seed `skip_champ_select`, `to_crawl` boş —
  **champ-select'te network yok** (player crawl tamamen ertelenir). Match aday intake yine çalışır (saf, network değil).
- **Player crawl (champ-select kapalıyken):** boş hash → `skip_invalid`; `crawled_players`'ta hash → `skip_already_crawled`;
  kalanlar eligible.
- **Priority sıralaması (deterministik):** `source_rank` desc (active_player=3 → match_participant=2 → manual_seed=1 → 0)
  → `contribution_count` desc → `seen_at` desc → `puuid_hash` asc.
- **Cap:** `cap = min(crawl_budget, max_breadth)` (budget 0 → cap 0, tüm eligible `skip_budget`).
  index < cap → `crawl`; index ≥ max_breadth → `skip_breadth_full`; arada → `skip_budget`.
- **Match aday intake:** boş match_id → `skip_invalid`; `known_matches`'te id → `skip_known`
  (**failed bile known sayılır** — detail retry fetch planner/history katmanında); aynı `source_puuid_hash` için
  `per_player_match_cap` üstü → `skip_player_cap`; kalan → `new`.
  Aday sıralaması: `source_puuid_hash` asc → `discovered_at` desc → `match_id` asc (cap'in deterministik kimi keseceğini sabitler).
- **Çıktı sıralaması:** player_decisions priority desc → puuid_hash asc; match_decisions match_id asc.
- **Sayılar:** `selected_crawl_count=to_crawl.len`, `new_match_count=new_match_ids.len`,
  `skipped_count = (seeds + candidates) − selected − new` (her giriş tam bir karara map'lenir).
- **No fabrication:** seed yoksa crawl yok; aday yoksa yeni id yok. **PII yok:** yalnız `puuid_hash`.

## 5. Test matrisi (17 — hepsi geçti)
champ-select → tüm seed skip_champ_select + to_crawl boş · already-crawled skip · budget=0 → skip_budget ·
breadth cap → skip_breadth_full · active_player priority en yüksek · contribution_count tie-break ·
deterministik tie-break (seen_at/hash) · boş seed → invalid · known dedup (**failed dahil**) · boş match_id → invalid ·
per_player_match_cap → skip_player_cap · count tutarlılığı (selected+new+skipped = total) ·
**token vocab lock** (PLAYER_DECISIONS/MATCH_DECISIONS) · **PII guard** (`output_exposes_only_hashes_no_raw_puuid` —
serde_json key denetimi: "puuid_hash" var, bare "puuid"/"summoner"/"name" yok).

## 6. ts-rs + contract
`PlayerCrawlDecision` · `MatchDiscoveryDecision` · `MatchDiscoveryPlan` üretildi (**bigint yok** —
priority/count'lar `number`). TS contract guard: `src/types/match-discovery-planner.contract.test.ts`
(exhaustive `Record<keyof T, true>` shape lock + decision token vocab + PII guard: key listesi `puuid_hash` içerir,
`puuid`/`summoner_name` içermez).

Baseline: cargo test **530** · gate clippy `-D warnings` exit 0 · fmt-all temiz · pnpm typecheck pass · vitest **189/49**.

## 7. i18n token vocab + drift guard (⏳ Codex i18n'i ekleyince bağlanacak)
`dataPipeline.matchDiscovery.*` i18n henüz YOK → şimdilik Rust `pub const` vocab-lock (test'te) guard'ı.
include_str!(tr.json) drift guard'ı **eklenmedi** (boş key'lere kırmızı olur, yeşil baseline kırılır) —
coverage_expansion deseni: Codex tr/en'e ekleyip REQUIRED_KEYS'e kilitledikten sonra Claude guard'ı bağlar.

**1. Codex tr.json + en.json'a ekler (her ikisi, i18n-parity REQUIRED_KEYS'e):**
```json
// tr.json — dataPipeline.matchDiscovery
"matchDiscovery": {
  "player": { "crawl": "Crawl edilecek", "skip_already_crawled": "Zaten crawl'landı",
              "skip_champ_select": "Champ-select aktif (ertelendi)", "skip_budget": "Bütçe yetersiz",
              "skip_breadth_full": "Breadth limiti doldu", "skip_invalid": "Geçersiz hash" },
  "match": { "new": "Yeni aday", "skip_known": "Zaten bilinen", "skip_invalid": "Geçersiz match id",
             "skip_player_cap": "Oyuncu aday limiti doldu" }
}
// en.json
"matchDiscovery": {
  "player": { "crawl": "Will crawl", "skip_already_crawled": "Already crawled",
              "skip_champ_select": "Champ-select active (deferred)", "skip_budget": "Budget exhausted",
              "skip_breadth_full": "Breadth limit reached", "skip_invalid": "Invalid hash" },
  "match": { "new": "New candidate", "skip_known": "Already known", "skip_invalid": "Invalid match id",
             "skip_player_cap": "Per-player cap reached" }
}
```

**2. Claude drift guard'ı bağlar** (`match_discovery_planner.rs` testine — i18n geldiğinde):
```rust
#[test]
fn every_emitted_discovery_token_has_an_i18n_label() {
    const TR: &str = include_str!("../../../src/i18n/tr.json");
    let tr: serde_json::Value = serde_json::from_str(TR).unwrap();
    let md = &tr["dataPipeline"]["matchDiscovery"];
    for d in PLAYER_DECISIONS { assert!(!md["player"][d].is_null(), "player decision '{d}' i18n yok"); }
    for d in MATCH_DECISIONS  { assert!(!md["match"][d].is_null(),  "match decision '{d}' i18n yok"); }
}
```
en parity TS `i18n tr/en parity` testinde.

## 8. Codex'e (runtime binding)
1. **match_discovery_players / crawl-history migration:** crawl'lanmış oyuncuları + keşfedilen match adaylarını
   kalıcılaştır (puuid_hash kolonu — **ham PUUID kolonu YOK**). Aday havuzu mevcut `match_v5_fetch_history`'e beslenir.
2. **hash↔ham PUUID map yalnız local:** participant PUUID'leri hash'le (tek yönlü, local map); saf motora **yalnız hash** ver.
3. **Seed üretimi:** active player → `source="active_player"`; çekilen maçların katılımcıları → `source="match_participant"`;
   manuel → `source="manual_seed"`. `contribution_count` = o hash'in kaç maça katkıda bulunduğu.
4. `plan_match_discovery(input)` → `to_crawl` hash'leri için her oyuncunun match listesini çek → katılımcıları yeni seed yap
   (genişleme); `new_match_ids` → `match_v5_fetch_history` aday havuzuna ekle.
5. **Zincir:** discovery → `match_fetch_planner` (batch/öncelik) → detail fetch → `match_v5_mapper` → `match_v5_aggregator`
   → `ingestion_contract` (canonical rows) → SQLite/backend upsert.
6. `champ_select_active` runtime state'ten — true iken crawl ertelenir (network yok); aday intake saf, çalışmaya devam eder.

> Sıra: Claude saf crawl-seçim + aday-intake + dedup + per-player cap + no-PII/no-fabrication motorunu kurdu
> (fixture-test'li, key/network/timer-bağımsız); Codex hash'leme + persistence + Riot fetch + plan→fetch binding'e bağlar.
> Bu blok veri hacmini **kontrollü** büyütür: champ-select'te durur, tek oyuncuya aşırı yüklenmez (cap), tekrar etmez (dedup).
