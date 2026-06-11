# i18n Parity Audit — Yeni DraftBrain/Coach UI metinleri

> Tarih: 2026-06-04 · Sahip: Claude (2. mühendis) · Kapsam: Codex'in UI binding'inde eklenen
> hardcoded metinler. **Bu bir plan/öneridir — hot UI dosyalarına Claude dokunmadı.** Component +
> tr/en değişiklikleri Codex'te (veya anlaşılan anahtar adlarıyla birlikte koordineli yapılmalı).

> ## ✅ DURUM: UYGULANDI (Codex, 2026-06-04)
> - `DataStatusBadges.tsx` pack/registry chip metinleri → `champSelect.*` i18n key'lerine taşındı.
> - `PerformancePanel.tsx` → `useTranslation` kullanıyor; **roleLabel duplication kaldırıldı**,
>   `champSelect.roles.*` reuse edildi (`performance.*` namespace eklendi).
> - tr.json/en.json paritesi **274/274**, onlyTR/onlyEN boş. Codex bu doc'taki anahtar adlarını birebir kullandı.
> - `DataStatusBadges` + `PerformancePanel` testleri TR-default i18n çıktısına göre güncellendi.
> - **Regresyon kalkanı (Claude):** `src/i18n/i18n-parity.test.ts` — parite + zorunlu anahtar varlığı +
>   boş-değer + `{{placeholder}}` parite kontrolü. tr/en herhangi bir gelecekte ayrışırsa kırmızı olur.
> - Baseline (Codex+Claude): typecheck pass · vitest 31 file/135 test · cargo test 304 · clippy/fmt pass.
>
> Aşağıdaki envanter/plan tarihsel kayıt olarak korunuyor (uygulanan hedef durumu gösterir).

## 1. Durum

- `DataStatusBadges.tsx` `useTranslation` kullanıyor ama **yeni pack/registry chip'leri hardcoded**.
- `PerformancePanel.tsx` `useTranslation`'ı hiç kullanmıyor — **tüm metinler hardcoded**.
- Ayrıca **diacritics/copy hataları** var (memory UX kuralı): `canli`→canlı, `uretim`→üretim,
  `maclardan`→maçlardan, `mac`→maç. i18n taşıması bunları düzeltmeli.
- **Rol etiketleri çoğaltılmış:** `PerformancePanel.roleLabel` Top/Jungle/Mid/Bot/Support'u yeniden
  yazıyor; oysa `champSelect.roles.{top,jungle,middle,bottom,utility}` zaten var → **yeniden kullan**.

## 2. Hardcoded metin envanteri + önerilen anahtarlar

### `DataStatusBadges.tsx`
| Mevcut literal | Önerilen anahtar | tr | en |
|---|---|---|---|
| `'Data pack eski'` | `champSelect.packStale` | Veri paketi eski | Data pack stale |
| `Son uretim: {time}` | `champSelect.packStaleHint` | Son üretim: {{time}} | Last generated: {{time}} |
| `'generated_at yok'` | `champSelect.packNoTimestamp` | Üretim zamanı bilinmiyor | No generated_at |
| `Pack {conf}` | `champSelect.packConfidence` | Paket {{conf}} | Pack {{conf}} |
| `'Local fallback'` | `champSelect.localFallback` | Yerel yedek | Local fallback |
| `'Cloud/canli kaynak yerine local seed sinyali aktif'` | `champSelect.localFallbackHint` | Canlı/cloud kaynak yerine yerel seed sinyali aktif | Running on local seed instead of a live/cloud source |
| `Veri {confidence}` | `champSelect.dataConfidence` | Veri {{conf}} | Data {{conf}} |
| `{matchups} matchup - {builds} build` | `champSelect.dataCoverageHint` | {{matchups}} matchup · {{builds}} build | {{matchups}} matchups · {{builds}} builds |

> `confidence` token'ları (`high/medium/low/unknown`) opsiyonel olarak `champSelect.confidence.*`
> ile yerelleştirilebilir (yüksek/orta/düşük/bilinmiyor). v1'de raw bırakılabilir.

### `PerformancePanel.tsx`
| Mevcut literal | Önerilen anahtar | tr | en |
|---|---|---|---|
| `'Post-game coach'` (eyebrow + aria) | `performance.eyebrow` | Maç sonrası koç | Post-game coach |
| `'Son maclardan ana ders'` | `performance.title` | Son maçlardan ana ders | Main lesson from recent games |
| `'ince veri'` | `performance.thinData` | ince veri | thin data |
| `'tilt riski'` | `performance.tiltRisk` | tilt riski | tilt risk |
| `{games} mac` | `performance.games` | {{n}} maç | {{n}} games |
| `Form {delta}` | `performance.form` | Form {{delta}} | Form {{delta}} |
| `{g} mac - {wr} WR - {kda} KDA` | `performance.champLine` | {{games}} maç · {{wr}} WR · {{kda}} KDA | {{games}} games · {{wr}} WR · {{kda}} KDA |
| `'Bilinmiyor'` (roleLabel fallback) | `champSelect.roles` yoksa `performance.unknownRole` | Bilinmiyor | Unknown |
| Top/Jungle/Mid/Bot/Support map | **REUSE** `champSelect.roles.{top,jungle,middle,bottom,utility}` | — | — |

> **Not — rol etiketi tutarsızlığı:** `champSelect.roles` tr build'de Üst/Orman/Orta/Alt/Destek
> gösteriyor; `PerformancePanel` ise tr build'de bile Top/Jungle/Mid/Bot/Support yazıyor. Reuse aynı
> zamanda bu tutarsızlığı da giderir (tek doğru kaynak).

### Düşük öncelik — trust-source chip (`DataStatusBadges.tsx` L119-121)
`${source.source.replace(/_/g,' ')} - ${source.confidence}` ve hint `${source}${sample}${patch}`
makine-kaynak adlarını (örn. `riot_match_v5`, `local_seed`) gösteriyor. Bunlar teknik provenance
etiketleri; v1'de raw bırakılabilir. İstenirse `confidence` token'ı (`low/heuristic`) için
`champSelect.confidence.*` map'i bunda da kullanılır. Kaynak adının kendisi çevrilmez (kanonik ID).

## 3. tr↔en key paritesi — ✅ DOĞRULANDI

Flatten-key diff çalıştırıldı: **tr 258 / en 258 anahtar, only-TR (none), only-EN (none)** — tam parite.
Yeni anahtarlar (`performance.*` + 8 `champSelect.*`) eklenirken **her ikisine** aynı yapıda eklenmeli;
taşıma sonrası diff tekrar çalıştırılıp 0 orphan teyit edilmeli.

```
node -e "const tr=require('./src/i18n/tr.json'),en=require('./src/i18n/en.json'); \
const flat=(o,p='')=>Object.entries(o).flatMap(([k,v])=>typeof v==='object'&&v?flat(v,p+k+'.'):[p+k]); \
const T=new Set(flat(tr)),E=new Set(flat(en)); \
console.log('onlyTR',[...T].filter(k=>!E.has(k))); console.log('onlyEN',[...E].filter(k=>!T.has(k)));"
```

## 4. Hazır JSON blokları (Codex yapıştırabilir)

**`champSelect` altına ekle (tr.json):**
```json
"packStale": "Veri paketi eski",
"packStaleHint": "Son üretim: {{time}}",
"packNoTimestamp": "Üretim zamanı bilinmiyor",
"packConfidence": "Paket {{conf}}",
"localFallback": "Yerel yedek",
"localFallbackHint": "Canlı/cloud kaynak yerine yerel seed sinyali aktif",
"dataConfidence": "Veri {{conf}}",
"dataCoverageHint": "{{matchups}} matchup · {{builds}} build"
```
**`champSelect` altına ekle (en.json):**
```json
"packStale": "Data pack stale",
"packStaleHint": "Last generated: {{time}}",
"packNoTimestamp": "No generated_at",
"packConfidence": "Pack {{conf}}",
"localFallback": "Local fallback",
"localFallbackHint": "Running on local seed instead of a live/cloud source",
"dataConfidence": "Data {{conf}}",
"dataCoverageHint": "{{matchups}} matchups · {{builds}} builds"
```
**Yeni `performance` namespace (tr.json):**
```json
"performance": {
  "eyebrow": "Maç sonrası koç",
  "title": "Son maçlardan ana ders",
  "thinData": "ince veri",
  "tiltRisk": "tilt riski",
  "games": "{{n}} maç",
  "form": "Form {{delta}}",
  "champLine": "{{games}} maç · {{wr}} WR · {{kda}} KDA",
  "unknownRole": "Bilinmiyor"
}
```
**Yeni `performance` namespace (en.json):**
```json
"performance": {
  "eyebrow": "Post-game coach",
  "title": "Main lesson from recent games",
  "thinData": "thin data",
  "tiltRisk": "tilt risk",
  "games": "{{n}} games",
  "form": "Form {{delta}}",
  "champLine": "{{games}} games · {{wr}} WR · {{kda}} KDA",
  "unknownRole": "Unknown"
}
```

## 5. Migration planı (Codex — hot UI) — ✅ TAMAMLANDI

1. **`PerformancePanel.tsx`**: `useTranslation` ekle; literal'leri `t('performance.*')` ile değiştir;
   `roleLabel`'ı `t('champSelect.roles.'+role)` (utility/jungle vb.) ile değiştir, fallback `unknownRole`.
   Metrikler: `t('performance.games',{n})`, `t('performance.champLine',{games,wr,kda})`.
2. **`DataStatusBadges.tsx`**: pack/registry chip label/hint'lerini `t('champSelect.*')` ile değiştir;
   `packConfidence`/`dataConfidence` `{{conf}}` ile, `dataCoverageHint` `{{matchups}},{{builds}}` ile.
3. **tr.json + en.json**: yukarıdaki blokları ekle (ikisine de).
4. **Diacritics:** taşıma sırasında canlı/üretim/maçlardan/maç düzeltilir (yukarıdaki tr değerleri doğru).
5. Test: `PerformancePanel.test.tsx` / `DataStatusBadges.test.tsx` literal yerine i18n çıktısını
   beklemeli (örn. `Pack high` → tr build'de `Paket high`; testler tr default'a göre güncellenir).

## 6. Not (Claude'dan)
- `DataSourceRegistryReport.generated_at` artık **`number`** (bigint değil) — `data-quality-parity.md`
  doğru, eski iddia yok. UI'da `formatUnixSeconds` `number` ile çalışıyor ✓.
- Bu plan paylaşımdır; tr/en + component edit'leri Codex'te kalsın (anahtar adlarında anlaşıldıktan sonra).
