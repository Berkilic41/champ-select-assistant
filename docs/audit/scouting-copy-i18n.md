# Scouting UI + Ban Coach — Copy / i18n Audit

> Tarih: 2026-06-04 · Sahip: Claude (2. mühendis) · Kapsam: Codex'in bağladığı Lobby Scouting +
> Ban Coach UI'sinin metin/i18n denetimi. **Hot UI'ya (BanSuggestionList/ChampSelectScreen) dokunulmadı.**
> Düzeltmeler Codex'te (UI) + koordineli (backend copy). Bu bir bulgu/öneri dokümanıdır.

## 1. Statik label'lar — ✅ TEMİZ

`BanSuggestionList.tsx` tüm sabit metinleri `t('champSelect.*')` ile alıyor. 7 anahtar, tr/en paritesi tam,
diacritics doğru:

| Anahtar | tr | en |
|---|---|---|
| `champSelect.banComputing` | Ban önerisi hesaplanıyor… | Calculating ban suggestions… |
| `champSelect.banSuggestions` | Ban önerileri: | Ban suggestions: |
| `champSelect.banOtp` | rakip OTP (%{{pct}}) | enemy OTP ({{pct}}%) |
| `champSelect.scoutingTitle` | Lobby scouting | Lobby scouting |
| `champSelect.scoutingPartial` | ince veri | thin data |
| `champSelect.scoutingBanTargets` | Scouting ban hedefleri | Scouting ban targets |
| `champSelect.scoutingProfiles` | Rakip profilleri | Enemy profiles |

→ 7 anahtar `src/i18n/i18n-parity.test.ts` REQUIRED_KEYS'e eklendi (regresyon kalkanı).

**Minör not:** `scoutingPartial` ("ince veri") değeri `performance.thinData` ile birebir aynı. İki
namespace'te kopya; zararsız ama istenirse tek anahtara indirgenebilir (düşük öncelik).

## 2. 🔴 Dinamik copy locale-leak (`recommendation/scouting.rs` — Claude-owned)

Scouting kartındaki **dinamik** metin backend'de üretilip UI'da ham gösteriliyor. Diller karışık:

| Alan | Backend üretimi | UI'da görünen | Sorun |
|---|---|---|---|
| `enemy.threat` | TR: `"yüksek"/"orta"/"düşük"` | TR | EN build'de de TR çıkar |
| `target.confidence` | **EN**: `"high"/"medium"/"low"` | **EN** | **TR build'de İngilizce** ← yan yana "yüksek" + "high" |
| `enemy.note` | TR cümle | TR | EN build'de TR |
| `target.reason` | TR cümle | TR | EN build'de TR |
| `enemy.playstyle_tags` | TR (`"erken agresif"`…) | TR | EN build'de TR |

**En görünür glitch:** Aynı kartta `threat` Türkçe ("yüksek") ama `confidence` İngilizce ("high").
Bu, daha önce düzelttiğimiz **PerformancePanel rol-etiketi** bug'ıyla aynı sınıf (backend i18n'i baypas ediyor).

Guardrail kontrolü: `reason` = "Rakibin ana kozu (%…); banı erken tempo ve oyun planını bozar" —
**abartı/garanti dili YOK** ("bozar" = ban'ın yaptığı, kazanç vaadi değil). ✅ coach_quality uyumlu.

## 3. Öneri — kademeli

### 3a. ✅ TAMAMLANDI (Codex, 2026-06-04)
`confidence` zaten **stabil token** (`high/medium/low`) döndürüyordu → UI `t('champSelect.confidence.*')` ile
map'ledi. tr/en'e `champSelect.confidence.{high,medium,low,unknown}` eklendi (tr: yüksek/orta/düşük/bilinmiyor),
parite **278/278**, REQUIRED_KEYS + BanSuggestionList testi güncellendi. TR kartta artık tek dil ("yüksek").

### 3b. ⏳ KISMİ — Rust bridge ✅ (Claude) · UI map AÇIK (Codex)

**Seçilen yol: B-bridge** (regresyon-sıfır, decoupled). Claude tarafı **TAMAMLANDI**:
- `scouting.rs`: **additive** `threat_level: String` token alanı eklendi (`"high"/"medium"/"low"`),
  eski `threat` (TR) **korundu** → UI hiç değişmeden TR çalışıyor. `threat_token()` helper TR bandı →
  token map'ler; iç logic karşılaştırmaları (`base_threat == "yüksek"`) TR'de bırakıldı (davranış değişmedi).
- 3 mevcut scouting testine `threat_level` assertion (high/low/medium — üç token kapsandı).
- ts-rs regen: `EnemyProfile.ts` artık `threat_level: string` taşıyor.
- `data-supremacy-contract.test.ts`: literal + runtime assert (`threat_level === 'high'`).
- ⚠️ **Codex dosyasına zorunlu 1-satır:** yeni **required** alan (repo ts-rs optional alan üretmiyor →
  Option bile `| null` required) `BanSuggestionList.test.tsx` EnemyProfile fixture'ının typecheck'ini kırdı;
  fixture'a `threat_level: 'high'` eklendi (sadece test fixture, component/behavior değişmedi).
- Baseline: cargo test 304 · clippy 0 · fmt (scouting.rs) · typecheck pass · vitest 135/31.

**Codex'in yapacağı (UI map — hot UI, istediği turda, senkron zorunluluk YOK):**
```tsx
// <span className="ban-scouting-threat">{enemy.threat}</span>
<span className="ban-scouting-threat">{t(`champSelect.threat.${enemy.threat_level}`, enemy.threat)}</span>
```
tr/en'e ekle (ikisine de) + i18n-parity REQUIRED_KEYS'e `champSelect.threat.{high,medium,low}`:
```json
// champSelect.threat (tr)
"threat": { "high": "yüksek", "medium": "orta", "low": "düşük" }
// champSelect.threat (en)
"threat": { "high": "high", "medium": "medium", "low": "low" }
```
UI token'a geçtikten sonra `threat` (TR) alanı deprecate edilebilir (ayrı temizlik turu).

`note`/`reason`/`playstyle_tags` tam i18n'i ise yapısal veri + UI template gerektirir (daha büyük; v1 TR-first
için ertelenebilir — mevcut TR copy doğru/güvenli, **veri uydurma yok**).

## 4. Durum
- Statik label denetimi: temiz, regresyon testine bağlandı.
- Kod davranışı **değişmedi** (sadece doc + test REQUIRED_KEYS genişletmesi). Baseline aynı.
- Sıradaki ürün işi Codex'te (DraftBrain detail panel / feedback loop UI). 3a tek-dil tutarlılığı için hızlı kazanım.
