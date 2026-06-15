//! FAZ 3 / Sprint 3B — learned ModelPack training.
//!
//! Yerel outcome etiketlerinden (pick anındaki per-signal feature vektörü →
//! gerçek win/loss) ModelPack lineer ağırlıklarını öğrenir. MUHAFAZAKÂR + GATE'li:
//!   - global min-sample altında `None` döner (→ host rules pack'e düşer).
//!   - ridge regression PRIOR'a (kural ağırlıkları) çekilir; λ büyük olduğundan
//!     az veride ağırlıklar prior'a yakın kalır, veri arttıkça öğrenilen sinyal
//!     baskınlaşır (adaptif muhafazakârlık).
//!   - her ağırlık prior'dan ±MAX_DRIFT'ten fazla sapamaz (clamp; işaret korunur).
//!   - 6 pozitif ağırlık prior'ın pozitif-toplamına yeniden normalize edilir →
//!     model_score ölçeği SABİT kalır (Tier eşikleri + breakdown UI tutarlı).
//!     Net etki: öğrenilen pack sinyallerin GÖRECELİ önemini ayarlar, toplam
//!     ölçeği değil.
//!
//! Determinist motor hâlâ oracle: ModelPack yalnız ağırlık katsayısıdır; üretilen
//! her öneri mevcut audit/fallback hattından aynen geçer (apply_model_score).

use serde::Deserialize;

use super::draft_brain::ModelPack;

/// Eğitim için global asgari etiketli maç — altında `None` (rules fallback).
/// 40 düşük gibi görünse de güvenlik yığını agresif: RIDGE_LAMBDA prior'ı baskın
/// tutar, ±MAX_DRIFT clamp sert sınır, BLEND_TO_PRIOR son harman kural ağırlığına
/// ≥%50 pay verir → 40'ta bile öğrenilen sinyal yalnız küçük, sınırlı bir eğme.
const MIN_TRAINING_GAMES: usize = 40;
/// Ridge'in prior'a çekme gücü (büyük = muhafazakâr; az veride prior baskın).
const RIDGE_LAMBDA: f32 = 12.0;
/// Bir ağırlık prior'dan en çok ±%50 sapabilir (clamp; sert güvenlik kalkanı).
const MAX_DRIFT: f32 = 0.5;
/// ORACLE GÜVENCESİ: nihai ağırlık = BLEND·öğrenilen + (1-BLEND)·prior. Kural
/// ağırlıkları her zaman ≥(1-BLEND)=%50 paya sahip → öğrenilen sinyal determinist
/// motoru asla sessizce EZMEZ, yalnız muhafazakâr biçimde eğer.
const BLEND_TO_PRIOR: f32 = 0.5;
/// apply_model_score'un kullandığı sinyaller (6 pozitif + risk penaltısı).
const N_SIGNALS: usize = 7;
/// Öğrenilen pack sürümü (rules pack'ten ayırt edilir).
pub const LEARNED_MODEL_PACK_VERSION: &str = "draft-brain-learned-v1";

/// Prior pozitif ağırlık varsayılanları (local_rules_model_pack ile aynı).
const PRIOR_DEFAULT: [f32; N_SIGNALS] = [0.17, 0.18, 0.14, 0.13, 0.12, 0.12, 0.08];
const SIGNAL_KEYS: [&str; N_SIGNALS] = [
    "comfort",
    "matchup",
    "team_counter",
    "synergy",
    "meta",
    "role_fit",
    "risk",
];

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TrainingFeatures {
    #[serde(default)]
    pub comfort: f32,
    #[serde(default)]
    pub matchup: f32,
    #[serde(default)]
    pub team_counter: f32,
    #[serde(default)]
    pub synergy: f32,
    #[serde(default)]
    pub meta: f32,
    #[serde(default)]
    pub role_fit: f32,
    #[serde(default)]
    pub risk: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrainingExample {
    pub features: TrainingFeatures,
    pub won: bool,
}

/// Tasarım satırı: apply_model_score = Σ pozitif·w − risk·w_risk. risk sütunu
/// `-risk` olarak konur → fitlenen katsayı doğrudan w_risk (pozitif penaltı).
fn design_row(f: &TrainingFeatures) -> [f32; N_SIGNALS] {
    [
        f.comfort,
        f.matchup,
        f.team_counter,
        f.synergy,
        f.meta,
        f.role_fit,
        -f.risk,
    ]
}

fn prior_vector(prior: &ModelPack) -> [f32; N_SIGNALS] {
    let mut p = PRIOR_DEFAULT;
    for (i, key) in SIGNAL_KEYS.iter().enumerate() {
        if let Some(&w) = prior.weights.get(*key) {
            p[i] = w;
        }
    }
    p
}

/// Gauss-Jordan + kısmi pivot ile A·x = b çöz (A = XᵀX + λI SPD → tersinir).
fn solve(
    mut a: [[f32; N_SIGNALS]; N_SIGNALS],
    mut b: [f32; N_SIGNALS],
) -> Option<[f32; N_SIGNALS]> {
    for col in 0..N_SIGNALS {
        let mut pivot = col;
        for row in (col + 1)..N_SIGNALS {
            if a[row][col].abs() > a[pivot][col].abs() {
                pivot = row;
            }
        }
        if a[pivot][col].abs() < 1e-9 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let pivot_row = a[col]; // Copy — eleme sırasında pivot satırı değişmez.
        let pivot_diag = pivot_row[col];
        let pivot_b = b[col];
        for row in 0..N_SIGNALS {
            if row == col {
                continue;
            }
            let factor = a[row][col] / pivot_diag;
            for (c, target) in a[row].iter_mut().enumerate().skip(col) {
                *target -= factor * pivot_row[c];
            }
            b[row] -= factor * pivot_b;
        }
    }
    let mut x = [0.0f32; N_SIGNALS];
    for i in 0..N_SIGNALS {
        x[i] = b[i] / a[i][i];
    }
    Some(x)
}

/// 6 pozitif ağırlığı, her birini ±MAX_DRIFT bandında tutarak `target` toplamına
/// taşı (bounded water-filling). Eksik/fazla, kalan "boşluğu" oranında dağıtılır;
/// birkaç iterasyonda yakınsar. Tüm ağırlıklar sınırdaysa ulaşılabilir en yakına
/// kalır (sert drift sınırı her zaman korunur).
fn redistribute_to_sum(w: &mut [f32; N_SIGNALS], p: &[f32; N_SIGNALS], target: f32) {
    let room = |i: usize, up: bool, w: &[f32; N_SIGNALS]| -> f32 {
        let r = if up {
            p[i] * (1.0 + MAX_DRIFT) - w[i]
        } else {
            w[i] - p[i] * (1.0 - MAX_DRIFT)
        };
        r.max(0.0)
    };
    for _ in 0..12 {
        let sum: f32 = w[..6].iter().sum();
        let diff = target - sum;
        if diff.abs() < 1e-5 {
            return;
        }
        let up = diff > 0.0;
        let total_room: f32 = (0..6).map(|i| room(i, up, w)).sum();
        if total_room < 1e-6 {
            return; // tüm pozitifler sınırda — daha fazla taşınamaz
        }
        for i in 0..6 {
            w[i] += diff * (room(i, up, w) / total_room);
            w[i] = w[i].clamp(p[i] * (1.0 - MAX_DRIFT), p[i] * (1.0 + MAX_DRIFT));
        }
    }
}

/// Etiketli örnekler + prior pack → öğrenilen ModelPack (veya min-sample altı/
/// dejenere veride `None`). prior ağırlıkları korunur; yalnız 7 skorlama sinyali
/// öğrenilir (sharpening + breakdown ağırlıkları prior'dan kopyalanır).
pub fn train_model_pack(examples: &[TrainingExample], prior: &ModelPack) -> Option<ModelPack> {
    if examples.len() < MIN_TRAINING_GAMES {
        return None;
    }
    let p = prior_vector(prior);

    // Normal denklemler: A = XᵀX + λI, b = Xᵀy + λ·prior.
    let mut a = [[0.0f32; N_SIGNALS]; N_SIGNALS];
    let mut b = [0.0f32; N_SIGNALS];
    for ex in examples {
        let x = design_row(&ex.features);
        let y = if ex.won { 1.0 } else { 0.0 };
        for i in 0..N_SIGNALS {
            b[i] += x[i] * y;
            for j in 0..N_SIGNALS {
                a[i][j] += x[i] * x[j];
            }
        }
    }
    for i in 0..N_SIGNALS {
        a[i][i] += RIDGE_LAMBDA;
        b[i] += RIDGE_LAMBDA * p[i];
    }

    let mut w = solve(a, b)?;

    // 1) Clamp: her ağırlık prior'dan ±MAX_DRIFT bandında (asıl güvenlik kalkanı
    //    — öğrenilen pack çılgın öneri üretemez).
    for i in 0..N_SIGNALS {
        w[i] = w[i].clamp(p[i] * (1.0 - MAX_DRIFT), p[i] * (1.0 + MAX_DRIFT));
    }

    // 2) 6 pozitif ağırlığı, ±MAX_DRIFT bandında KALARAK prior pozitif-toplamına
    //    geri taşı (bounded redistribution). Böylece HEM sert drift sınırı HEM de
    //    model_score ölçeği (Tier eşikleri + breakdown UI) korunur.
    let prior_pos_sum: f32 = p[..6].iter().sum();
    redistribute_to_sum(&mut w, &p, prior_pos_sum);

    // 3) ORACLE GÜVENCESİ: öğrenilen ağırlıkları prior ile AÇIKÇA harmanla. Kural
    //    ağırlıkları her zaman ≥%50 paya sahip olur → determinist motor sessizce
    //    ezilmez (kullanıcı onaylı "muhafazakâr blend"). prior toplamı da ~0.86
    //    olduğundan pozitif toplam SABİT kalır; efektif drift ±%25'e iner.
    for i in 0..N_SIGNALS {
        w[i] = BLEND_TO_PRIOR * w[i] + (1.0 - BLEND_TO_PRIOR) * p[i];
    }

    let mut weights = prior.weights.clone();
    for (i, key) in SIGNAL_KEYS.iter().enumerate() {
        weights.insert((*key).to_string(), w[i]);
    }
    Some(ModelPack {
        version: LEARNED_MODEL_PACK_VERSION.to_string(),
        weights,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft_brain::local_rules_model_pack;

    fn example(features: [f32; 7], won: bool) -> TrainingExample {
        TrainingExample {
            features: TrainingFeatures {
                comfort: features[0],
                matchup: features[1],
                team_counter: features[2],
                synergy: features[3],
                meta: features[4],
                role_fit: features[5],
                risk: features[6],
            },
            won,
        }
    }

    fn positive_sum(pack: &ModelPack) -> f32 {
        ["comfort", "matchup", "team_counter", "synergy", "meta", "role_fit"]
            .iter()
            .map(|k| pack.weights.get(*k).copied().unwrap_or(0.0))
            .sum()
    }

    #[test]
    fn below_min_sample_returns_none() {
        let prior = local_rules_model_pack();
        let examples: Vec<_> = (0..20).map(|i| example([0.5; 7], i % 2 == 0)).collect();
        assert!(train_model_pack(&examples, &prior).is_none());
    }

    #[test]
    fn preserves_scale_and_drift_bounds() {
        let prior = local_rules_model_pack();
        // matchup kazanmayı belirliyor; gerisi gürültü.
        let mut examples = Vec::new();
        for i in 0..80 {
            let high = i % 2 == 0;
            let matchup = if high { 0.9 } else { 0.2 };
            examples.push(example([0.5, matchup, 0.5, 0.5, 0.5, 0.5, 0.2], high));
        }
        let pack = train_model_pack(&examples, &prior).expect("trained");
        assert_eq!(pack.version, LEARNED_MODEL_PACK_VERSION);
        // Ölçek korunur: bounded redistribution pozitif toplamı prior'a geri taşır.
        assert!((positive_sum(&pack) - positive_sum(&prior)).abs() < 0.02);
        // SERT güvenlik: her ağırlık prior'dan ±%50 içinde — abartı imkânsız.
        for key in SIGNAL_KEYS {
            let learned = pack.weights.get(key).copied().unwrap();
            let prior_w = prior.weights.get(key).copied().unwrap();
            assert!(
                learned >= prior_w * 0.5 - 1e-4 && learned <= prior_w * 1.5 + 1e-4,
                "{key}: {learned} prior {prior_w} ±%50 dışına çıktı"
            );
        }
    }

    #[test]
    fn learns_relative_importance_of_predictive_signal() {
        let prior = local_rules_model_pack();
        // matchup mükemmel ayırt edici; comfort sabit (ayırt etmez).
        let mut examples = Vec::new();
        for i in 0..120 {
            let win = i % 2 == 0;
            let matchup = if win { 0.95 } else { 0.15 };
            examples.push(example([0.6, matchup, 0.5, 0.5, 0.5, 0.5, 0.1], win));
        }
        let pack = train_model_pack(&examples, &prior).expect("trained");
        let m = pack.weights.get("matchup").copied().unwrap();
        let m_prior = prior.weights.get("matchup").copied().unwrap();
        // Ayırt edici sinyalin görece ağırlığı prior'ın üstüne çıkmalı.
        assert!(m > m_prior, "matchup ağırlığı artmadı: {m} <= {m_prior}");
    }

    #[test]
    fn risk_weight_stays_positive_penalty() {
        let prior = local_rules_model_pack();
        // Yüksek risk → kayıp; öğrenilen risk penaltısı pozitif kalmalı.
        let mut examples = Vec::new();
        for i in 0..80 {
            let win = i % 2 == 0;
            let risk = if win { 0.05 } else { 0.6 };
            examples.push(example([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, risk], win));
        }
        let pack = train_model_pack(&examples, &prior).expect("trained");
        let risk_w = pack.weights.get("risk").copied().unwrap();
        assert!(risk_w > 0.0, "risk penaltısı pozitif değil: {risk_w}");
    }

    #[test]
    fn degenerate_all_wins_stays_near_prior_no_nan() {
        let prior = local_rules_model_pack();
        let examples: Vec<_> = (0..50).map(|_| example([0.5; 7], true)).collect();
        let pack = train_model_pack(&examples, &prior).expect("trained");
        for key in SIGNAL_KEYS {
            let w = pack.weights.get(key).copied().unwrap();
            assert!(w.is_finite(), "{key} NaN/inf");
            assert!(w > 0.0, "{key} pozitif değil");
        }
        assert!((positive_sum(&pack) - positive_sum(&prior)).abs() < 0.02);
    }
}
