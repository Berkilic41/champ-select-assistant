# champ-select-assistant — Agent Ekibi

> Referans: `~/.claude/agents/lol/` (wshobson/agents kaynaklı)

## Çekirdek Ekip (Her Sprint Aktif)

| Rol (name) | Agent Type | Model | Sorumluluk | Tipik Görev |
|------------|------------|-------|------------|-------------|
| **lol-architect** | `system-architect` | Opus | Modül sınırları, API kontratları, trait tanımları | LCU/Riot client ayrımı, recommendation engine şeması |
| **lol-rust-dev** | `rust-pro` | Sonnet | `src-tauri/src/` Rust implementasyonu | LCU WebSocket, SQLite repo, rate limiter, Tauri commands |
| **lol-ts-dev** | `typescript-pro` | Sonnet | `src/` React/TS, ts-rs type bridge | UI state, IPC types, shared/types codegen |
| **lol-frontend** | `frontend-developer` | Inherit | UI bileşenleri, performans | Champion grid, öneri kartları, overlay layout |
| **lol-ux** | `ui-designer` | Sonnet | 30 saniyelik champ-select UX | Hiyerarşi, renk, sıralama, accessibility |
| **lol-tester** | `test-automator` | Sonnet | LCU mock, integration test, e2e | Lockfile parser tests, recommendation snapshot |
| **lol-reviewer** | `code-reviewer` | Opus | Kalite + güvenlik audit | PR review, secret leak, unwrap audit |
| **lol-perf** | `performance-engineer` | Opus | Latency, render time, polling | <500ms champ-select event → öneri hedefi |
| **lol-debugger** | `debugger` | Sonnet | Üretim sorunları, cert/lockfile | Windows path edge cases, SSL sorunları |

## İhtisas Ekibi (Sprint'e Göre Aktive)

| Rol (name) | Agent Type | Model | Ne Zaman |
|------------|------------|-------|----------|
| **lol-data-eng** | `data-engineer` | Sonnet | Sprint 1.5-2 — SQLite şema, Riot ETL |
| **lol-ml** | `ml-engineer` | Opus | Sprint 3 — Recommendation engine |
| **lol-security** | `security-auditor` | Opus | Her sprint sonu — ToS uyumu, credential audit |
| **lol-api-docs** | (claude: researcher) | Sonnet | Sprint 2 — LCU undocumented endpoint doc |
| **lol-scraper** | `python-pro` | Sonnet | Sprint 4 — Lolalytics/op.gg scraping |

## Komms Protokolü

### Spawn Kuralı
- Tüm agentler **tek mesajda** `run_in_background: true` ile spawn edilir
- Her prompt'ta **kime SendMessage yapacağı** açıkça belirtilir
- Lead sadece kickoff `SendMessage` atar, ardından agentler kendi aralarında koordine olur
- Status polling YAPILMAZ

### Pipeline Şablonları

**Yeni Feature (standart)**:
```
lol-architect → lol-rust-dev veya lol-ts-dev → lol-tester → lol-reviewer
                        ↘ lol-perf (gerekirse) ↗
```

**Bug / Incident**:
```
lol-debugger → (lol-rust-dev | lol-ts-dev) → lol-tester → lol-reviewer
```

**Veri Modeli Değişikliği**:
```
lol-architect + lol-data-eng (paralel) → lol-rust-dev → lol-tester → lol-reviewer
```

**Security Review (her sprint sonu)**:
```
lol-reviewer + lol-security (paralel) → lol-architect (kararlar)
```

### Spawn Örneği (Sprint 1.5 — SQLite + Data Dragon)

```javascript
// HEPSİ TEK MESAJDA
Agent({
  name: "lol-architect",
  subagent_type: "system-architect",
  run_in_background: true,
  prompt: `champ-select-assistant için SQLite + Data Dragon entegrasyonu modül tasarımı.
Mevcut: src-tauri/src/lcu/ (lockfile, client, mod.rs).
Görev: db/ ve ddragon/ modül trait'lerini, error type'larını, AppState yapısını tasarla.
Tamamlayınca SendMessage ile 'lol-data-eng' ve 'lol-rust-dev'e tasarım dokümanını ilet.`
})

Agent({
  name: "lol-data-eng",
  subagent_type: "data-engineer",
  run_in_background: true,
  prompt: `lol-architect'ten tasarımı bekle.
Görev: refinery SQL migration dosyaları (V001__init.sql, V002__champions.sql, V003__matches.sql).
Tamamlayınca SendMessage ile 'lol-rust-dev'e SQL dosyalarını ilet.`
})

Agent({
  name: "lol-rust-dev",
  subagent_type: "rust-pro",
  run_in_background: true,
  prompt: `lol-architect'ten modül tasarımını, lol-data-eng'den migration SQL'lerini bekle.
Görev: rusqlite + refinery entegrasyonu (src-tauri/src/db/), Data Dragon downloader (src-tauri/src/ddragon/), Cargo.toml güncellemesi.
Tamamlayınca SendMessage ile 'lol-tester'a implementasyonu ilet.`
})

Agent({
  name: "lol-tester",
  subagent_type: "test-automator",
  run_in_background: true,
  prompt: `lol-rust-dev'den implementasyonu bekle.
Görev: db migration testleri + Data Dragon cache testleri.
Tamamlayınca SendMessage ile 'lol-reviewer'a ilet.`
})

Agent({
  name: "lol-reviewer",
  subagent_type: "code-reviewer",
  run_in_background: true,
  prompt: `lol-tester'dan tamamlanma sinyalini bekle.
Görev: kod kalitesi + güvenlik + unwrap audit.
Tamamlayınca sonuçları lead (Claude) konuşma akışına raporla.`
})

// Kickoff
SendMessage({
  to: "lol-architect",
  summary: "Sprint 1.5 Başlat",
  message: "SQLite + Data Dragon entegrasyonu tasarımına başla. Önceki plan: ~/.claude/plans/sen-deneyimli-bir-r-n-zippy-tiger.md"
})
```

## Memory Hijyeni

```bash
# Her sprint başı
npx @claude-flow/cli@latest memory search --query "lol champ-select" --namespace patterns

# Her başarılı feature sonrası
npx @claude-flow/cli@latest memory store --namespace patterns --key "lol-<feature>" --value "<ne işe yaradı>"
```

## Sprint Atama Özeti

| Sprint | Aktif Agentlar |
|--------|---------------|
| 1.5 | architect, data-eng, rust-dev, tester, reviewer |
| 2 | architect, rust-dev, ts-dev, data-eng, tester, reviewer, api-docs |
| 3 | architect, rust-dev, ts-dev, frontend, ux, tester, reviewer, ml, perf |
| 4 | rust-dev, ts-dev, frontend, scraper, tester, reviewer, security |
