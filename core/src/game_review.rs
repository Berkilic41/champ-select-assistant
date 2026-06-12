//! Maç sonu karnesi + odak döngüsü (Faz C1+C2) — "koç döngüsünün" çekirdeği.
//!
//! Saf (I/O yok): incelenen maç + AYNI rol & queue-grubundaki geçmiş maçlar
//! (host filtreler) + önceki açık hedef → notlu karne satırları, "iyi giden /
//! düzeltilecek" okuması, önceki hedefin kontrolü ve SONRAKİ maç için TEK
//! ölçülebilir hedef.
//!
//! Dürüstlük kuralları:
//! - Baseline = oyuncunun KENDİ geçmişinin medyanı (rol+queue içi) — mutlak
//!   eşik yok, rol önyargısı yok (jungle CS'i jungle medyanına kıyaslanır).
//! - Geçmiş < `MIN_BASELINE_GAMES` → baseline yok, hedef yok, `partial: true`.
//! - Timeline metrikleri (`cs_at_10`, `deaths_pre_14`) veri yoksa `locked`
//!   işaretlenir — UI "Riot anahtarıyla açılır" der, sahte 0 üretilmez.
//! - Hedef seçimi = baseline'a göre EN KÖTÜ sapan metrik; hedef değeri
//!   medyanın kendisi (ulaşılabilir, abartısız).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::postgame::MatchRow;

/// Baseline ve hedef üretimi için gereken en az geçmiş maç sayısı.
pub const MIN_BASELINE_GAMES: usize = 5;
/// "even" bandı: baseline'dan ±%8 sapma berabere sayılır.
const EVEN_BAND: f32 = 0.08;

/// Ölçülebilir tek maç hedefi ("sonraki maç odağı").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct FocusGoal {
    /// Makine anahtarı: "cs_per_min" | "deaths_per_10" | "kda" | "vision_score"
    /// | "cs_at_10" | "deaths_pre_14".
    pub metric: String,
    pub target: f32,
    /// "at_least" | "at_most".
    pub direction: String,
    /// Türkçe görünür etiket, örn. "CS/dk ≥ 6.1".
    pub label: String,
}

/// Karnenin tek satırı: metrik değeri vs kişisel baseline.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct ReviewLine {
    pub metric: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<f32>,
    /// "better" | "even" | "worse" | "no_baseline" | "locked".
    pub verdict: String,
}

/// Önceki açık hedefin bu maçtaki sonucu.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct FocusCheck {
    pub metric: String,
    pub target: f32,
    pub direction: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub achieved: Option<f32>,
    /// "met" | "missed" | "no_data".
    pub result: String,
}

/// Maç sonu karnesi.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct GameReview {
    pub champion_id: u32,
    pub champion_key: String,
    pub position: String,
    pub win: bool,
    pub lines: Vec<ReviewLine>,
    /// "İyi giden 1 şey" — Türkçe, ölçülü dil.
    pub went_right: String,
    /// "Düzeltilecek 1 şey" — Türkçe, ölçülü dil.
    pub to_fix: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_check: Option<FocusCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_focus: Option<FocusGoal>,
    /// Geçmiş < MIN_BASELINE_GAMES → baseline'sız, hedefsiz okuma.
    pub partial: bool,
}

/// (anahtar, yön, Türkçe kısa ad) — karne satır sırası da bu.
const METRICS: &[(&str, &str, &str)] = &[
    ("cs_per_min", "at_least", "CS/dk"),
    ("deaths_per_10", "at_most", "10 dk başına ölüm"),
    ("kda", "at_least", "KDA"),
    ("vision_score", "at_least", "Vizyon skoru"),
    ("cs_at_10", "at_least", "CS@10"),
    ("deaths_pre_14", "at_most", "14 dk öncesi ölüm"),
];

pub(crate) fn metric_value(m: &MatchRow, key: &str) -> Option<f32> {
    let mins = m.duration_secs as f32 / 60.0;
    match key {
        "cs_per_min" => m.cs.filter(|_| m.duration_secs > 0).map(|c| c as f32 / mins),
        "deaths_per_10" => (m.duration_secs > 0).then(|| m.deaths as f32 / mins * 10.0),
        "kda" => Some((m.kills + m.assists) as f32 / m.deaths.max(1) as f32),
        "vision_score" => m.vision_score.map(|v| v as f32),
        "cs_at_10" => m.cs_at_10.map(|v| v as f32),
        "deaths_pre_14" => m.deaths_pre_14.map(|v| v as f32),
        _ => None,
    }
}

/// Timeline metriği mi (key'siz kurulumlarda kilitli)?
fn is_timeline_metric(key: &str) -> bool {
    matches!(key, "cs_at_10" | "deaths_pre_14")
}

fn median(values: &mut Vec<f32>) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    })
}

/// `direction` yönünde değerin baseline'a göre işaretli iyilik payı.
/// Pozitif = baseline'dan iyi. `at_most` metriklerinde eksen çevrilir.
fn signed_margin(value: f32, baseline: f32, direction: &str) -> f32 {
    let denom = baseline.abs().max(0.5); // sıfır baseline'da bölme patlamasın
    let raw = (value - baseline) / denom;
    if direction == "at_most" {
        -raw
    } else {
        raw
    }
}

fn verdict_for(margin: f32) -> &'static str {
    if margin > EVEN_BAND {
        "better"
    } else if margin < -EVEN_BAND {
        "worse"
    } else {
        "even"
    }
}

fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

fn goal_label(short: &str, direction: &str, target: f32) -> String {
    let sym = if direction == "at_most" { "≤" } else { "≥" };
    format!("{short} {sym} {}", round1(target))
}

/// Karneyi üret. `history` = incelenen maçtan ÖNCEKİ, aynı rol + queue-grubu
/// maçları (host filtreler; sıra önemsiz). `prev_goal` = bu queue-grubundaki
/// açık hedef (varsa) — bu maçta kontrol edilir.
pub fn build_game_review(
    reviewed: &MatchRow,
    history: &[MatchRow],
    prev_goal: Option<&FocusGoal>,
) -> GameReview {
    let partial = history.len() < MIN_BASELINE_GAMES;

    // Satırlar: değer + (yeterli geçmişte) medyan baseline + hüküm.
    let mut lines = Vec::new();
    let mut margins: Vec<(usize, f32)> = Vec::new(); // (METRICS idx, signed margin)
    for (idx, (key, direction, _short)) in METRICS.iter().enumerate() {
        let value = metric_value(reviewed, key);
        let baseline = if partial {
            None
        } else {
            let mut vals: Vec<f32> =
                history.iter().filter_map(|m| metric_value(m, key)).collect();
            // Baseline da en az MIN sayıda gerçek değer ister (örn. vision'sız
            // eski satırlar medyanı çarpıtmasın).
            if vals.len() >= MIN_BASELINE_GAMES {
                median(&mut vals)
            } else {
                None
            }
        };
        let verdict = match (value, baseline) {
            (None, _) if is_timeline_metric(key) => "locked",
            (None, _) => "no_baseline",
            (Some(_), None) => "no_baseline",
            (Some(v), Some(b)) => {
                let m = signed_margin(v, b, direction);
                margins.push((idx, m));
                verdict_for(m)
            }
        };
        lines.push(ReviewLine {
            metric: key.to_string(),
            value: value.map(round1),
            baseline: baseline.map(round1),
            verdict: verdict.to_string(),
        });
    }

    // İyi giden / düzeltilecek: en iyi ve en kötü işaretli pay. Ölçülü dil,
    // garanti dili yok (coach_quality kuralları).
    let best = margins
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let worst = margins
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let went_right = match best {
        Some(&(idx, m)) if m > EVEN_BAND => {
            let (_, _, short) = METRICS[idx];
            format!("{short} kendi ortalamanın üzerindeydi — bunu korumaya çalış.")
        }
        _ if reviewed.win => "Maç kazanıldı — net bir metrik sıçraması olmasa da sonuç lehine.".to_string(),
        _ => "Bu maçta öne çıkan bir metrik yok; bir sonrakine temiz başla.".to_string(),
    };
    let to_fix = match worst {
        Some(&(idx, m)) if m < -EVEN_BAND => {
            let (_, _, short) = METRICS[idx];
            format!("{short} kendi ortalamanın altında kaldı — bir sonraki maçın odağı bu olabilir.")
        }
        _ if partial => "Karşılaştırma için henüz yeterli geçmiş yok — birkaç maç sonra netleşir.".to_string(),
        _ => "Belirgin bir zayıf metrik yok — istikrarı sürdür.".to_string(),
    };

    // Önceki hedef kontrolü.
    let focus_check = prev_goal.map(|g| {
        let achieved = metric_value(reviewed, &g.metric);
        let result = match achieved {
            None => "no_data",
            Some(v) => {
                let met = if g.direction == "at_most" { v <= g.target } else { v >= g.target };
                if met {
                    "met"
                } else {
                    "missed"
                }
            }
        };
        FocusCheck {
            metric: g.metric.clone(),
            target: g.target,
            direction: g.direction.clone(),
            label: g.label.clone(),
            achieved: achieved.map(round1),
            result: result.to_string(),
        }
    });

    // Sonraki hedef: baseline'a göre en kötü sapan metrik; hedef = medyan.
    let next_focus = if partial {
        None
    } else {
        worst.map(|&(idx, _)| {
            let (key, direction, short) = METRICS[idx];
            let target = lines[idx].baseline.unwrap_or(0.0);
            FocusGoal {
                metric: key.to_string(),
                target,
                direction: direction.to_string(),
                label: goal_label(short, direction, target),
            }
        })
    };

    GameReview {
        champion_id: reviewed.champion_id,
        champion_key: reviewed.champion_key.clone(),
        position: reviewed.position.clone(),
        win: reviewed.win,
        lines,
        went_right,
        to_fix,
        focus_check,
        next_focus,
        partial,
    }
}

// ── D1: Kişisel form sinyali ──────────────────────────────────────────────────

/// Şampiyon-başına form girdisi: rol baseline'ına karşı deltalar.
#[derive(Debug, Clone, PartialEq)]
pub struct LaneFormEntry {
    /// CS/dk: şampiyondaki ortalaman − rol baseline'ın (pozitif = iyi).
    pub cs_delta: f32,
    /// 10 dk başına ölüm: baseline − şampiyondaki ortalaman (pozitif = iyi).
    pub deaths_delta: f32,
    pub games: u32,
}

/// Form skoru için prior (comfort'un prior_n=5 deseniyle aynı).
const FORM_PRIOR_N: f32 = 5.0;
/// Delta'ların doyduğu bant: CS/dk için ±2.0, ölüm/10dk için ±2.0.
const FORM_BAND: f32 = 2.0;

/// `recent_matches`'ten (host AYNI queue-grubunu yollar) draft'ın rolündeki
/// şampiyon-başına form haritası. Rol baseline'ı = o roldeki TÜM maçların
/// medyanı; şampiyon değeri = o şampiyondaki ortalama. Rolde maç yoksa boş.
pub fn build_lane_form(matches: &[MatchRow], my_pos: &str) -> std::collections::HashMap<u32, LaneFormEntry> {
    use std::collections::HashMap;
    let mut out = HashMap::new();
    if my_pos.is_empty() || my_pos == "aram" {
        return out; // form rol-bazlı; ARAM/bilinmeyen pozisyonda sinyal yok
    }
    let role_rows: Vec<&MatchRow> = matches
        .iter()
        .filter(|m| m.position.eq_ignore_ascii_case(my_pos))
        .collect();
    let mut cs_all: Vec<f32> = role_rows
        .iter()
        .filter_map(|m| metric_value(m, "cs_per_min"))
        .collect();
    let mut deaths_all: Vec<f32> = role_rows
        .iter()
        .filter_map(|m| metric_value(m, "deaths_per_10"))
        .collect();
    let (Some(cs_base), Some(deaths_base)) = (median(&mut cs_all), median(&mut deaths_all))
    else {
        return out;
    };

    let mut per_champ: std::collections::HashMap<u32, (f32, f32, u32)> = HashMap::new();
    for m in &role_rows {
        let (Some(cs), Some(d)) = (
            metric_value(m, "cs_per_min"),
            metric_value(m, "deaths_per_10"),
        ) else {
            continue;
        };
        let e = per_champ.entry(m.champion_id).or_insert((0.0, 0.0, 0));
        e.0 += cs;
        e.1 += d;
        e.2 += 1;
    }
    for (id, (cs_sum, d_sum, n)) in per_champ {
        if n == 0 {
            continue;
        }
        out.insert(
            id,
            LaneFormEntry {
                cs_delta: cs_sum / n as f32 - cs_base,
                deaths_delta: deaths_base - d_sum / n as f32,
                games: n,
            },
        );
    }
    out
}

/// 0..1 form skoru (0.5 = baseline'da). Delta'lar ±FORM_BAND'da doyar; az maç
/// prior'la 0.5'e çekilir — 1 iyi maç "formda" demek için yetmez.
pub fn lane_form_score(entry: &LaneFormEntry) -> f32 {
    let cs = (entry.cs_delta / FORM_BAND).clamp(-1.0, 1.0);
    let d = (entry.deaths_delta / FORM_BAND).clamp(-1.0, 1.0);
    let raw = 0.5 + 0.25 * cs + 0.25 * d;
    let n = entry.games as f32;
    ((raw * n) + 0.5 * FORM_PRIOR_N) / (n + FORM_PRIOR_N)
}

// ── F1: Seans okuması (tilt koruması) ─────────────────────────────────────────

/// Bu oturumda (app açılışından beri) oynanan maçların seans-düzeyi okuması.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct SessionRead {
    pub games: u32,
    pub wins: u32,
    pub losses: u32,
    /// En son maçtan geriye ardışık kayıp.
    pub loss_streak: u32,
    /// "ok" | "caution" (2 ardışık kayıp) | "break" (3+ — ara öner).
    pub verdict: String,
    /// Ölçülü Türkçe seans notu (caution/break'te dolu).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Seans okuması. `session_matches` = bu oturumda oynanan maçlar (host
/// `played_at >= boot` ile filtreler); sıra önemsiz, içeride yeniye göre
/// sıralanır. Karar oyuncuya kalır — kart yalnız ÖNERİR (zorlamaz).
pub fn build_session_read(session_matches: &[MatchRow]) -> SessionRead {
    let mut sorted: Vec<&MatchRow> = session_matches.iter().collect();
    sorted.sort_by_key(|m| std::cmp::Reverse(m.played_at));

    let games = sorted.len() as u32;
    let wins = sorted.iter().filter(|m| m.win).count() as u32;
    let losses = games - wins;
    let mut loss_streak = 0u32;
    for m in &sorted {
        if m.win {
            break;
        }
        loss_streak += 1;
    }

    let (verdict, note) = if loss_streak >= 3 {
        (
            "break",
            Some(format!(
                "Bu oturumda üst üste {loss_streak} kayıp — 15 dakikalık bir ara çoğu oyuncuda seriyi keser. Karar senin."
            )),
        )
    } else if loss_streak == 2 {
        (
            "caution",
            Some(
                "Üst üste 2 kayıp — bir sonraki maçtan önce kısa bir nefes ve net bir plan iyi gelir."
                    .to_string(),
            ),
        )
    } else {
        ("ok", None)
    };

    SessionRead {
        games,
        wins,
        losses,
        loss_streak,
        verdict: verdict.to_string(),
        note,
    }
}

// ── C4: Trend raporu ──────────────────────────────────────────────────────────

/// Sparkline noktası (eski→yeni sırada).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct TrendPoint {
    pub played_at: i64,
    pub win: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cs_per_min: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deaths_per_10: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vision_score: Option<f32>,
    pub kda: f32,
}

/// Metrik başına yön hükmü: ilk yarı medyanı vs ikinci yarı medyanı.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct TrendVerdict {
    pub metric: String,
    /// "improving" | "flat" | "declining".
    pub direction: String,
    pub first_half: f32,
    pub second_half: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct TrendReport {
    pub points: Vec<TrendPoint>,
    pub verdicts: Vec<TrendVerdict>,
    /// < `TREND_MIN_GAMES` maçta hüküm üretilmez.
    pub partial: bool,
}

/// Yön hükmü için gereken en az maç (iki yarıda da ≥4 değer).
pub const TREND_MIN_GAMES: usize = 8;

/// Trend hükmüne giren metrikler (win_rate ayrıca hesaplanır).
const TREND_METRICS: &[(&str, &str)] = &[
    ("cs_per_min", "at_least"),
    ("deaths_per_10", "at_most"),
    ("vision_score", "at_least"),
    ("kda", "at_least"),
];

fn half_median(matches: &[MatchRow], key: &str) -> Option<f32> {
    let mut vals: Vec<f32> = matches.iter().filter_map(|m| metric_value(m, key)).collect();
    if vals.len() < 4 {
        return None;
    }
    median(&mut vals)
}

/// Trend raporu. `matches` host tarafından AYNI rol + queue-grubuyla
/// filtrelenmiş olmalı; sıra önemsiz (içeride eski→yeni sıralanır).
pub fn build_trend_report(matches: &[MatchRow]) -> TrendReport {
    let mut sorted: Vec<&MatchRow> = matches.iter().collect();
    sorted.sort_by_key(|m| m.played_at);

    let points: Vec<TrendPoint> = sorted
        .iter()
        .map(|m| TrendPoint {
            played_at: m.played_at,
            win: m.win,
            cs_per_min: metric_value(m, "cs_per_min").map(round1),
            deaths_per_10: metric_value(m, "deaths_per_10").map(round1),
            vision_score: metric_value(m, "vision_score").map(round1),
            kda: round1(metric_value(m, "kda").unwrap_or(0.0)),
        })
        .collect();

    let partial = sorted.len() < TREND_MIN_GAMES;
    let mut verdicts = Vec::new();
    if !partial {
        let mid = sorted.len() / 2;
        let (first, second): (Vec<MatchRow>, Vec<MatchRow>) = (
            sorted[..mid].iter().map(|m| (*m).clone()).collect(),
            sorted[mid..].iter().map(|m| (*m).clone()).collect(),
        );
        for (key, direction) in TREND_METRICS {
            if let (Some(a), Some(b)) = (half_median(&first, key), half_median(&second, key)) {
                let m = signed_margin(b, a, direction);
                let dir = if m > EVEN_BAND {
                    "improving"
                } else if m < -EVEN_BAND {
                    "declining"
                } else {
                    "flat"
                };
                verdicts.push(TrendVerdict {
                    metric: key.to_string(),
                    direction: dir.to_string(),
                    first_half: round1(a),
                    second_half: round1(b),
                });
            }
        }
        // Win-rate hükmü (yarı başına kazanma oranı).
        let wr = |half: &[MatchRow]| -> f32 {
            half.iter().filter(|m| m.win).count() as f32 / half.len().max(1) as f32
        };
        let (a, b) = (wr(&first), wr(&second));
        let m = signed_margin(b, a.max(0.05), "at_least");
        verdicts.push(TrendVerdict {
            metric: "win_rate".to_string(),
            direction: if m > EVEN_BAND {
                "improving".to_string()
            } else if m < -EVEN_BAND {
                "declining".to_string()
            } else {
                "flat".to_string()
            },
            first_half: round1(a * 100.0),
            second_half: round1(b * 100.0),
        });
    }

    TrendReport { points, verdicts, partial }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cs: u32, deaths: u32, vision: u32, win: bool) -> MatchRow {
        MatchRow {
            champion_id: 86,
            champion_key: "Garen".into(),
            position: "top".into(),
            win,
            kills: 5,
            deaths,
            assists: 5,
            played_at: 0,
            duration_secs: 1800, // 30 dk
            cs: Some(cs),
            cs_at_10: None,
            deaths_pre_14: None,
            vision_score: Some(vision),
        }
    }

    fn history() -> Vec<MatchRow> {
        // CS/dk medyanı: [150,165,180,195,210]/30dk → 6.0; vision medyanı 18.
        vec![
            row(150, 4, 14, true),
            row(165, 5, 16, false),
            row(180, 4, 18, true),
            row(195, 6, 20, false),
            row(210, 5, 22, true),
        ]
    }

    #[test]
    fn baselines_are_personal_medians_and_verdicts_compare_against_them() {
        let reviewed = row(240, 2, 25, true); // 8.0 CS/dk, az ölüm, yüksek vizyon
        let review = build_game_review(&reviewed, &history(), None);
        assert!(!review.partial);

        let cs = review.lines.iter().find(|l| l.metric == "cs_per_min").unwrap();
        assert_eq!(cs.baseline, Some(6.0));
        assert_eq!(cs.value, Some(8.0));
        assert_eq!(cs.verdict, "better");

        let deaths = review.lines.iter().find(|l| l.metric == "deaths_per_10").unwrap();
        assert_eq!(deaths.verdict, "better"); // at_most ekseni çevrilir

        // Timeline metrikleri key'siz kurulumda dürüstçe kilitli.
        let locked = review.lines.iter().find(|l| l.metric == "cs_at_10").unwrap();
        assert_eq!(locked.verdict, "locked");
        assert!(review.went_right.contains("ortalamanın üzerinde"));
    }

    #[test]
    fn worst_metric_becomes_the_next_focus_with_median_target() {
        let reviewed = row(120, 9, 19, false); // CS/dk 4.0 → en kötü sapma
        let review = build_game_review(&reviewed, &history(), None);
        let goal = review.next_focus.expect("yeterli geçmişte hedef üretilir");
        // 4.0/6.0 CS sapması, 3.0/1.67 ölüm sapmasından daha mı kötü? Ölüm:
        // (3.0-1.67)/1.67 ≈ -0.80; CS: (4.0-6.0)/6.0 ≈ -0.33 → ölüm daha kötü.
        assert_eq!(goal.metric, "deaths_per_10");
        assert_eq!(goal.direction, "at_most");
        assert!(goal.label.contains("≤"));
        assert!(review.to_fix.contains("ölüm"));
    }

    #[test]
    fn focus_check_met_and_missed() {
        let goal = FocusGoal {
            metric: "vision_score".into(),
            target: 18.0,
            direction: "at_least".into(),
            label: "Vizyon skoru ≥ 18".into(),
        };
        let good = build_game_review(&row(180, 5, 21, true), &history(), Some(&goal));
        assert_eq!(good.focus_check.as_ref().unwrap().result, "met");
        assert_eq!(good.focus_check.as_ref().unwrap().achieved, Some(21.0));

        let bad = build_game_review(&row(180, 5, 12, false), &history(), Some(&goal));
        assert_eq!(bad.focus_check.as_ref().unwrap().result, "missed");

        // Hedef metriği bu maçta ölçülemiyorsa dürüst no_data.
        let tl_goal = FocusGoal {
            metric: "cs_at_10".into(),
            target: 70.0,
            direction: "at_least".into(),
            label: "CS@10 ≥ 70".into(),
        };
        let nd = build_game_review(&row(180, 5, 21, true), &history(), Some(&tl_goal));
        assert_eq!(nd.focus_check.as_ref().unwrap().result, "no_data");
    }

    #[test]
    fn session_read_escalates_with_consecutive_losses() {
        let m = |win: bool, at: i64| {
            let mut r = row(180, 5, 18, win);
            r.played_at = at;
            r
        };
        // Boş oturum → ok.
        assert_eq!(build_session_read(&[]).verdict, "ok");
        // 1 kayıp → ok; 2 ardışık → caution; 3 ardışık → break (ara öner).
        assert_eq!(build_session_read(&[m(false, 1)]).verdict, "ok");
        let two = build_session_read(&[m(true, 1), m(false, 2), m(false, 3)]);
        assert_eq!(two.verdict, "caution");
        assert_eq!(two.loss_streak, 2);
        let three = build_session_read(&[m(false, 1), m(false, 2), m(false, 3)]);
        assert_eq!(three.verdict, "break");
        assert!(three.note.as_deref().unwrap_or("").contains("ara"));
        // Galibiyet seriyi keser (en yeni maç galibiyet → ok).
        let reset = build_session_read(&[m(false, 1), m(false, 2), m(true, 3)]);
        assert_eq!(reset.verdict, "ok");
        assert_eq!(reset.loss_streak, 0);
        assert_eq!(reset.wins, 1);
        assert_eq!(reset.losses, 2);
    }

    #[test]
    fn lane_form_builds_role_deltas_and_shrinks_small_samples() {
        // top rolünde 6 maç: Garen(86) 3 maç güçlü CS, sahte 99 3 maç zayıf.
        let mut matches = history(); // 5 Garen maçı, cs 150..210 (6.0 medyan civarı)
        let mut weak = row(105, 8, 10, false); // 3.5 cs/dk, çok ölüm
        weak.champion_id = 99;
        weak.champion_key = "Weak".into();
        matches.push(weak.clone());
        matches.push(weak.clone());

        let form = build_lane_form(&matches, "top");
        let garen = form.get(&86).expect("Garen formu var");
        let weak_e = form.get(&99).expect("99 formu var");
        assert!(garen.cs_delta > weak_e.cs_delta);
        assert!(garen.deaths_delta > weak_e.deaths_delta);

        // Skor: iyi form > 0.5 > kötü form; prior az maçı nötre çeker.
        let g = lane_form_score(garen);
        let w = lane_form_score(weak_e);
        assert!(g > 0.5, "iyi form 0.5 üstü: {g}");
        assert!(w < 0.5, "kötü form 0.5 altı: {w}");
        let one_game = LaneFormEntry { cs_delta: 2.0, deaths_delta: 2.0, games: 1 };
        let many = LaneFormEntry { cs_delta: 2.0, deaths_delta: 2.0, games: 20 };
        assert!(lane_form_score(&one_game) < lane_form_score(&many));

        // ARAM / boş pozisyon → sinyal yok (dürüst).
        assert!(build_lane_form(&matches, "aram").is_empty());
        assert!(build_lane_form(&matches, "").is_empty());
    }

    #[test]
    fn trend_report_compares_half_medians_and_is_honest_when_thin() {
        // İlk 4 maç zayıf CS (4.0), son 4 maç güçlü CS (7.0) → improving.
        let mut matches: Vec<MatchRow> = Vec::new();
        for i in 0..4 {
            let mut m = row(120, 6, 14, false); // 4.0 cs/dk
            m.played_at = i;
            matches.push(m);
        }
        for i in 4..8 {
            let mut m = row(210, 3, 22, true); // 7.0 cs/dk
            m.played_at = i;
            matches.push(m);
        }
        let report = build_trend_report(&matches);
        assert!(!report.partial);
        assert_eq!(report.points.len(), 8);
        let cs = report.verdicts.iter().find(|v| v.metric == "cs_per_min").unwrap();
        assert_eq!(cs.direction, "improving");
        let deaths = report.verdicts.iter().find(|v| v.metric == "deaths_per_10").unwrap();
        assert_eq!(deaths.direction, "improving"); // 6→3 ölüm, at_most ekseni
        let wr = report.verdicts.iter().find(|v| v.metric == "win_rate").unwrap();
        assert_eq!(wr.direction, "improving");

        // < 8 maç → hüküm yok, partial true (sparkline noktaları yine döner).
        let thin = build_trend_report(&matches[..5]);
        assert!(thin.partial);
        assert!(thin.verdicts.is_empty());
        assert_eq!(thin.points.len(), 5);
    }

    #[test]
    fn thin_history_is_partial_with_no_goal_and_no_baselines() {
        let review = build_game_review(&row(200, 3, 20, true), &history()[..3].to_vec(), None);
        assert!(review.partial);
        assert!(review.next_focus.is_none());
        assert!(review.lines.iter().all(|l| l.baseline.is_none()));
        assert!(review.to_fix.contains("yeterli geçmiş yok"));
    }
}
