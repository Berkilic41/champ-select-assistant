# Draft IQ Knowledge Base — Schema Reference

## Kaynak Disiplini

### Şampiyon verisi için kabul edilen kaynaklar
- **Archetype / Class:** League Wiki "Champions by class" sayfası (resmi Riot sınıflandırması)
- **CC tipi:** League Wiki ability sayfasındaki "Crowd Control" tag'leri
- **Damage profile:** League Wiki "Damage type" + DDragon `info.attack`/`info.magic` değerleri (kaba rehber)
- **Şampiyon listesi seçimi:** u.gg, lolalytics, op.gg pick popülerlik verileri veya mevcut app match_history verisi

### Combo için kurallar
- Her combo'nun `ability_ref` alanı iki şampiyonun gerçek ability adlarına referans vermelidir.
- "X iyi gider Y ile" tarzı subjektif yorum **yasaktır**.
- Kanıtlanamayan mekanik etkileşim içeren combo eklenmez.
- `strength` değeri: 0.90+ = neredeyse garantili wombo / klasik pro-play combo; 0.75-0.89 = güçlü sinerji, koşula bağımlı.

### Confidence seviyeleri
- `"high"`: League Wiki veya yaygın kabul görmüş pro-play referansı var.
- `"medium"`: Genel olarak doğru ama patch değişimiyle stale olabilir veya kaynağı zayıf.
- `"low"`: Tahmine dayalı veya kaynak belirsiz. Engine bu değeri fallback weight için kullanır.

### Kaynak notları (bu KB versiyonu için)
- Şampiyon archetypes: League Wiki Class sayfası (Vanguard, Diver, Juggernaut, Skirmisher, Burst Mage, Control Mage, Artillery, Marksman, Catcher, Enchanter, Warden)
- Champion IDs: DDragon 14.x champion.json
- CC types: League Wiki ability descriptions (hard CC = stun/root/knockup/knock_back/suppression/charm/taunt/fear/polymorph/freeze/airborne; soft CC = slow/silence/nearsight/disarm)
- Damage profiles: oyun-içi deneyim + wiki verisi; oran tahmin, DPS hesabı değil

---

## champions.json Schema

Her anahtar, DDragon `key` değeridir (örn. `"Orianna"`, `"LeeSin"`, `"KaiSa"`).

```
{
  "ChampionKey": {
    "champion_id":          number,       // DDragon champion ID (integer)
    "archetype":            ArchetypeEnum,
    "damage_profile": {
      "ad":                 number,       // 0.0 – 1.0; ad + ap + true = 1.0
      "ap":                 number,
      "true":               number
    },
    "cc": {
      "has_hard_cc":        boolean,      // true if any ability applies hard CC
      "hard_cc_count":      number,       // number of abilities with hard CC
      "primary_cc":         string[]      // CcTypeEnum values (may be empty)
    },
    "mobility":             MobilityEnum,
    "engage_role":          EngageRoleEnum,
    "peel_capability":      PeelEnum,
    "blind_safety":         number,       // 0.0 – 1.0; pick safety in blind/first pick
    "execution_difficulty": number,       // 1 (easiest) – 5 (hardest)
    "win_condition":        WinConditionEnum,
    "ult_type":             string,       // free-form descriptor (e.g. "zone_aoe")
    "confidence":           ConfidenceEnum
  }
}
```

### ArchetypeEnum
`juggernaut` · `diver` · `vanguard` · `skirmisher` · `control_mage` · `burst_mage` · `artillery` · `assassin` · `marksman` · `catcher` · `enchanter` · `warden`

### MobilityEnum
`none` · `low` · `medium` · `high` · `very_high`

### EngageRoleEnum
`none` · `initiator` · `follow_up` · `pick` · `disengage`

### PeelEnum
`none` · `low` · `medium` · `high`

### WinConditionEnum
`teamfight` · `split_push` · `pick` · `skirmish` · `poke` · `siege` · `protect`

### CcTypeEnum (primary_cc values)
Hard CC: `stun` · `root` · `knockup` · `knockback` · `suppression` · `charm` · `taunt` · `fear` · `polymorph` · `freeze` · `hook` · `cage` · `pull`
Soft CC: `slow` · `silence` (soft CC değerleri has_hard_cc = false ile kullanılır)

### ConfidenceEnum
`high` · `medium` · `low`

---

## combos.json Schema

Dizi: her eleman bir combo çiftidir. Çiftler simetriktir (`a` ile `b` sırası önemsiz).

```
[
  {
    "a":            string,       // DDragon champion key (e.g. "Nocturne")
    "b":            string,       // DDragon champion key (e.g. "Orianna")
    "name":         string,       // İngilizce kısa isim
    "type":         ComboTypeEnum,
    "ability_ref":  string,       // Zorunlu: ability adları + mekanik açıklama (İngilizce)
    "tr":           string,       // Türkçe display string (UI'da gösterilecek)
    "strength":     number,       // 0.0 – 1.0; combo gücü
    "confidence":   ConfidenceEnum
  }
]
```

### ComboTypeEnum
`engage_followup` · `wombo` · `pick_potential` · `peel_chain` · `zone_control`

**Tanımlar:**
- `engage_followup`: A engage eder, B takip eder (örn. Malphite R → Orianna R)
- `wombo`: İkisi birlikte AOE teamfight kombinasyonu (örn. Amumu R + MF R)
- `pick_potential`: İzole target lockdown (örn. Thresh Q + Caitlyn trap)
- `peel_chain`: Koruma/disengage zinciri (örn. Lulu R + Jinx passive)
- `zone_control`: Alan kontrolü sinerjisi (örn. Jarvan E+Q + Orianna R)

---

## Eksik verilerde ne yapılır

- Ability detayı bilinmiyorsa `confidence: "low"` kullan; `has_hard_cc: false`, `hard_cc_count: 0` ile conservative başla.
- Combo mekanik etkileşimi açıklanamıyorsa combo ekleme.
- Patch sonrası stale olan veriler için bu dosyayı güncelle ve `confidence` seviyesini düşür.

## Patch güncellemesi checklist
- [ ] `has_hard_cc` değişen şampiyonlar (rework)
- [ ] Silinen veya eklenen ability'ler (E/W/R rework)
- [ ] Yeni champion ekleme: tüm alanlar, `confidence: "medium"` ile başla
- [ ] Güncellenen combos: eski `ability_ref` hâlâ geçerli mi?
