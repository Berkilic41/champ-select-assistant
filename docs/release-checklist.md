# Release Checklist — Champ Select Assistant

## Versiyon Durumu

| Dosya | Versiyon | Durum |
|-------|---------|-------|
| `src-tauri/Cargo.toml` | 0.9.0-beta.1 | ✅ |
| `src-tauri/tauri.conf.json` | 0.9.0 | ✅ (MSI pre-release label desteklemiyor) |

**Dağıtım etiketi:** `0.9.0-beta.1` (closed beta candidate)  
**Not:** MSI target semver pre-release alfanümerik label kabul etmiyor. Bundle version `0.9.0`, Rust crate version `0.9.0-beta.1`.

---

## Updater Kararı — **Seçenek B Seçildi (Beta için kapalı)**

`active: false` yapıldı. Güvenli beta dağıtımı.

Sebep: `pubkey` boşken `active: true` imzasız güncelleme veya runtime hatasına yol açar.

### 1.0.0 Public Release İçin Yapılacaklar

```powershell
# 1. Key çifti üret
npx tauri signer generate -w ~/.tauri/champ-select-assistant.key

# 2. Pubkey'i tauri.conf.json'a yapıştır:
# "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6..."

# 3. active: true yap

# 4. Private key'i GitHub Secret ekle:
#    TAURI_SIGNING_PRIVATE_KEY + TAURI_SIGNING_PRIVATE_KEY_PASSWORD

# 5. release-public.yml zaten secret'ları kullanıyor.
```

---

## Pre-Release Test Kapısı

```powershell
cd "C:\Users\aslan\OneDrive\Masaüstü\lol\champ-select-assistant"
pnpm typecheck
pnpm test:run
cd src-tauri
cargo test --all
cargo clippy --all-features -- -D warnings
cargo fmt --all -- --check
```

## Production Build

```powershell
cd "C:\Users\aslan\OneDrive\Masaüstü\lol\champ-select-assistant"
pnpm tauri build
# Çıktı: src-tauri/target/release/bundle/
```

> **NOT:** `tauri.conf.json` semver pre-release label'ı reddederse:
> - `tauri.conf.json version` → `0.9.0`
> - `Cargo.toml version` → `0.9.0-beta.1` (kalsın)
> - Bu belgede dağıtım adı `0.9.0-beta.1` olarak belgelenir

---

## Sprint Tamamlanma Durumu

| Sprint | Konu | Durum |
|--------|------|-------|
| 1 | Repo Hygiene (garbage dosyalar temizlendi) | ✅ |
| 2 | LCU Dispatcher Fix (poller/WS duplicate kapat) | ✅ |
| 3 | Snapshot Test Altyapısı (insta, 5 fixture) | ✅ |
| 4 | Build/Rune Wiring (command-layer enrich, seed 20 champ) | ✅ |
| 5 | Matchup Real Data (ScoringContext map, seed 50 entry) | ✅ |
| 6 | Meta Source Spike (Lolalytics ERTELENDI — Cloudflare/ToS) | ⏸ Deferred |
| 7 | i18n + Settings Polish (SettingsPanel tam i18n, TR/EN key) | ✅ |
| 8 | UI Polish (QuickPick outline, BanSuggestion empty state, max-3 chip test) | ✅ |
| 9 | Release Gate (updater active:false, checklist) | ✅ |

---

## Temizlik Durumu

**Stray dosyalar (lol/ parent dizini) — Temizlendi ✅**

Silinen: `!o.isAllyAction`, `,`, `0.1)`, `0.10`, `0.20`, `0.3`, `0.7`, `30_000)`, `Err(e.into())`, `ItemData`, `[1]`, `r.draftScreenSize)`, `void`, `{},+`, `main.js`

Korunan: `ruvector.db` (tracked), `.claude/` (Claude workspace)

---

## Seed Tazeleme (her release öncesi — Faz A3)

- [ ] `node scripts/refresh-seeds.mjs` koş (yerel DB'de taze u.gg sync'i olmalı)
- [ ] Diff özetini incele (+yeni / -kalkan anahtar sayıları makul mü?)
- [ ] Adayları gözden geçir; uygunsa `builds_seed.json` / `matchup_seed.json` yerine koy
- [ ] `*.candidate.json` dosyaları commit'lenmez (.gitignore kuralı var)

---

## Visual QA Akışları

- [ ] Onboarding: 4 adım, doğru Türkçe, "otomatik kilitleme yok" mesajı net
- [ ] Lobby: Patch badge görünüyor, meta sync badge
- [x] ChampSelect pick: HeroCard chip'leri (max 3) ← Sprint 8 test ile garanti
- [ ] ChampSelect pick: DraftPlanPanel (damage_profile + blind_safety + exec_diff)
- [x] Ban phase: BanSuggestionList empty state görünür ← Sprint 8
- [ ] Finalization: "Seçim kilitlendi — Build planı:" başlığı
- [x] Settings: Tüm metinler i18n — hardcoded TR string yok ← Sprint 7
- [ ] Settings: Kaydet/İptal akışı, toast mesajı
- [ ] Error/Loading: LoadingSkeleton, ErrorBanner
- [ ] In-game overlay: Compact pencere; oyun bitince kullanıcı tercihine dönüş
