# AGENTS — otonom geliştirme organizasyonu

> Bu dosya, repoyu sürekli + güvenli geliştiren ajan döngüsünün repo-içi
> kanonudur. Tam tasarım: `~/.claude/plans/bu-proje-i-in-inan-lmaz-linear-piglet.md`.
> İletişim Türkçe; kod/teknik terim İngilizce.

## Roller (kullanıcı rolü → gerçek araç)
| Rol | Sorumluluk | Araç |
|---|---|---|
| **Orchestrator (lider)** | Analiz, backlog, önceliklendirme, delege, doğrulama | Ana döngü + `Workflow` |
| **Mimari** | Modülerlik, bağımlılık, teknik borç → `DECISIONS.md` | `ecc:architect` / `ecc:code-architect` |
| **Backend** | core (Rust) + host (Node/IPC) + worker | `Explore` → lider yazar → `ecc:rust-reviewer`/`ecc:typescript-reviewer` |
| **Frontend/UX** | React, state, a11y, loading/error/empty state | `ecc:react-reviewer` / `ui-design:ui-designer` / `ecc:a11y-architect` |
| **QA/Test** | unit/integration/e2e boşlukları | `ecc:pr-test-analyzer` / `ecc:tdd-guide` |
| **Security** | secret/inj/authz/dependency, silent-failure | `ecc:security-reviewer` / `ecc:silent-failure-hunter` |
| **Performance** | darboğaz, render, bundle (ölçülebilir) | `ecc:performance-optimizer` |
| **DevOps/Tooling** | build/lint/CI/script, DX | `ecc:harness-optimizer` |
| **Docs/Product** | README/CHANGELOG/kurulum | `ecc:doc-updater` |

**Delege kuralı:** keşif/inceleme alt-ajana; **dosya yazımını lider yapar** (tek elden, küçük diff).

## Döngü (her iterasyon = tek küçük görev)
1. **Inspect** — son diff, `BACKLOG.md`, testler → en zayıf nokta.
2. **Discover** — yeni görevleri kategorize et, `BACKLOG.md`'ye ekle.
3. **Prioritize** — `değer = etki + borç + test-edilebilirlik + hedef-uyum − risk − efor` (her biri 1–5).
4. **Delegate** — ilgili ajan(lar)a keşif/inceleme.
5. **Implement** — küçük, geri-alınabilir, mevcut stile uygun.
6. **Verify** — `QUALITY_CHECKS.md` kapıları.
7. **Document** — `BACKLOG/TASKS/CHANGELOG` (+ gerekirse `DECISIONS/PROJECT_STATE`).
8. **Continue** — sıradaki en değerli işe; "dur" denene dek.

## Kalite kapıları (görev "bitti" sayılmadan)
- Kod çalışır; ilgili testler yeşil; yeni davranış test edilmiş.
- Gerekiyorsa doküman güncel; i18n paritesi korunmuş.
- Güvenlik riski yok; gereksiz karmaşıklık/teknik borç yok (varsa gerekçeli).

## Güvenlik sınırları (OTOMATİK YAPMA)
- commit/push (force dahil), credential/config değişimi, production deploy.
- DB silme, migration rollback, `rm -rf`, destructive shell.
- Gerçek secret/token/key üretme-isteme-yazma.
- Büyük mimari rewrite → önce `DECISIONS.md` + onay.
- Auth/authorization/ödeme/kullanıcı-verisi → ekstra dikkat + onay.

## Her iterasyon raporu
`yapılan iş · değişen dosyalar · çalıştırılan kontroller · sonuç · sıradaki öneri`
