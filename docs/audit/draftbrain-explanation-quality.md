# DraftBrain Explanation Quality — Audit & Rubric (Sprint A)

> Tarih: 2026-06-04 · Sahip: Claude (2. mühendis) · Kapsam: DraftBrain açıklama (coaching)
> kalitesi — rubric, 20-senaryo kapsama matrisi, coach_quality guardrail genişletme. **Hot UI'ya
> dokunulmadı** (`src/components/champ-select/*`, `HeroCard.tsx`, `ChampSelectScreen.tsx`,
> `commands/champ_select.rs` hariç tutuldu). Sayılar **ölçüldü**, varsayılmadı (veri uydurma yok).

## 1. Rubric — açıklama pillar'ları

Her öneri (`Recommendation`) için 8 "açıklama direği" puanlanır. Kaynak: `recommendation/explanation_audit.rs`
(saf rubric + `coach_quality` primitive'leri). Her direk **covered / Thin / Missing / Absolute**.

| Pillar | Alan | "Covered" eşiği |
|---|---|---|
| DecisionSentence (NEDEN) | `decision_sentence` | anlamlı ≥4 kelime, abartı yok |
| LanePlan | `lane_plan` | present + ≥3 kelime + abartı yok |
| TeamfightPlan | `teamfight_plan` ∥ `teamfight_job` | present + ≥3 kelime |
| MidGamePlan | `mid_game_plan` | present + ≥3 kelime |
| FallbackPlan | `fallback_plan` | present + ≥3 kelime |
| RiskSurfaced (KAYBETME riski) | `risk_summary` | present + ≥3 kelime |
| WhyNot | `why_not[]` | boş değil, her madde ≥2 kelime, dedup, abartı yok |
| DataConfidence | `data_sources` / `*_confidence` | en az bir provenance/confidence sinyali |

**Hard bar (her öneride zorunlu):** abartı/garanti dil YOK **ve** anlamlı decision_sentence.
Diğerleri *soft* completeness (raporlanır, regresyonda kırmızı yapılır).

## 2. 20-senaryo kapsama matrisi

`explanation_quality_matrix` testi (KB bir kez yüklenir) tüm senaryoları **gerçek** `compute_recommendations`
+ `upgrade_recommendations_with_context` (local model+data pack) pipeline'ından geçirir.

| # | Senaryo | Rol | Bağlam | Kuyruk |
|---|---|---|---|---|
| 1 | mid-vs-ap-laner | mid | görünür AP laner (Ahri) | 420 |
| 2 | top-vs-bruiser | top | görünür bruiser (Darius) | 420 |
| 3 | jungle-blind-firstpick | jungle | blind, ilk pick | 420 |
| 4 | bot-vs-botlane | bot | 2'li botlane (Jinx+Leona) | 420 |
| 5 | support-vs-catcher | sup | catcher (Thresh) | 420 |
| 6 | mid-blind-firstpick | mid | blind, ilk pick | 420 |
| 7 | top-lastpick-vs-ad-comp | top | son pick, full AD comp | 420 |
| 8 | mid-vs-full-ap-comp | mid | full AP comp | 420 |
| 9 | aram-no-role | — | ARAM, rolsüz | 450 |
| 10 | jungle-vs-diver | jungle | diver (Lee Sin) | 420 |
| 11 | support-enchanter-vs-engage | sup | enchanter havuz vs engage | 420 |
| 12 | bot-poke-adc | bot | poke ADC (Ezreal) | 420 |
| 13 | top-tank-vs-ap | top | tank havuz vs AP | 420 |
| 14 | mid-assassin-vs-squishy | mid | suikastçı vs squishy | 420 |
| 15 | mid-no-mastery-stretch | mid | mastery YOK (stretch) | 420 |
| 16 | pool-with-kbless-champ | mid | KB-siz şampiyon havuzda | 420 |
| 17 | jungle-blind-ranked | jungle | blind | 420 |
| 18 | support-vs-hook-comp | sup | hook comp (Blitz+Naut) | 420 |
| 19 | top-vs-ranged | top | menzilli toplaner (Vayne) | 420 |
| 20 | mid-control-vs-dive | mid | control mage vs dive | 420 |

### Ölçülen kapsama (40 rec / 20 senaryo)

```
DecisionSentence: 100.0%  (40/40)
LanePlan:         100.0%  (40/40)
TeamfightPlan:    100.0%  (40/40)
MidGamePlan:      100.0%  (40/40)
FallbackPlan:     100.0%  (40/40)
RiskSurfaced:     100.0%  (40/40)
WhyNot:           100.0%  (40/40)
DataConfidence:   100.0%  (40/40)
soft gaps total: 0
empty scenarios: ["mid-no-mastery-stretch"]
```

## 3. Bulgular

1. **Mevcudiyet (presence) kapsama mükemmel.** Offline upgrade seviyesinde (local model+data pack) 8 direğin
   tamamı 40 önerinin tamamında dolu; hiçbir öneride abartı/garanti dil yok. Pipeline açıklama üretiminde sağlam.
2. **Hipotez düzeltildi (ölçümle):** `risk_summary`'nin `draft_plan`'a bağlı olduğu için sık boş kalacağını
   varsaymıştım — **ölçüm RiskSurfaced %100 gösterdi.** "KAYBETME riski" direği güvenilir doluyor. Varsayım değil
   ölçüm esas alındı.
3. **`mid-no-mastery-stretch` boş döndü** (0 öneri). Mastery YOK + meta verisi YOK → stretch-pick gate her şeyi
   eliyor. Engine seviyesinde beklenen; **UI/command layer "öneri yok" durumunu zarif ele almalı** (ya da command
   layer meta-floor sağlamalı). Tek-katman riski: sıfır mastery + sıfır meta = boş liste.
4. **DataConfidence, bir data_pack varlığına bağlı** (`attach_data_pack_badge`). Test offline floor olan
   local_seed pack'i geçiyor → %100. Mimaride command layer her zaman local_seed pack'i taban olarak sağladığı
   için pratikte hep dolu; ama pack hiç yoksa bu direk düşer (mimari değişmezliğe bağlı).

### Kapsamın ölçmediği (gerçek bir sonraki frontier)
Rubric **mevcudiyet + anlamlılık + abartısızlık** ölçer; **özgüllük/grounding** ölçmez:
- `lane_plan`/`risk_summary` matchup'a gerçekten **uyarlanıyor mu**, yoksa arketip-şablonu mu? (presence ≠ specificity)
- `why_not` gerçekten **karşılaştırmalı** mı (alternatifi adıyla eliyor mu), yoksa generic mi?
- `decision_sentence` somut sinyale (rakip/komp) **bağlanıyor mu**?

Bunlar gelecekteki rubric boyutları (Sprint A+1 önerisi): şablon-tekrarı tespiti, matchup-token grounding,
why_not karşılaştırma-referansı kontrolü. Bugün eklemedim çünkü yanlış-pozitif riski yüksek + ölçüm gerektirir.

## 4. coach_quality guardrail genişletme (uygulandı)

`recommendation/coach_quality.rs` (saf, hard guardrail) sertleştirildi — non-breaking:
- **why_not madde-bazlı anlamlılık:** her "neden X değil" satırı ≥2 kelime olmalı; çıplak etiket (`"Risk"`) →
  `CoachIssue::TooShort("why_not")`. (Önceden yalnız dedup + abartı kontrol ediliyordu.)
- **Yeni abartı ifadeleri:** `rakip oynayamaz`, `kaybetmen imkansız`, `kesin üstünlük` (çok-kelimeli → yanlış
  pozitif yok; "erken oyunda üstünlük kur" gibi meşru koçluk tetiklemiyor — testle doğrulandı).
- Yeni testler: `bare_why_not_entry_is_too_short`, `newly_added_absolute_phrases_flagged`.

`engine_pipeline_coaching_passes_audit` + 20-senaryo matrisi bu sertleştirmelerden sonra da yeşil → gerçek
pipeline çıktısı zaten bu bardan geçiyor.

## 5. Regresyon kalkanı
- `explanation_quality_matrix` — 20 bağlam, her öneride hard bar (abartı yok + decision_sentence dolu) +
  decision_sentence %100 invariant. Gelecekte bir builder değişikliği bir direği düşürür veya abartı dil
  sokarsa **kırmızı** olur.
- `audit_recommendation` rubric'i ileride QA/telemetri (lokal) için yeniden kullanılabilir saf API.

Doğrulama: cargo test **309** · clippy `-D warnings` **0** · fmt (kendi dosyalarım) temiz. Frontend etkisi yok
(yeni Rust modülü ts-rs export etmiyor).

## 6. Codex'e — yalnızca büyük UI adoption notları
Bu sprint **kod davranışını değiştirmedi** (rubric/test/doc + saf guardrail). UI tarafında **acil iş yok**.
İleride değer için (opsiyonel, hot UI sende):
1. **"Öneri yok" durumu:** sıfır-mastery + sıfır-meta bağlamında engine boş liste dönebiliyor (senaryo #15).
   HeroCard/ChampSelectScreen bunu zarif ele almalı (boş-durum metni) — veya command layer meta-floor versin.
2. **Specificity sinyali (ileride):** rubric bir sonraki sürümde "şablon mu, matchup'a özgü mü" ölçerse,
   düşük-özgüllük önerileri için UI'da nazik bir "genel plan" rozeti düşünülebilir (mevcut "Genel öneri"
   build rozetiyle aynı dürüstlük çizgisi). Henüz sinyal yok; sadece yön notu.

> Not: `risk_summary` ("nasıl kaybedersin") güvenilir doluyor — UI'da bu pillar'ı yüzeye çıkarmak yüksek
> değerli (master plan'ın "KAYBETME riski" sütunu). Şu an HeroCard'da gösterimi senin kapsamında.
