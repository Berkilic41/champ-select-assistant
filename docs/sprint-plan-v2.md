# champ-select-assistant — Sprint Planı v2 [ARŞİV]

> **Bu belge geçerliliğini yitirmiştir.** Aktif yol haritası: `docs/senior-roadmap-v3.md`

---

# champ-select-assistant — Sprint Planı v2

> Oluşturulma: 2026-05-12  
> Kaynak: lol-architect + lol-data-eng + lol-ux paralel analiz  
> Referans: `~/.claude/plans/sen-deneyimli-bir-r-n-zippy-tiger.md` (orijinal plan)

---

## Mevcut Durum (Sprint 1 Kısmi)

**Tamamlanan:**
- LCU lockfile parsing — `src-tauri/src/lcu/lockfile.rs` (birim testleri dahil)
- LCU HTTP client — `src-tauri/src/lcu/client.rs` (rustls, Basic Auth)
- 2 Tauri command — `connect_lcu`, `get_lcu_status` (`lib.rs:96-103`)
- Temel bağlantı badge'i — `src/App.tsx`

**Eksik:**
- `Cargo.toml`'da `rusqlite`, `refinery`, `governor`, `ts-rs`, `dotenvy` yok
- `db/`, `riot/`, `ddragon/`, `recommendation/` modülleri yok
- SQLite migration runner yok
- Data Dragon cache yok
- `AppState`'de DB handle yok
- `tokio::sync::Mutex` geçişi yapılmamış (deadlock riski)

---

## Cargo.toml — Eklenecek Bağımlılıklar

Sprint 1.5 **ilk adım** olarak `src-tauri/Cargo.toml`'a eklenmeli:

```toml
[dependencies]
# Mevcut: tokio, reqwest, serde, base64, anyhow, thiserror, tracing

# Sprint 1.5
rusqlite  = { version = "0.31", features = ["bundled"] }
refinery  = { version = "0.8",  features = ["rusqlite"] }
dotenvy   = "0.15"

# Sprint 2
governor  = "0.6"
ts-rs     = "10"

# Sprint 3 (sonra eklenecek)
# tokio-tungstenite = "0.21"
# scraper           = "0.19"
```

---

## Modül Yapısı (Hedef)

```
src-tauri/src/
├── lib.rs                      # AppState, run(), Tauri builder
├── errors.rs                   # AppError enum (thiserror)
├── commands/                   # #[tauri::command] thin wrapper'lar
│   ├── mod.rs
│   ├── champ_select.rs         # connect_lcu, get_session
│   ├── recommendations.rs      # get_recommendations
│   └── settings.rs             # get/set app settings
├── lcu/                        # MEVCUT — genişletilecek
│   ├── mod.rs
│   ├── lockfile.rs             # TAMAM
│   ├── client.rs               # TAMAM
│   ├── session.rs              # /v1/session parse (Sprint 3)
│   ├── websocket.rs            # WS subscribe (Sprint 3)
│   └── events.rs               # Tauri event emit router (Sprint 3)
├── db/                         # Sprint 1.5
│   ├── mod.rs                  # embed_migrations! + DbPool
│   ├── connection.rs           # WAL mode, migration runner
│   ├── champion_repo.rs
│   ├── match_repo.rs
│   └── meta_repo.rs
├── riot/                       # Sprint 2
│   ├── mod.rs
│   ├── client.rs               # API key + base URLs
│   ├── rate_limiter.rs         # governor token bucket (20/s)
│   └── endpoints/              # summoner, match, mastery
├── ddragon/                    # Sprint 1.5
│   ├── mod.rs
│   ├── downloader.rs           # versions.json + champion.json fetch
│   └── cache.rs                # SQLite'a yaz/oku
└── recommendation/             # Sprint 3
    ├── mod.rs
    ├── scoring.rs
    ├── comfort.rs
    └── engine.rs
src-tauri/migrations/
├── V001__init.sql
├── V002__champions.sql
├── V003__matches.sql
└── V004__builds.sql
```

---

## AppState (Hedef)

```rust
pub struct AppState {
    pub lcu_client: tokio::sync::Mutex<Option<lcu::LcuClient>>,  // std→tokio geçiş
    pub db:         Arc<db::DbPool>,
    pub riot:       Arc<riot::RiotClient>,      // Sprint 2'de eklenir
    pub ddragon:    Arc<ddragon::DdragonCache>, // Sprint 1.5'te eklenir
}
```

---

## Veritabanı Şeması

```
app_config       (key TEXT PK, value)
ddragon_cache    (version TEXT PK, base_path, downloaded_at)
champions        (champion_id INT PK, key UNIQUE, name, title, cached_at)
summoners        (puuid TEXT PK, game_name, tag_line, summoner_id?, region, cached_at)
    ├── matches  (match_id TEXT PK, puuid FK, champion_id FK, position?, win, kda, queue_id, played_at)
    └── mastery  (puuid+champion_id PK, mastery_level, mastery_points, last_play_time?)
builds           (id AUTOINCREMENT, champion_id FK, position, patch_version, item_ids JSON, rune_ids JSON, win_rate, pick_rate, source, cached_at)
```

### Migration Dosyaları

**`src-tauri/migrations/V001__init.sql`**
```sql
CREATE TABLE IF NOT EXISTS app_config (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS summoners (
    puuid        TEXT NOT NULL PRIMARY KEY,
    game_name    TEXT NOT NULL,
    tag_line     TEXT NOT NULL,
    summoner_id  TEXT,
    region       TEXT NOT NULL,
    cached_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_summoners_region ON summoners(region);
```

**`src-tauri/migrations/V002__champions.sql`**
```sql
CREATE TABLE IF NOT EXISTS ddragon_cache (
    version       TEXT NOT NULL PRIMARY KEY,
    base_path     TEXT NOT NULL,
    downloaded_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS champions (
    champion_id INTEGER NOT NULL PRIMARY KEY,
    key         TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    title       TEXT NOT NULL,
    cached_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_champions_key ON champions(key);
```

**`src-tauri/migrations/V003__matches.sql`**
```sql
CREATE TABLE IF NOT EXISTS matches (
    match_id      TEXT    NOT NULL PRIMARY KEY,
    puuid         TEXT    NOT NULL,
    champion_id   INTEGER NOT NULL,
    position      TEXT,
    win           INTEGER NOT NULL CHECK (win IN (0,1)),
    kills         INTEGER NOT NULL DEFAULT 0,
    deaths        INTEGER NOT NULL DEFAULT 0,
    assists       INTEGER NOT NULL DEFAULT 0,
    duration_secs INTEGER NOT NULL,
    queue_id      INTEGER NOT NULL,
    played_at     INTEGER NOT NULL,
    FOREIGN KEY (puuid)       REFERENCES summoners(puuid)  ON DELETE CASCADE,
    FOREIGN KEY (champion_id) REFERENCES champions(champion_id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_matches_puuid       ON matches(puuid);
CREATE INDEX IF NOT EXISTS idx_matches_puuid_champ ON matches(puuid, champion_id);
CREATE INDEX IF NOT EXISTS idx_matches_played_at   ON matches(played_at DESC);
CREATE INDEX IF NOT EXISTS idx_matches_queue       ON matches(queue_id);

CREATE TABLE IF NOT EXISTS mastery (
    puuid          TEXT    NOT NULL,
    champion_id    INTEGER NOT NULL,
    mastery_level  INTEGER NOT NULL,
    mastery_points INTEGER NOT NULL DEFAULT 0,
    last_play_time INTEGER,
    PRIMARY KEY (puuid, champion_id),
    FOREIGN KEY (puuid)       REFERENCES summoners(puuid)  ON DELETE CASCADE,
    FOREIGN KEY (champion_id) REFERENCES champions(champion_id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_mastery_puuid  ON mastery(puuid);
CREATE INDEX IF NOT EXISTS idx_mastery_points ON mastery(puuid, mastery_points DESC);
```

**`src-tauri/migrations/V004__builds.sql`**
```sql
CREATE TABLE IF NOT EXISTS builds (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    champion_id   INTEGER NOT NULL,
    position      TEXT    NOT NULL,
    patch_version TEXT    NOT NULL,
    item_ids      TEXT    NOT NULL,
    rune_ids      TEXT    NOT NULL,
    win_rate      REAL    NOT NULL DEFAULT 0.0,
    pick_rate     REAL    NOT NULL DEFAULT 0.0,
    source        TEXT    NOT NULL,
    cached_at     INTEGER NOT NULL,
    FOREIGN KEY (champion_id) REFERENCES champions(champion_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_builds_unique
    ON builds(champion_id, position, patch_version, source);
CREATE INDEX IF NOT EXISTS idx_builds_champ_pos ON builds(champion_id, position);
CREATE INDEX IF NOT EXISTS idx_builds_cached_at ON builds(cached_at DESC);
```

---

## UI Component Yapısı

```
src/
├── index.css                  # CSS değişkenleri (tüm root vars)
├── App.tsx                    # AppShell'e delege et
└── components/
    ├── layout/
    │   └── AppShell.tsx       # header + state router (status prop)
    ├── connection/
    │   ├── ConnectionBadge.tsx    # mevcut badge — genişletilecek
    │   └── DisconnectedView.tsx   # hata mesajı + retry button
    ├── lobby/
    │   └── LobbyView.tsx          # "Bağlandı, CS bekleniyor"
    ├── champ-select/
    │   ├── ChampSelectScreen.tsx  # koordinatör konteyner
    │   ├── Timer.tsx              # countdown + urgency renk
    │   ├── TeamPanel.tsx          # 5 slot (ally/enemy)
    │   │   └── TeamSlot.tsx       # tek pozisyon satırı
    │   ├── RecommendationList.tsx
    │   │   └── ChampionCard.tsx   # icon + isim + wr + pr + not
    │   ├── BuildSummary.tsx       # 3 core item + rune
    │   └── EnemyPanel.tsx         # ban + pick listesi
    └── shared/
        ├── ChampionIcon.tsx       # Data Dragon img + skeleton
        ├── LoadingSkeleton.tsx    # shimmer animasyon
        └── DataBadge.tsx          # "düşük veri" / "yeni patch" uyarısı
```

### Renk Paleti

```css
:root {
  --color-gold:       #C89B3C;
  --color-bg-deep:    #0A0A0A;
  --color-bg-panel:   #141414;
  --color-border:     #2A2A2A;
  --color-success:    #4CAF50;
  --color-danger:     #F44336;
  --color-text:       #E8E0D0;
  --color-text-muted: #888888;

  --space-1: 4px; --space-2: 8px; --space-3: 12px;
  --space-4: 16px; --space-6: 24px;
  --champ-icon-sm: 32px; --champ-icon-md: 48px;
  --panel-width:  160px; --card-height: 56px;
  --transition-fast: 150ms ease;
}
```

---

## Sprint Planı

### Sprint 1.5 — Temel Altyapı (~3-4 akşam)

**Hedef:** DB çalışıyor, Data Dragon cache'lendi, uygulama "DB ready + DDragon cached" gösteriyor.

**Sıra:**
1. `Cargo.toml` güncelle — rusqlite (bundled) + refinery + dotenvy
2. `src-tauri/src/errors.rs` — `AppError` enum (thiserror), lcu/db varyantları
3. `src-tauri/migrations/V001__init.sql` + `V002__champions.sql` — (yukarıdaki SQL)
4. `src-tauri/src/db/connection.rs` — WAL mode, `embed_migrations!`, startup runner
5. `src-tauri/src/db/mod.rs` — `DbPool` tip alias, `get_conn()` helper
6. `AppState` güncelle — `db: Arc<DbPool>` ekle, `tokio::sync::Mutex` geçişi
7. `src-tauri/src/ddragon/downloader.rs` — versions.json → champion.json → DB upsert
8. `src-tauri/src/ddragon/cache.rs` — trait impl, `refresh_if_stale()`
9. `commands/` klasörü oluştur — mevcut inline command'ları taşı
10. Frontend: `AppShell.tsx` state router + `DisconnectedView` + `LobbyView` temel yapısı

**Agent ekibi:** `lol-architect` → `lol-rust-dev` + `lol-ts-dev` (paralel) → `lol-tester` → `lol-reviewer`

**Doğrulama:**
```powershell
cd src-tauri; cargo test           # migration + ddragon testleri geçer
pnpm tauri dev                     # "DB ready" + "DDragon 14.x cached" görünür
```

---

### Sprint 2 — Riot API + Match Ingestion (~4-5 akşam)

**Hedef:** Oyuncunun son 20 ranked maçı DB'de, champion mastery DB'de, UI'da champion grid var.

**Görevler:**
1. `src-tauri/migrations/V003__matches.sql` + `V004__builds.sql`
2. `riot/client.rs` — API key, platform/regional URL builder
3. `riot/rate_limiter.rs` — governor Quota::per_second(20)
4. `riot/endpoints/summoner.rs` — by-name → PUUID
5. `riot/endpoints/match.rs` — IDs + detail (queue=420, last 20)
6. `riot/endpoints/mastery.rs` — top mastery
7. `db/match_repo.rs` + `db/champion_repo.rs` — CRUD + upsert
8. `commands/riot.rs` — `sync_riot_player`, `sync_match_history`, `sync_masteries`
9. ts-rs type codegen — `shared/types/` oluştur
10. Frontend: `ChampionIcon` + `LoadingSkeleton` + basit champion grid (mastery sıralı)

**Agent ekibi:** `lol-architect` (kontrat) → `lol-rust-dev` + `lol-ts-dev` (paralel) → `lol-data-eng` (match repo SQL) → `lol-tester` → `lol-reviewer`

**Doğrulama:**
```powershell
cargo test --test integration    # LCU mock + Riot mock testleri
pnpm tauri dev                   # Champion grid görünür (mastery sıralı)
```

---

### Sprint 3 — Champ Select WebSocket + Öneri Motoru (~4-6 akşam)

**Hedef:** LoL champion select açıldığında otomatik 5 öneri görünüyor, timer çalışıyor.

**Görevler:**
1. `lcu/session.rs` — `/lol-champ-select/v1/session` parse + `ChampSelectState` struct
2. `lcu/websocket.rs` — WS bağlantısı, event subscribe
3. `lcu/events.rs` — Tauri `emit` router ("champ-select-updated" event)
4. `recommendation/scoring.rs` — comfort score (mastery + match win rate) + meta score
5. `recommendation/engine.rs` — `RecommendationEngine` trait impl
6. `commands/recommendations.rs` — `get_recommendations(session_json)`
7. Frontend: `ChampSelectScreen` tam implementasyon (Timer + TeamPanel + RecommendationList + BuildSummary + EnemyPanel)
8. `ChampionCard` — icon + wr + pr + 1 satır not
9. State management: Tauri event listener → React state

**Agent ekibi:** `lol-architect` → `lol-rust-dev` (WS + engine) + `lol-frontend` (UI) + `lol-ux` (UX review) paralel → `lol-tester` → `lol-perf` + `lol-reviewer` paralel

**Doğrulama:**
- LoL client aç, champion select gir → uygulama otomatik öneri gösteriyor
- Timer countdown çalışıyor, renk urgency doğru
- < 500ms champ-select event → öneri latency hedefi

---

### Sprint 4 — Build/Rune Scraper + UI Polish + Sentry (~4-5 akşam)

**Hedef:** Gerçek Lolalytics/op.gg build/rune verileri, production-ready uygulama.

**Görevler:**
1. `src-tauri/src/scraper/lolalytics.rs` — reqwest + scraper crate ile build/rune veri çekimi
2. Cron-style refresh: her ~2 saatte bir arka plan task
3. `BuildSummary` — gerçek item ikonları (Data Dragon CDN)
4. `EnemyPanel` — counter highlight (düşman champ'a göre öneri sırası değişir)
5. `DataBadge` — "düşük veri", "yeni patch (güncellenmiyor)" uyarıları
6. Sentry entegrasyonu (Tauri crash telemetrisi)
7. Error boundaries + loading state polish
8. `lol-security` audit — Riot ToS uyumu, credential audit
9. Tauri updater config (otomatik güncelleme)
10. `pnpm tauri build` — installer üretimi, son test

**Agent ekibi:** `lol-rust-dev` (scraper) + `lol-frontend` (polish) paralel → `lol-security` + `lol-reviewer` paralel → `lol-perf` (final benchmark)

---

## Tauri Commands — Tam Liste

| Command | Sprint | Modül |
|---------|--------|-------|
| `connect_lcu` | 1 (MEVCUT) | lcu |
| `get_lcu_status` | 1 (MEVCUT) | lcu |
| `get_ddragon_version` | 1.5 | ddragon |
| `sync_ddragon` | 1.5 | ddragon |
| `get_settings` | 1.5 | settings |
| `set_settings` | 1.5 | settings |
| `sync_riot_player` | 2 | riot |
| `sync_match_history` | 2 | riot |
| `sync_masteries` | 2 | riot |
| `get_player_champion_stats` | 2 | db |
| `start_champ_select_watch` | 3 | lcu/ws |
| `get_champ_select_session` | 3 | lcu |
| `get_recommendations` | 3 | recommendation |
| `get_build` | 4 | db |
| `get_runes` | 4 | db |
| `trigger_meta_scrape` | 4 | scraper |

---

## Performance Hedefleri

| Metrik | Hedef |
|--------|-------|
| Champ-select event → öneri | **< 500ms** |
| Uygulama cold start | < 2s |
| Data Dragon ilk indirme | < 10s (arka planda) |
| DB sorgusu (recommendations) | < 50ms |
| Memory (Tauri process) | < 80MB |

---

## Sprint 1.5 Swarm Kickoff (Hazır)

Şu komutu çalıştırdığında 5-agent pipeline başlar:

```javascript
// HEPSİ TEK MESAJDA — lol-architect önce tasarım gönderir
Agent({ name: "lol-architect", subagent_type: "system-architect", run_in_background: true,
  prompt: "Sprint 1.5 başlat: Cargo.toml + errors.rs + AppState değişikliklerini tasarla. SendMessage 'lol-rust-dev'e ilet." })

Agent({ name: "lol-rust-dev",  subagent_type: "rust-pro",  run_in_background: true,
  prompt: "lol-architect'ten tasarımı bekle. db/ + ddragon/ modüllerini + migration runner'ı yaz. SendMessage 'lol-tester'a ilet." })

Agent({ name: "lol-ts-dev",    subagent_type: "typescript-pro", run_in_background: true,
  prompt: "lol-architect'ten tasarımı bekle. AppShell + DisconnectedView + LobbyView temelini yaz. SendMessage 'lol-tester'a ilet." })

Agent({ name: "lol-tester",    subagent_type: "tester",    run_in_background: true,
  prompt: "lol-rust-dev + lol-ts-dev'den tamamlanma sinyali bekle. Migration + ddragon testlerini yaz. SendMessage 'lol-reviewer'a." })

Agent({ name: "lol-reviewer",  subagent_type: "reviewer",  run_in_background: true,
  prompt: "lol-tester'dan sinyali bekle. Kod kalitesi + security audit. Lead'e raporla." })

SendMessage({ to: "lol-architect", summary: "Sprint 1.5 başlat",
  message: "Plan v2: docs/sprint-plan-v2.md. Mevcut kod: src-tauri/src/lcu/. Migration SQL: docs/sprint-plan-v2.md#migration-dosyalari." })
```

Sprint 1.5'i başlatmak için "swarm başlat" de.
