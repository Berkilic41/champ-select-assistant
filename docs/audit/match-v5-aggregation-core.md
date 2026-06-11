# Match-V5 Aggregation Core v1 — Audit

> Tarih: 2026-06-06 · Sahip: Claude (2. mühendis) · Kapsam: Riot Match-V5 maç batch'ini agregat champion
> rate / lane matchup / build'e çeviren **saf motor**. Hot UI yok · backend deploy yok · network yok · Riot
> key yok · DB yazma yok. Veri uydurma yok: agregalanacak veri yoksa boş çıktı + warning. Codex sonra
> gerçek fetcher → bu aggregator → DB upsert diye bağlar (paralel kurulan iş, key beklemeden).

## 1. Modül
`recommendation/match_v5_aggregator.rs` (saf, `#[allow(dead_code)]` bağlanana dek).
Giriş: `aggregate_matches(&[MatchV5]) -> AggregationResult`.

## 2. Input (simplified Match-V5)
`MatchV5 { match_id, queue_id, patch, participants[] }` ·
`MatchV5Participant { participant_id, champion_id, team_id, team_position, win, k/d/a, items[], runes[], summoner_spells[] }`.
`patch` caller'da game_version'dan normalize edilir (örn. "14.11").

## 3. Output (ts-rs export'lu)
- `AggregatedChampionRate { champion_id, position, games, wins, win_rate, pick_rate, sample_size, patch, source, confidence }`
- `AggregatedMatchup { champion_id, opponent_id, position, games, wins, win_rate, patch, source, confidence }`
- `AggregatedBuild { champion_id, position, patch, core_items, situational_items, rune_ids, summoner_spells, games, win_rate, source, confidence }`
- `AggregationQuality { match_count, champion_rate_count, matchup_count, build_count, skipped_matches, warnings }`
- `AggregationResult { rates, matchups, builds, quality }`

`source` her zaman `"riot_match_v5"`. **Per-patch keyed** ((champ, position, patch)) → cross-patch batch karışmaz.

## 4. Kurallar (uygulandı)
- **Sample confidence:** ≥100 high · ≥30 medium · else low.
- **Matchup:** sadece aynı `team_position`, **karşı takım**; mirror (aynı champ) yok, aynı takım yok. Her iki
  yön (A→B + B→A) kaydedilir. Bir pozisyonda tam 2 oyuncu yoksa o lane atlanır.
- **ARAM (450) / Arena (1700):** skip + warning (`aram_skipped`/`arena_skipped`). Pozisyonsuz participant atlanır.
- **Build item filtresi:** item 0 (boş) + trinket'ler (3340/3363/3364/3330) elenir. **core_items** = en sık ilk 3
  geçerli item; **situational** = sonraki 3; **rune_ids** = en sık 6; **summoner_spells** = en sık (sıralı) çift.
- **Pick rate denominator açık:** role-bazlı toplam participant sayısı (aynı patch) → `pick_rate = champ_games / role_total`.
- **No fabrication:** veri yoksa boş çıktı; tümü skip ise `no_aggregatable_data` warning.
- **Deterministik sıralama:** rates (champion_id, position) · matchups (champion_id, position, opponent_id) · builds (champion_id, position).

## 5. Test matrisi (8 — hepsi geçti)
| Test | Sonuç |
|---|---|
| 2 maçtan champion rate (games=2, wr doğru) | ✓ |
| aynı rol rakip matchup (A↔B her iki yön) | ✓ |
| aynı takım matchup ÜRETMEZ | ✓ |
| low sample → confidence low | ✓ |
| build core item = en sık non-zero (0 + trinket elendi) | ✓ |
| ARAM skip + warning, çıktı boş | ✓ |
| empty input safe | ✓ |
| deterministik + sıralı çıktı | ✓ |

## 6. Durum
- Baseline: cargo test **426** · gate clippy `-D warnings` **0** · fmt-all temiz · pnpm typecheck pass · vitest **174/44**.
- ts-rs: 5 DTO üretildi (hepsi number/string/array — **bigint yok**) + TS contract guard
  (`match-aggregation.contract.test.ts`).

## 7. Codex'e (sonraki bağlama işi)
1. **Raw Riot Match-V5 fetcher** (key/proxy hazır olunca) → `MatchV5` listesine map'le (game_version → patch normalize).
2. **Aggregator çağırma:** `aggregate_matches(&matches)` → `AggregationResult`.
3. **DB upsert mapping:** `rates`/`matchups`/`builds` → `champion_rates`/`champion_matchups`/`champion_builds`
   (source='riot_match_v5', confidence + sample_size taşınır). `quality.warnings`/`skipped_matches` loglanır.
4. **Scheduler/cache:** pipeline `actions` (refresh_rates/builds/matchups) bu akışı tetikler; başarıda
   last-good cache güncellenir.

> Bu sıra: Claude saf aggregation **mantığını** kurdu; Codex raw fetch + DB upsert + scheduler/cache'e bağlar.
