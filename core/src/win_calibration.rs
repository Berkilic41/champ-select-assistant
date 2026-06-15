//! FAZ 3 / Sprint 3A — win-probability calibration.
//!
//! Yerel `recommendation_outcomes` etiketlerinden (öneri skoru → gerçek win/loss)
//! kalibre edilmiş kazanma olasılığı üretir. SAF görsel augment: determinist
//! öneri skorunu DEĞİŞTİRMEZ; yalnız "bu skor seviyesinde geçmişte ~%X kazandın"
//! sinyalini verir. Muhafazakâr: her tahmin global base-rate'e Beta-shrink edilir
//! (ince/boş binler abartılı oran iddia edemez) ve global min-sample altında hiç
//! gösterilmez (dürüst soğuk-başlangıç). Determinist motor hâlâ oracle.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Global asgari etiketli maç — altında kalibrasyon yok (`available=false`).
const MIN_TOTAL_GAMES: usize = 20;
/// Beta prior gücü: base-rate'e doğru shrink pseudo-count'ı (muhafazakârlık kolu).
const PRIOR_STRENGTH: f32 = 6.0;
/// [0,1] öneri-skoru aralığı bin sayısı.
const BIN_COUNT: usize = 5;

#[derive(Debug, Clone, Deserialize)]
pub struct WinProbExample {
    /// Öneri skoru (Recommendation.total_score, 0..1) pick anında.
    pub score: f32,
    pub won: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct WinProbEstimate {
    /// Sorgulanan öneri skoru (0..1).
    pub score: f32,
    /// Kalibre + shrink edilmiş kazanma olasılığı (0..1).
    pub probability: f32,
    /// "low"/"medium"/"high" — bu skor bandındaki örnek sayısına göre.
    pub confidence: String,
    /// Bu tahmini besleyen (skor bandındaki) etiketli maç sayısı.
    pub sample_size: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct WinProbReport {
    /// false = min-sample altı; renderer hiç göstermez (dürüst "veri birikiyor").
    pub available: bool,
    /// Genel kazanma oranı (tüm etiketli maçlar) — bağlam + shrink hedefi.
    pub base_rate: f32,
    /// Toplam etiketli (resolved + skorlu) maç sayısı.
    pub total: u32,
    pub estimates: Vec<WinProbEstimate>,
}

struct CalibrationBin {
    total: u32,
    calibrated: f32,
}

struct WinCalibration {
    bins: Vec<CalibrationBin>,
}

fn bin_index(score: f32) -> usize {
    let clamped = score.clamp(0.0, 1.0);
    ((clamped * BIN_COUNT as f32) as usize).min(BIN_COUNT - 1)
}

fn confidence_for(sample: u32) -> &'static str {
    if sample < 8 {
        "low"
    } else if sample < 25 {
        "medium"
    } else {
        "high"
    }
}

/// Weighted Pool-Adjacent-Violators → monotone non-decreasing kalibrasyon.
/// Kalibrasyon özelliği: daha yüksek öneri skoru → daha düşük OLMAYAN kazanma
/// olasılığı. İnce veride bile utandırıcı "yüksek skor = düşük win%" üretmez.
fn pava(values: &mut [f32], weights: &[f32]) {
    let n = values.len();
    let mut v: Vec<f32> = Vec::with_capacity(n);
    let mut w: Vec<f32> = Vec::with_capacity(n);
    let mut len: Vec<usize> = Vec::with_capacity(n);
    for i in 0..n {
        let mut cur_v = values[i];
        let mut cur_w = weights[i];
        let mut cur_len = 1usize;
        while let Some(&prev_v) = v.last() {
            if prev_v <= cur_v {
                break;
            }
            let prev_w = w.pop().unwrap();
            let prev_len = len.pop().unwrap();
            v.pop();
            let total_w = prev_w + cur_w;
            cur_v = (prev_v * prev_w + cur_v * cur_w) / total_w;
            cur_w = total_w;
            cur_len += prev_len;
        }
        v.push(cur_v);
        w.push(cur_w);
        len.push(cur_len);
    }
    let mut idx = 0;
    for block in 0..v.len() {
        for _ in 0..len[block] {
            values[idx] = v[block];
            idx += 1;
        }
    }
}

fn build(examples: &[WinProbExample]) -> Option<WinCalibration> {
    let total = examples.len();
    if total < MIN_TOTAL_GAMES {
        return None;
    }
    let wins = examples.iter().filter(|e| e.won).count();
    let base_rate = wins as f32 / total as f32;

    let mut bin_wins = [0u32; BIN_COUNT];
    let mut bin_total = [0u32; BIN_COUNT];
    for example in examples {
        let idx = bin_index(example.score);
        bin_total[idx] += 1;
        if example.won {
            bin_wins[idx] += 1;
        }
    }

    // Beta-shrink: her bin global base-rate'e PRIOR_STRENGTH pseudo-count ile
    // çekilir → boş bin tam base-rate okur, ince bin abartmaz.
    let mut calibrated: Vec<f32> = (0..BIN_COUNT)
        .map(|i| {
            (bin_wins[i] as f32 + PRIOR_STRENGTH * base_rate)
                / (bin_total[i] as f32 + PRIOR_STRENGTH)
        })
        .collect();
    // Prior her bine ≥PRIOR_STRENGTH ağırlık verir → PAVA sıfır-ağırlık yaşamaz.
    let weights: Vec<f32> = (0..BIN_COUNT)
        .map(|i| bin_total[i] as f32 + PRIOR_STRENGTH)
        .collect();
    pava(&mut calibrated, &weights);

    let bins = (0..BIN_COUNT)
        .map(|i| CalibrationBin {
            total: bin_total[i],
            calibrated: calibrated[i].clamp(0.0, 1.0),
        })
        .collect();
    Some(WinCalibration { bins })
}

impl WinCalibration {
    fn estimate(&self, score: f32) -> WinProbEstimate {
        let bin = &self.bins[bin_index(score)];
        WinProbEstimate {
            score: score.clamp(0.0, 1.0),
            probability: bin.calibrated,
            confidence: confidence_for(bin.total).to_string(),
            sample_size: bin.total,
        }
    }
}

/// Etiketli örnekler + sorgu skorları → kalibre kazanma olasılığı raporu.
/// Min-sample altında `available=false` (estimates boş) — dürüst soğuk-başlangıç.
pub fn win_prob_report(examples: &[WinProbExample], scores: &[f32]) -> WinProbReport {
    let total = examples.len() as u32;
    let wins = examples.iter().filter(|e| e.won).count();
    let base_rate = if total == 0 {
        0.0
    } else {
        wins as f32 / total as f32
    };
    match build(examples) {
        None => WinProbReport {
            available: false,
            base_rate,
            total,
            estimates: Vec::new(),
        },
        Some(cal) => WinProbReport {
            available: true,
            base_rate,
            total,
            estimates: scores.iter().map(|&s| cal.estimate(s)).collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn examples(spec: &[(f32, usize, usize)]) -> Vec<WinProbExample> {
        // (score, wins, losses) → düz örnek listesi.
        let mut out = Vec::new();
        for &(score, wins, losses) in spec {
            for _ in 0..wins {
                out.push(WinProbExample { score, won: true });
            }
            for _ in 0..losses {
                out.push(WinProbExample { score, won: false });
            }
        }
        out
    }

    #[test]
    fn below_min_sample_is_unavailable_but_reports_base_rate() {
        let ex = examples(&[(0.7, 6, 4)]); // 10 maç < 20
        let report = win_prob_report(&ex, &[0.7]);
        assert!(!report.available);
        assert!(report.estimates.is_empty());
        assert_eq!(report.total, 10);
        assert!((report.base_rate - 0.6).abs() < 1e-6);
    }

    #[test]
    fn monotone_data_yields_increasing_calibration() {
        // Yüksek skor → daha çok kazanç.
        let ex = examples(&[(0.2, 2, 8), (0.5, 5, 5), (0.8, 9, 3)]);
        let report = win_prob_report(&ex, &[0.2, 0.5, 0.8]);
        assert!(report.available);
        let p: Vec<f32> = report.estimates.iter().map(|e| e.probability).collect();
        assert!(p[0] <= p[1] && p[1] <= p[2], "monoton değil: {p:?}");
        // Yüksek bandın olasılığı base-rate'in üstünde olmalı.
        assert!(p[2] > report.base_rate);
    }

    #[test]
    fn pava_enforces_monotonicity_on_inverted_data() {
        // Ters (yüksek skor daha az kazanır) — PAVA non-decreasing'e zorlar.
        let ex = examples(&[(0.1, 9, 1), (0.5, 5, 5), (0.9, 1, 9)]);
        let report = win_prob_report(&ex, &[0.1, 0.5, 0.9]);
        let p: Vec<f32> = report.estimates.iter().map(|e| e.probability).collect();
        assert!(p[0] <= p[1] && p[1] <= p[2], "PAVA monoton kılmadı: {p:?}");
    }

    #[test]
    fn shrinkage_keeps_thin_bin_off_the_extreme() {
        // 1 maçlık bant 1.0 okumamalı (base-rate'e çekilmeli).
        let mut ex = examples(&[(0.5, 10, 10)]); // base ~0.5, 20 maç → available
        ex.push(WinProbExample { score: 0.95, won: true }); // tek galibiyet bandı
        let report = win_prob_report(&ex, &[0.95]);
        let top = &report.estimates[0];
        assert!(top.probability < 0.95, "shrink yok: {}", top.probability);
        assert_eq!(top.sample_size, 1);
        assert_eq!(top.confidence, "low");
    }

    #[test]
    fn empty_band_reads_base_rate_with_zero_sample() {
        // 0.3 ve 0.7 bantlarında veri var; 0.9 bandı boş → base-rate civarı.
        let ex = examples(&[(0.3, 6, 6), (0.7, 8, 4)]); // 24 maç
        let report = win_prob_report(&ex, &[0.9]);
        let est = &report.estimates[0];
        assert_eq!(est.sample_size, 0);
        assert_eq!(est.confidence, "low");
        assert!((est.probability - report.base_rate).abs() < 0.20);
    }

    #[test]
    fn confidence_rises_with_sample_size() {
        assert_eq!(confidence_for(3), "low");
        assert_eq!(confidence_for(10), "medium");
        assert_eq!(confidence_for(40), "high");
    }

    #[test]
    fn bin_index_clamps_edges() {
        assert_eq!(bin_index(-1.0), 0);
        assert_eq!(bin_index(0.0), 0);
        assert_eq!(bin_index(1.0), BIN_COUNT - 1);
        assert_eq!(bin_index(2.0), BIN_COUNT - 1);
    }
}
