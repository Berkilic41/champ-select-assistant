# champ-select-assistant

League of Legends champ-select overlay. Kendi maç geçmişin, mastery verilerin ve düşman kompozisyonunu birleştirerek seçim ekranında 5 şampiyon önerisi sunar.

## Stack

- **Backend**: Rust 1.80+ · Tauri 2 · SQLite (rusqlite + refinery)
- **Frontend**: React 19 · TypeScript 5 · Vite 7
- **Package manager**: pnpm

## Kurulum

```powershell
# Gereksinimler: Rust 1.80+, Node 20+, pnpm, cargo-tauri

cp .env.example .env
# .env içine Riot API anahtarını gir (RIOT_API_KEY=RGAPI-...)

pnpm install
```

## Geliştirme

```powershell
pnpm tauri dev          # Tauri dev modu (hot reload)
pnpm typecheck          # TypeScript tip kontrolü
```

## Test

```powershell
cd src-tauri
cargo test              # Rust birim testleri (188 test)
cargo clippy            # Rust lint
cargo fmt               # Rust formatlama
```

## Build

```powershell
pnpm tauri build        # Üretim installer'ı oluşturur
```

## Ortam Değişkenleri

`.env.example` dosyasına bakın. Asla `.env` dosyasını commit etmeyin.

| Değişken | Açıklama |
|----------|----------|
| `RIOT_API_KEY` | Riot Games API anahtarı (geliştirme key'i) |

## Mimari

```
src-tauri/src/
├── lcu/           # League Client Update lockfile + HTTP + WebSocket
├── db/            # SQLite repo katmanı (rusqlite + refinery)
├── ddragon/       # Data Dragon + CDragon cache
├── riot/          # Riot API client + rate limiter
├── recommendation/# Öneri motoru (scoring, team analysis)
├── meta/          # Meta veri kaynakları (Sprint D)
└── commands/      # Tauri command thin wrapper'ları
src/
└── components/    # React UI
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

Champ Select Assistant, Riot Games, Inc. ile bağlantılı değildir ve Riot Games
tarafından onaylanmamıştır. League of Legends ve Riot Games, Riot Games, Inc.'in
ticari markalarıdır. Uygulama otomatik lock/ban/pick yapmaz; yalnızca öneri sunar.
