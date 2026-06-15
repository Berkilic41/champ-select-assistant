//! FAZ 4 / Sprint 1 — coach narrator seam (pluggable, offline-first).
//!
//! Grounded FACTS → doğal-dil koçluk notu. PLUGGABLE: gelecekte bir dış anlatıcı
//! (yerel-ONNX / edge LLM) aday prose üretebilir; bu aday AYNI audit'ten geçmek
//! ZORUNDA (over-promising yok + bu pick hakkında olmalı), geçemezse deterministik
//! narrative'e FALLBACK eder. Determinist motor hâlâ oracle: LLM yalnız aynı
//! grounded gerçekleri daha akıcı anlatır, yeni iddia EKLEYEMEZ (audit reddeder).
//!
//! core'da yalnız deterministik narrator + audit-gate vardır (offline, sıfır
//! bağımlılık, bugün çalışır). Bir LLM provider host/edge'de bu kontratı sağlar:
//! `narrate(brief, Some(llm_prose))` → audit geçerse llm_prose, yoksa deterministik.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::coach_quality::has_absolute_language;
use super::models::{ComboType, Recommendation};

const MIN_EXTERNAL_LEN: usize = 12;
const MAX_NARRATIVE_CHARS: usize = 600;

/// Anlatılacak grounded gerçekler — her narrator'ın aldığı tek girdi.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoachBrief {
    pub champion_name: String,
    /// En güçlü grounded sürücüler (kısa Türkçe ifadeler).
    #[serde(default)]
    pub drivers: Vec<String>,
    /// Lane rakibi key/adı (varsa) — grounding.
    #[serde(default)]
    pub lane_opponent: Option<String>,
    /// Birincil combo müttefik key'i (varsa).
    #[serde(default)]
    pub combo_ally: Option<String>,
    /// Combo tipi etiketi (ör. "engage follow-up").
    #[serde(default)]
    pub combo_type: Option<String>,
    /// İlk core item adı (varsa).
    #[serde(default)]
    pub core_item: Option<String>,
    /// Kısa risk özeti (varsa).
    #[serde(default)]
    pub risk_note: Option<String>,
    /// FAZ 3A: kalibre kazanma olasılığı (%) + örnek sayısı (varsa).
    #[serde(default)]
    pub win_prob_pct: Option<u32>,
    #[serde(default)]
    pub win_prob_games: Option<u32>,
    /// FAZ 3C: bu combo'da co-pick geçmişi (maç, galibiyet) (varsa).
    #[serde(default)]
    pub combo_history_games: Option<u32>,
    #[serde(default)]
    pub combo_history_wins: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CoachNarrative {
    pub text: String,
    /// "deterministic" | "external" — anlatıcı kaynağı (gözlemlenebilirlik).
    pub source: String,
    /// Bir dış aday sunulmuş ama audit'i geçemediği için reddedildi mi.
    pub external_rejected: bool,
}

fn combo_type_label(combo_type: &ComboType) -> &'static str {
    match combo_type {
        ComboType::EngageFollowup => "engage follow-up",
        ComboType::PickPotential => "pick yakalama",
        ComboType::ZoneControl => "alan kontrolü",
        ComboType::PeelChain => "peel zinciri",
        ComboType::Wombo => "wombo",
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// En güçlü 2 grounded sürücü (>0.55) — kısa Türkçe ifadelerle.
fn top_drivers(rec: &Recommendation) -> Vec<String> {
    let mut signals = [
        ("konforun yüksek", rec.comfort_score),
        ("matchup penceren iyi", rec.matchup_score),
        ("rakip kompa net cevabın var", rec.team_counter_score),
        ("takım sinerjin güçlü", rec.synergy_score),
        ("meta sinyalin sağlam", rec.meta_score),
        ("rol uyumun temiz", rec.role_fit_score),
    ];
    signals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    signals
        .iter()
        .filter(|(_, v)| *v > 0.55)
        .take(2)
        .map(|(phrase, _)| (*phrase).to_string())
        .collect()
}

/// Bir Recommendation'dan temel brief kur (FAZ 3 sinyalleri host'ta eklenir).
pub fn brief_from_recommendation(rec: &Recommendation) -> CoachBrief {
    let combo = rec
        .draft_plan
        .as_ref()
        .and_then(|plan| plan.combo_with.first());
    CoachBrief {
        champion_name: rec.champion_name.clone(),
        drivers: top_drivers(rec),
        lane_opponent: rec.lane_opponent_name.as_deref().and_then(non_empty),
        combo_ally: combo.map(|c| c.ally_champion_key.clone()),
        combo_type: combo.map(|c| combo_type_label(&c.combo_type).to_string()),
        core_item: rec.core_item_name.as_deref().and_then(non_empty),
        risk_note: rec.risk_summary.as_deref().and_then(non_empty),
        win_prob_pct: None,
        win_prob_games: None,
        combo_history_games: None,
        combo_history_wins: None,
    }
}

/// Deterministik koçluk notu: grounded gerçekleri + FAZ 3 sinyallerini akıcı bir
/// Türkçe nota örer. Yapısı gereği audit-temiz (mutlak/garanti dili yok).
fn deterministic_narrative(brief: &CoachBrief) -> String {
    let champ = if brief.champion_name.trim().is_empty() {
        "Bu pick"
    } else {
        brief.champion_name.trim()
    };

    let mut sentences: Vec<String> = Vec::new();

    // Açılış: şampiyon + en güçlü sürücü(ler).
    let opener = match brief.drivers.len() {
        0 => format!("{champ} dengeli bir seçenek."),
        1 => format!("{champ} için öne çıkan: {}.", brief.drivers[0]),
        _ => format!("{champ} için {} + {}.", brief.drivers[0], brief.drivers[1]),
    };
    sentences.push(opener);

    // Lane rakibi (grounding).
    if let Some(opp) = &brief.lane_opponent {
        sentences.push(format!(
            "{opp} karşısında erken takasları cooldown'lara göre seç."
        ));
    }

    // Combo + co-pick geçmişi.
    if let Some(ally) = &brief.combo_ally {
        let kind = brief.combo_type.as_deref().unwrap_or("combo");
        let mut combo_line = format!("{ally} ile {kind} penceren var");
        if let (Some(games), Some(wins)) = (brief.combo_history_games, brief.combo_history_wins) {
            if games >= 2 {
                let wr = (wins as f32 / games as f32 * 100.0).round() as u32;
                combo_line.push_str(&format!("; bu ikiliyle geçmişin {games} maç, %{wr}"));
            }
        }
        combo_line.push('.');
        sentences.push(combo_line);
    }

    // Kalibre kazanma olasılığı (FAZ 3A).
    if let (Some(pct), Some(games)) = (brief.win_prob_pct, brief.win_prob_games) {
        sentences.push(format!(
            "Bu skor seviyesinde geçmiş kazanman ~%{pct} ({games} maç) — yalnız bilgi."
        ));
    }

    // İlk item tempo penceresi.
    if let Some(item) = &brief.core_item {
        sentences.push(format!("İlk item {item} oturunca tempo penceresi ara."));
    }

    // Risk (varsa) — soft uyarı.
    if let Some(risk) = &brief.risk_note {
        let trimmed = risk.trim().trim_end_matches(['.', '!', '?']);
        sentences.push(format!("Dikkat: {trimmed}."));
    }

    let text = sentences.join(" ");
    if text.chars().count() > MAX_NARRATIVE_CHARS {
        text.chars().take(MAX_NARRATIVE_CHARS).collect()
    } else {
        text
    }
}

/// Dış aday prose'u doğrula: makul boy + over-promising YOK + bu pick hakkında
/// (şampiyonu anar). Geçerse audit-onaylı kabul edilir.
fn validate_external(brief: &CoachBrief, candidate: &str) -> bool {
    let trimmed = candidate.trim();
    let len = trimmed.chars().count();
    if !(MIN_EXTERNAL_LEN..=MAX_NARRATIVE_CHARS).contains(&len) {
        return false;
    }
    if has_absolute_language(trimmed) {
        return false; // mutlak/garanti dili — determinist oracle'ı aşırı-vaatle ezemez
    }
    if !brief.champion_name.trim().is_empty()
        && !trimmed
            .to_lowercase()
            .contains(&brief.champion_name.trim().to_lowercase())
    {
        return false; // bu pick hakkında değil → grounding ihlali
    }
    true
}

/// Brief + opsiyonel dış aday → koçluk notu. Aday audit'i geçerse onu, yoksa
/// deterministik notu döndürür. Determinist motor hâlâ oracle; bu yalnız anlatım.
pub fn narrate(brief: &CoachBrief, candidate: Option<&str>) -> CoachNarrative {
    if let Some(candidate) = candidate {
        if validate_external(brief, candidate) {
            return CoachNarrative {
                text: candidate.trim().to_string(),
                source: "external".to_string(),
                external_rejected: false,
            };
        }
        // Aday sunuldu ama audit'i geçemedi → deterministik fallback (işaretle).
        return CoachNarrative {
            text: deterministic_narrative(brief),
            source: "deterministic".to_string(),
            external_rejected: true,
        };
    }
    CoachNarrative {
        text: deterministic_narrative(brief),
        source: "deterministic".to_string(),
        external_rejected: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brief() -> CoachBrief {
        CoachBrief {
            champion_name: "LeeSin".to_string(),
            drivers: vec!["matchup penceren iyi".to_string()],
            lane_opponent: Some("Elise".to_string()),
            combo_ally: Some("Jarvan".to_string()),
            combo_type: Some("wombo".to_string()),
            core_item: Some("Goredrinker".to_string()),
            risk_note: Some("erken ölürsen tempo düşer".to_string()),
            win_prob_pct: Some(58),
            win_prob_games: Some(40),
            combo_history_games: Some(5),
            combo_history_wins: Some(3),
        }
    }

    #[test]
    fn deterministic_weaves_grounded_facts_and_faz3_signals() {
        let n = narrate(&brief(), None);
        assert_eq!(n.source, "deterministic");
        assert!(!n.external_rejected);
        assert!(n.text.contains("LeeSin"));
        assert!(n.text.contains("Elise")); // lane grounding
        assert!(n.text.contains("Jarvan")); // combo grounding
        assert!(n.text.contains("5 maç")); // combo history (FAZ 3C)
        assert!(n.text.contains("~%58")); // win-prob (FAZ 3A)
        assert!(n.text.contains("Goredrinker")); // build grounding
                                                 // Audit-temiz: mutlak dil yok.
        assert!(!has_absolute_language(&n.text));
    }

    #[test]
    fn accepts_a_clean_external_candidate() {
        let candidate = "LeeSin ile erken tempo kurup Jarvan'la pencere ararsan iyi olur.";
        let n = narrate(&brief(), Some(candidate));
        assert_eq!(n.source, "external");
        assert!(!n.external_rejected);
        assert_eq!(n.text, candidate);
    }

    #[test]
    fn rejects_over_promising_external_falls_back() {
        let candidate = "LeeSin ile bu maçı kesinlikle kazanırsın, garanti.";
        let n = narrate(&brief(), Some(candidate));
        assert_eq!(n.source, "deterministic"); // fallback
        assert!(n.external_rejected);
        assert!(n.text.contains("LeeSin"));
    }

    #[test]
    fn rejects_external_that_ignores_the_pick() {
        // Şampiyonu anmıyor → grounding ihlali → fallback.
        let candidate = "Erken oyunda tempo kur ve nehir görüşünü al.";
        let n = narrate(&brief(), Some(candidate));
        assert_eq!(n.source, "deterministic");
        assert!(n.external_rejected);
    }

    #[test]
    fn rejects_too_short_external() {
        let n = narrate(&brief(), Some("LeeSin"));
        assert!(n.external_rejected);
        assert_eq!(n.source, "deterministic");
    }

    #[test]
    fn handles_minimal_brief_without_panic() {
        let b = CoachBrief {
            champion_name: "Yasuo".to_string(),
            ..Default::default()
        };
        let n = narrate(&b, None);
        assert!(n.text.contains("Yasuo"));
        assert!(!has_absolute_language(&n.text));
    }
}
