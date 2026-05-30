# Draft IQ Engine

Winrate/pickrate ötesinde oyun mantığı üreten tavsiye motoru. Yalnızca istatistik değil; combo planı, takım kompozisyonu, execution riski ve blind pick güvenliği de hesaplanır.

## Özellikler

- **Combo tespiti**: KB'deki combo çiftleri ile ally takımdaki şampiyonlar eşleştirilir
- **Damage balance**: Takımın AP/AD ağırlığına göre karşıt pick önerilir
- **Takım ihtiyacı**: Engage, frontline, hard CC, peel eksiklikleri tespit edilir
- **Blind pick safety**: İlk pick'te düşük güvenlikli şampiyonlar risk puanı alır
- **Stretch pick gate**: Comfort < 0.10 olan şampiyon yalnızca combo_bonus ≥ 0.80 ile ve en fazla 1 adet top-5'e girer; `risk_note` zorunlu
- **UI**: HeroCard expanded overlay'deki DraftPlanPanel (Win Condition, Combo Planları, Takım İhtiyacı, Riskler, Stretch Uyarısı)

## Mimari

```
resources/draft_iq/
├── champions.json   ← 80 şampiyon arketip verisi
├── combos.json      ← 50 kanıtlanmış combo çifti
└── SCHEMA.md        ← Alan tanımları ve kaynak disiplini

src-tauri/src/recommendation/draft_iq/
├── mod.rs           ← DraftKnowledgeBase (compile-time include_str!)
├── archetype.rs     ← ChampionArchetype struct + loader
├── combos.rs        ← ComboPair, ComboDirectory, find_for_ally()
├── analyzer.rs      ← analyze_pick(), compute_combo_bonus() vb.
├── narrative.rs     ← TR metin üretimi (win_condition, team_role, threats)
└── tests.rs         ← 15 birim + 7 entegrasyon + 1 E2E testi

src-tauri/src/recommendation/engine.rs  ← compute_recommendations() + stretch gate
src/components/champ-select/DraftPlanPanel.tsx  ← UI
```

## Scoring Değişiklikleri

| Bileşen | Eski | Yeni |
|---------|------|------|
| `synergy_score` | Tag heuristik | `max(tag, combo_bonus, team_need_score × 0.6)` |
| `team_counter_score` | Tag heuristik | `(tag + damage_balance) / 2` |
| `risk_score` | Ban rate + sample ceza | `+ blind_unsafety` (first pick) |

## JSON Kaynak Disiplini

Detaylar için `resources/draft_iq/SCHEMA.md`. Özetle:
- Archetype: League Wiki "Class/Subclass"
- CC tipi: League Wiki ability detail
- Combo: Her combo'nun `ability_ref` alanı gerçek ability adlarını içermeli
- Spekülasyon yasak; emin değilsek `confidence: "low"`

## Patch Sonrası Güncelleme Checklist

- [ ] Rework'ed şampiyonların `has_hard_cc`, `engage_role`, `mobility` alanları güncellendi mi?
- [ ] Yeni şampiyonlar eklendi mi (tüm alanlar, `confidence: "medium"` ile başla)?
- [ ] Combos.json'daki `ability_ref` hâlâ güncel mi (ability ismi/mekanik değişti mi)?
- [ ] Confidence = "low" olanlar gelişmiş patch notlarıyla revize edildi mi?
- [ ] `cargo test --all` geçiyor mu (özellikle `archetype_count_and_champion_id: 80`)?

## Geliştirme Notları

- `DraftKnowledgeBase::load()` compile-time `include_str!` ile JSON'ları gömer; build artifact boyutuna etkisi minimal
- `analyzer.rs` tamamen saf fonksiyon (no I/O); test edilmesi kolay
- KB'de olmayan şampiyon için engine tag-heuristic fallback'e düşer; `draft_plan = None`
- ARAM (queue_id 450) için combo mantığı devre dışı; mevcut ARAM ağırlık profili korunur
