# champ-select-assistant

League of Legends champ-select overlay. Kendi maç geçmişin, mastery verilerin ve düşman kompozisyonunu birleştirerek seçim ekranında 5 şampiyon önerisi sunar.

## Öne Çıkanlar (doğrulanabilir)

- **Tüm roster — eksiksiz:** 172/172 şampiyon için draft analizi (arketip, hasar profili, CC, engage/peel). Kapsam, DDragon'a karşı bir testle zorunlu kılınır (`validate_ddragon_completeness`).
- **Güncel patch:** Şampiyon/item/rune verisi Data Dragon / Community Dragon üzerinden o anki en güncel patch'ten otomatik çekilir (sürüm sabit değil, DDragon'ın son sürümü neyse o). Meta (win/pick/ban) oranları yakında kendi Riot API tabanlı veri servisinden gelecek (bkz. `docs/api-key-policy.md`); şu an comfort/matchup/sinerji/arketip sinyalleriyle öneri üretilir.
- **Draft IQ:** 123 ability-referanslı combo + lane matchup tablosu; kapsanmayan her eşleşme arketip-tabanlı counter ile yine de değerlendirilir (boş öneri yok).
- **Dürüst güven:** Az veri / yeni patch / düşük örneklem durumları "güven" etiketiyle açıkça gösterilir — uydurma kesinlik yok.
- **Güvenli:** Otomatik lock/ban/pick **yapmaz** (sadece öneri); LCU-first (developer API key gerekmez); telemetry yok.

## Stack

- **Core**: Rust 1.80+ → WebAssembly (`core/` — host-agnostik scoring & draft motoru)
- **Host**: Electron (`desktop/` — Node I/O, LCU, `node:sqlite` ile DB)
- **Frontend**: React 19 · TypeScript 5 · Vite 7 (`src/`, iki host'ta ortak)
- **Package manager**: pnpm (workspace)

## Kurulum

```powershell
# Gereksinimler: Rust 1.80+ (wasm32-unknown-unknown + wasm-pack), Node 22.5+, pnpm

cp .env.example .env
# .env içine Riot API anahtarını gir (RIOT_API_KEY=RGAPI-...)

pnpm install
```

## Geliştirme

```powershell
pnpm --filter csa-desktop dev   # Electron dev (hot reload)
pnpm typecheck                  # TypeScript tip kontrolü
```

## Test

```powershell
cd core
cargo test                            # Rust/core birim testleri
cargo clippy --all-features -- -D warnings
cargo fmt --all -- --check

# kökten (renderer) + desktop host:
pnpm test:run                         # React/renderer testleri
pnpm --filter csa-desktop test        # Electron host testleri (önce WASM build)
```

## Build

```powershell
pnpm --filter csa-desktop build:wasm   # core → WASM (dist öncesi otomatik çalışır)
pnpm --filter csa-desktop dist         # electron-builder ile Windows installer
```

## Ortam Değişkenleri

`.env.example` dosyasına bakın. Asla `.env` dosyasını commit etmeyin.

| Değişken | Açıklama |
|----------|----------|
| `RIOT_API_KEY` | Riot Games API anahtarı (geliştirme key'i) |

## Mimari

```
core/                    # Rust → WASM motoru (host-agnostik)
├── src/recommendation/  # Öneri motoru (scoring, team analysis)
├── resources/draft_iq/  # Draft IQ seed verisi (combos, arketipler, şampiyonlar)
└── pkg/                 # wasm-pack çıktısı (build'de üretilir, gitignore)
desktop/                 # Electron host (Node I/O)
├── src/main/            # LCU (lockfile+HTTP+WS), node:sqlite DB, IPC, pencere
└── src/preload/         # contextBridge köprüsü (window.api)
src/                     # React/TS renderer (Tauri ve Electron'da ortak)
└── components/
cloudflare-worker/       # Match-V5 ingestion + anonim /v1/rates aggregate
```

## Roadmap

Bkz. `docs/senior-roadmap-v3.md`

## Yasal ve Gizlilik

- [Gizlilik Politikası](PRIVACY.md) — veriler yerelde kalır, telemetry yok
- [Kullanım Şartları](TERMS.md) — yalnızca öneri, otomatik aksiyon yok
- [Lisans](LICENSE) — proprietary, tüm hakları saklıdır
- [Changelog](CHANGELOG.md)

Destek ve hata bildirimi: [GitHub Issues](https://github.com/Berkilic41/champ-select-assistant/issues)

## Disclaimer

Champ Select Assistant isn't endorsed by Riot Games and doesn't reflect the views
or opinions of Riot Games or anyone officially involved in producing or managing
Riot Games properties. Riot Games and all associated properties are trademarks or
registered trademarks of Riot Games, Inc.

Uygulama otomatik lock/ban/pick **yapmaz**; yalnızca öneri sunar.
