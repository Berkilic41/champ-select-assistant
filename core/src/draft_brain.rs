use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::coach_quality;
use super::models::{ComboType, DataSourceBadge, Recommendation, ScoreBreakdownItem, Tier};

pub const DRAFT_BRAIN_RULES_VERSION: &str = "draft-brain-rules-v2";
pub const LOCAL_SEED_DATA_PACK_VERSION: &str = "local-seed-v1";
pub const COACH_DEPTH_SHORT_DEEP: &str = "short_deep";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelPack {
    pub version: String,
    pub weights: HashMap<String, f32>,
}

impl ModelPack {
    pub fn from_json(payload: &str) -> serde_json::Result<Self> {
        serde_json::from_str(payload)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DataPackQuality {
    #[serde(default, alias = "rates")]
    pub champion_rates: u32,
    #[serde(default)]
    pub matchups: u32,
    #[serde(default)]
    pub builds: u32,
    #[serde(default)]
    pub feedback: u32,
    #[serde(default)]
    pub draft_samples: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DataPack {
    pub version: String,
    pub patch: Option<String>,
    pub region: Option<String>,
    pub sources: Vec<String>,
    pub quality: DataPackQuality,
    pub fallback: bool,
    #[serde(default)]
    pub confidence: Option<String>,
    #[serde(default)]
    pub generated_at: Option<u32>,
}

impl DataPack {
    pub fn from_json(payload: &str) -> serde_json::Result<Self> {
        serde_json::from_str(payload)
    }
}

pub fn local_rules_model_pack() -> ModelPack {
    let mut weights = HashMap::new();
    weights.insert("comfort".to_string(), 0.17);
    weights.insert("matchup".to_string(), 0.18);
    weights.insert("team_counter".to_string(), 0.14);
    weights.insert("synergy".to_string(), 0.13);
    weights.insert("meta".to_string(), 0.12);
    weights.insert("role_fit".to_string(), 0.12);
    weights.insert("risk".to_string(), 0.08);
    weights.insert("blind_safety".to_string(), 0.05);
    weights.insert("team_need".to_string(), 0.05);
    weights.insert("phase".to_string(), 0.04);
    weights.insert("execution".to_string(), 0.04);

    ModelPack {
        version: DRAFT_BRAIN_RULES_VERSION.to_string(),
        weights,
    }
}

pub fn local_seed_data_pack() -> DataPack {
    DataPack {
        version: LOCAL_SEED_DATA_PACK_VERSION.to_string(),
        patch: Some("local".to_string()),
        region: Some("global".to_string()),
        fallback: true,
        quality: DataPackQuality {
            champion_rates: 0,
            matchups: 0,
            builds: 0,
            feedback: 0,
            draft_samples: 0,
        },
        sources: vec!["manual_seed".to_string()],
        confidence: Some("low".to_string()),
        generated_at: None,
    }
}

#[allow(dead_code)]
pub fn upgrade_recommendations(recommendations: &mut [Recommendation]) {
    upgrade_recommendations_with_context(recommendations, None, None);
}

pub fn upgrade_recommendations_with_context(
    recommendations: &mut [Recommendation],
    model_pack: Option<&ModelPack>,
    data_pack: Option<&DataPack>,
) {
    for recommendation in recommendations.iter_mut() {
        upgrade_recommendation_with_context(recommendation, model_pack, data_pack);
    }

    recommendations.sort_by(|left, right| {
        right
            .total_score
            .partial_cmp(&left.total_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let best = recommendations.first().cloned();
    if let Some(best) = best {
        for recommendation in recommendations.iter_mut().skip(1) {
            let note = comparative_why_not(&best, recommendation);
            recommendation.why_not.push(note);
            finalize_coaching_quality(recommendation);
        }
    }
}

#[allow(dead_code)]
pub fn upgrade_recommendation(recommendation: &mut Recommendation, model_pack: Option<&ModelPack>) {
    upgrade_recommendation_with_context(recommendation, model_pack, None);
}

pub fn upgrade_recommendation_with_context(
    recommendation: &mut Recommendation,
    model_pack: Option<&ModelPack>,
    data_pack: Option<&DataPack>,
) {
    recommendation.rules_score = recommendation.total_score;
    recommendation.model_score = apply_model_score(recommendation, model_pack);
    recommendation.total_score = recommendation.model_score;
    recommendation.tier = Tier::from_score(recommendation.total_score);
    recommendation.model_version = model_pack
        .map(|pack| pack.version.clone())
        .unwrap_or_else(|| DRAFT_BRAIN_RULES_VERSION.to_string());
    recommendation.coach_depth = COACH_DEPTH_SHORT_DEEP.to_string();

    attach_data_pack_badge(recommendation, data_pack);
    recommendation.score_breakdown = build_score_breakdown(recommendation, model_pack);

    if recommendation.lane_plan.is_none() {
        recommendation.lane_plan = build_lane_plan(recommendation);
    }
    if recommendation.mid_game_plan.is_none() {
        recommendation.mid_game_plan = build_mid_game_plan(recommendation);
    }
    if recommendation.teamfight_job.is_none() {
        recommendation.teamfight_job = build_teamfight_job(recommendation);
    }
    if recommendation.fallback_plan.is_none() {
        recommendation.fallback_plan = build_fallback_plan(recommendation);
    }

    // Ground the finalized lane plan in the specific matchup — name the resolved lane
    // opponent — whatever the plan's source (engine `lane_phase_advice` or the heuristic
    // builder). Archetype-level only; no fabricated opponent mechanics. The grounded
    // lane line then flows into the decision sentence's "Lane:" clause below.
    if let Some(plan) = recommendation.lane_plan.take() {
        recommendation.lane_plan = Some(ground_lane_with_opponent(recommendation, plan));
    }

    // Ground the macro line in the actual build — name the first core item (resolved by
    // the command layer). Honesty: only the item name + its build provenance; no
    // fabricated item stats / winrate. Lives in the (uncapped) macro plan, not the
    // word-budgeted decision sentence.
    ground_mid_game_with_build(recommendation);

    recommendation.headline = Some(build_headline(recommendation));
    recommendation.decision_sentence = build_decision_sentence(recommendation);
    finalize_coaching_quality(recommendation);
}

/// Prefix lane advice with the resolved opponent so the prose is tied to THIS matchup
/// (e.g. "Ahri karşısında: …"). No-op when the opponent is unknown (blind / first-pick /
/// ARAM) or already named — honest grounding, never an empty name-drop.
fn ground_lane_with_opponent(recommendation: &Recommendation, advice: String) -> String {
    match recommendation.lane_opponent_name.as_deref() {
        Some(opponent) if !opponent.trim().is_empty() && !advice.contains(opponent) => {
            format!("{opponent} karşısında: {advice}")
        }
        _ => advice,
    }
}

/// Append a build-grounded note (first core item + its provenance) to the macro plan.
/// No-op when there is no resolved item or it is already named. Provenance is honest:
/// `seed` = curated build, otherwise an archetype-general suggestion with no winrate claim.
fn ground_mid_game_with_build(recommendation: &mut Recommendation) {
    let Some(item) = recommendation
        .core_item_name
        .as_deref()
        .filter(|i| !i.trim().is_empty())
        .map(str::to_string)
    else {
        return;
    };
    let note = if recommendation.build_source == "seed" {
        format!("İlk item {item}: bu eşleşmede oturmuş seed build, spike'ında tempo penceresi ara.")
    } else {
        format!("İlk item {item}: arşetip-genel öneri (kaynaklı winrate yok), spike'ında tempoyu kullan.")
    };
    recommendation.mid_game_plan = Some(match recommendation.mid_game_plan.take() {
        Some(mid) if mid.contains(&item) => mid,
        Some(mid) => format!("{mid} {note}"),
        None => note,
    });
}

fn attach_data_pack_badge(recommendation: &mut Recommendation, data_pack: Option<&DataPack>) {
    let Some(pack) = data_pack else {
        return;
    };

    let source = pack
        .sources
        .first()
        .cloned()
        .unwrap_or_else(|| "local_seed".to_string());
    let mut badge = DataSourceBadge {
        source,
        patch: pack.patch.clone(),
        sample_size: None,
        confidence: pack_confidence(pack),
        risk_level: if pack.fallback { "medium" } else { "low" }.to_string(),
        updated_at: pack.generated_at,
    };

    if badge.patch.as_deref().unwrap_or("").trim().is_empty() {
        badge.patch = Some("local".to_string());
    }
    if badge.confidence.trim().is_empty() {
        badge.confidence = pack_confidence(pack);
    }

    recommendation.data_sources.push(badge);
    recommendation.data_sources = dedup_badges(&recommendation.data_sources);

    let build_confidence = confidence_from_quality(pack.quality.builds, pack.fallback);
    let matchup_confidence = confidence_from_quality(pack.quality.matchups, pack.fallback);

    // Honest overall confidence: never exceed the weakest backing signal. A high
    // personal-comfort pick with thin build/matchup data must NOT read "high".
    recommendation.confidence = combine_confidence(&[
        &recommendation.confidence, // personal (local games)
        &build_confidence,
        &matchup_confidence,
    ])
    .to_string();

    recommendation.build_confidence = build_confidence;
    recommendation.matchup_confidence = matchup_confidence;
}

/// Weakest-link confidence: the displayed label never claims more than the data
/// that backs it. `"high">"medium">"low"`; `"heuristic"` counts as low (opponent
/// known but no real data); `"none"`/empty are *not applicable* (e.g. a blind pick
/// has no opponent) and are skipped rather than dragging the result down.
fn combine_confidence(labels: &[&str]) -> &'static str {
    let rank = |c: &str| match c {
        "high" => Some(3u8),
        "medium" => Some(2),
        "low" | "heuristic" => Some(1),
        _ => None,
    };
    match labels.iter().filter_map(|c| rank(c)).min() {
        Some(3) => "high",
        Some(2) => "medium",
        _ => "low",
    }
}

fn pack_confidence(pack: &DataPack) -> String {
    pack.confidence
        .as_deref()
        .filter(|confidence| !confidence.trim().is_empty())
        .unwrap_or(if pack.fallback { "low" } else { "medium" })
        .to_string()
}

fn dedup_badges(badges: &[DataSourceBadge]) -> Vec<DataSourceBadge> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for badge in badges {
        let key = format!(
            "{}|{}|{}|{}",
            badge.source,
            badge.patch.as_deref().unwrap_or(""),
            badge.confidence,
            badge.risk_level
        );
        if seen.insert(key) {
            result.push(badge.clone());
        }
    }

    result
}

fn confidence_from_quality(quality_count: u32, fallback: bool) -> String {
    if quality_count == 0 || (fallback && quality_count < 10) {
        "low".to_string()
    } else if quality_count < 50 {
        "medium".to_string()
    } else {
        "high".to_string()
    }
}

fn apply_model_score(recommendation: &Recommendation, model_pack: Option<&ModelPack>) -> f32 {
    let Some(model_pack) = model_pack else {
        return apply_rules_sharpening(recommendation.total_score, recommendation);
    };

    let get = |key: &str, default: f32| model_pack.weights.get(key).copied().unwrap_or(default);
    let positive = recommendation.comfort_score * get("comfort", 0.17)
        + recommendation.matchup_score * get("matchup", 0.18)
        + recommendation.team_counter_score * get("team_counter", 0.14)
        + recommendation.synergy_score * get("synergy", 0.13)
        + recommendation.meta_score * get("meta", 0.12)
        + recommendation.role_fit_score * get("role_fit", 0.12);

    let risk_penalty = recommendation.risk_score * get("risk", 0.08);
    let model_score = positive - risk_penalty;
    apply_rules_sharpening(model_score, recommendation)
}

fn apply_rules_sharpening(score: f32, recommendation: &Recommendation) -> f32 {
    let plan_delta = 0.05 * (blind_safety_score(recommendation) - 0.50)
        + 0.05 * (team_need_score(recommendation) - 0.50)
        + 0.04 * (phase_score(recommendation) - 0.50)
        + 0.04 * (execution_score(recommendation) - 0.50);

    (score + plan_delta).clamp(0.0, 1.0)
}

fn build_score_breakdown(
    recommendation: &Recommendation,
    model_pack: Option<&ModelPack>,
) -> Vec<ScoreBreakdownItem> {
    let weights = model_pack
        .cloned()
        .unwrap_or_else(local_rules_model_pack)
        .weights;

    let get = |key: &str, default: f32| weights.get(key).copied().unwrap_or(default);
    let mut rows = vec![
        score_row(
            "comfort",
            "Konfor",
            recommendation.comfort_score,
            get("comfort", 0.17),
            "high",
        ),
        score_row(
            "matchup",
            "Matchup",
            recommendation.matchup_score,
            get("matchup", 0.18),
            confidence_or_default(&recommendation.matchup_confidence),
        ),
        score_row(
            "team_counter",
            "Rakip kompa cevap",
            recommendation.team_counter_score,
            get("team_counter", 0.14),
            "medium",
        ),
        score_row(
            "synergy",
            "Takım sinerjisi",
            recommendation.synergy_score,
            get("synergy", 0.13),
            "medium",
        ),
        score_row(
            "meta",
            "Meta",
            recommendation.meta_score,
            get("meta", 0.12),
            "medium",
        ),
        score_row(
            "role_fit",
            "Rol uyumu",
            recommendation.role_fit_score,
            get("role_fit", 0.12),
            "high",
        ),
        score_row(
            "blind_safety",
            "Blind güvenliği",
            blind_safety_score(recommendation),
            get("blind_safety", 0.05),
            "medium",
        ),
        score_row(
            "team_need",
            "Takım ihtiyacı",
            team_need_score(recommendation),
            get("team_need", 0.05),
            "medium",
        ),
        score_row(
            "phase",
            "Faz uyumu",
            phase_score(recommendation),
            get("phase", 0.04),
            "medium",
        ),
        score_row(
            "execution",
            "Execution",
            execution_score(recommendation),
            get("execution", 0.04),
            "medium",
        ),
        score_row(
            "risk_control",
            "Risk yönetimi",
            (1.0 - recommendation.risk_score).clamp(0.0, 1.0),
            get("risk", 0.08),
            "medium",
        ),
    ];

    rows.sort_by(|left, right| {
        right
            .contribution
            .partial_cmp(&left.contribution)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows
}

fn confidence_or_default(confidence: &str) -> &str {
    if confidence.trim().is_empty() {
        "medium"
    } else {
        confidence
    }
}

fn score_row(
    key: &str,
    label: &str,
    value: f32,
    weight: f32,
    confidence: &str,
) -> ScoreBreakdownItem {
    ScoreBreakdownItem {
        key: key.to_string(),
        label: label.to_string(),
        value: value.clamp(0.0, 1.0),
        weight,
        contribution: (value.clamp(0.0, 1.0) * weight).max(0.0),
        confidence: confidence.to_string(),
    }
}

fn blind_safety_score(recommendation: &Recommendation) -> f32 {
    recommendation
        .draft_plan
        .as_ref()
        .map(|plan| plan.blind_pick_safety.clamp(0.0, 1.0))
        .unwrap_or(0.50)
}

fn team_need_score(recommendation: &Recommendation) -> f32 {
    recommendation
        .draft_plan
        .as_ref()
        .map(|plan| {
            if plan.fills_team_need.is_empty() {
                0.50
            } else {
                (0.56 + plan.fills_team_need.len() as f32 * 0.10).clamp(0.0, 1.0)
            }
        })
        .unwrap_or(0.50)
}

fn phase_score(recommendation: &Recommendation) -> f32 {
    recommendation
        .phase_matchup
        .map(|phase| ((phase[0] + phase[1] + phase[2]) / 3.0).clamp(0.0, 1.0))
        .unwrap_or(0.50)
}

fn execution_score(recommendation: &Recommendation) -> f32 {
    recommendation
        .draft_plan
        .as_ref()
        .map(|plan| {
            let difficulty = plan.execution_difficulty.clamp(1, 5) as f32;
            (1.0 - ((difficulty - 1.0) / 4.0)).clamp(0.0, 1.0)
        })
        .unwrap_or(0.50)
}

/// Punchy hero headline — the single strongest grounded driver(s), capitalized, with no
/// combo/lane/risk tail. The crisp "why" for the decision card; the fuller story is the
/// decision sentence + deep-dive. Reuses the same drivers, so it never over-promises.
fn build_headline(recommendation: &Recommendation) -> String {
    let drivers = positive_driver_rows(recommendation)
        .into_iter()
        .take(2)
        .map(|row| driver_phrase(recommendation, row))
        .collect::<Vec<_>>();
    let driver_text = if drivers.is_empty() {
        strongest_positive(recommendation)
    } else {
        drivers.join(" + ")
    };
    let mut chars = driver_text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => driver_text,
    }
}

fn build_decision_sentence(recommendation: &Recommendation) -> String {
    let drivers = positive_driver_rows(recommendation)
        .into_iter()
        .take(2)
        .map(|row| driver_phrase(recommendation, row))
        .collect::<Vec<_>>();

    let driver_text = if drivers.is_empty() {
        strongest_positive(recommendation)
    } else {
        drivers.join(" + ")
    };

    let mut parts = vec![format!(
        "{} seç: {}.",
        recommendation.champion_name, driver_text
    )];

    if let Some(combo) = combo_driver(recommendation) {
        parts.push(combo);
    } else if let Some(spike) = spike_driver(recommendation) {
        parts.push(spike);
    }

    if let Some(lane_plan) = recommendation.lane_plan.as_deref() {
        // Keep the decision sentence within the word budget: inline only the lead
        // clause of the lane plan; its archetype-threat tail stays in the standalone
        // lane plan (shown in its own UI section).
        let lead = lane_plan.split("; ").next().unwrap_or(lane_plan);
        parts.push(format!("Lane: {}", trim_sentence(lead)));
    }

    if let Some(risk) = recommendation.risk_summary.as_deref() {
        parts.push(format!("Risk: {}", trim_sentence(risk)));
    } else if recommendation.risk_score > 0.55 {
        parts.push("Risk: execution temizliği ister.".to_string());
    }

    parts.join(" ")
}

fn positive_driver_rows(recommendation: &Recommendation) -> Vec<&ScoreBreakdownItem> {
    let mut rows = recommendation
        .score_breakdown
        .iter()
        .filter(|row| {
            row.contribution > 0.01
                && !matches!(
                    row.key.as_str(),
                    "risk_control" | "execution" if row.value < 0.55
                )
        })
        .collect::<Vec<_>>();

    rows.sort_by(|left, right| {
        right
            .contribution
            .partial_cmp(&left.contribution)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows
}

/// The phase (erken/orta/geç) where the pick holds a CLEAR matchup advantage, or
/// `None` when the phase profile is flat/neutral (blind pick, even matchup). Used to
/// ground the otherwise-generic "matchup penceren iyi" driver in a concrete window.
fn dominant_phase_tr(recommendation: &Recommendation) -> Option<&'static str> {
    let phase = recommendation.phase_matchup?;
    let max = phase.iter().copied().fold(f32::MIN, f32::max);
    let min = phase.iter().copied().fold(f32::MAX, f32::min);
    // Only ground when there is a real tilt — otherwise stay honestly generic.
    if max < 0.55 || (max - min) < 0.08 {
        return None;
    }
    let idx = phase.iter().position(|&v| v == max)?;
    Some(match idx {
        0 => "erken",
        1 => "orta",
        _ => "geç",
    })
}

/// Turkish phrase for a positive score driver. Grounds the two highest-signal drivers
/// (matchup → which phase you lead, synergy → which ally you combo with) in concrete,
/// data-backed specifics instead of generic labels; the rest stay as crisp labels.
fn driver_phrase(recommendation: &Recommendation, row: &ScoreBreakdownItem) -> String {
    match row.key.as_str() {
        "comfort" => "konfor sinyalin yüksek".to_string(),
        "matchup" => match dominant_phase_tr(recommendation) {
            Some(phase) => format!("{phase} oyunda matchup öndesin"),
            None => "matchup penceren iyi".to_string(),
        },
        "team_counter" => "rakip kompa net cevabın var".to_string(),
        "synergy" => match recommendation
            .draft_plan
            .as_ref()
            .and_then(|plan| plan.combo_with.first())
        {
            Some(combo) => format!("{} ile sinerjin güçlü", combo.ally_champion_key),
            None => "takım sinerjin güçlü".to_string(),
        },
        "meta" => "meta sinyalin güvenli".to_string(),
        "role_fit" => "rol uyumun temiz".to_string(),
        "blind_safety" => "blind güvenliğin iyi".to_string(),
        "team_need" => "takım ihtiyacını kapatıyorsun".to_string(),
        "phase" => "oyun fazı uyumun iyi".to_string(),
        "execution" => "execution riski yönetilebilir".to_string(),
        "risk_control" => "riskin kontrol edilebilir".to_string(),
        _ => format!("{} güçlü", row.label),
    }
}

fn weakness_phrase(row: &ScoreBreakdownItem) -> String {
    match row.key.as_str() {
        "comfort" => "konfor sinyali",
        "matchup" => "matchup sinyali",
        "team_counter" => "rakip kompa cevap",
        "synergy" => "takım sinerjisi",
        "meta" => "meta sinyali",
        "role_fit" => "rol uyumu",
        "blind_safety" => "blind güvenliği",
        "team_need" => "takım ihtiyacı",
        "phase" => "oyun fazı uyumu",
        "execution" => "execution riski",
        "risk_control" => "risk yönetimi",
        _ => row.label.as_str(),
    }
    .to_string()
}

fn combo_driver(recommendation: &Recommendation) -> Option<String> {
    let combo = recommendation.draft_plan.as_ref()?.combo_with.first()?;
    Some(format!(
        "{} ile {} penceresi var.",
        combo.ally_champion_key,
        combo_type_label(&combo.combo_type)
    ))
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

fn spike_driver(recommendation: &Recommendation) -> Option<String> {
    recommendation
        .draft_plan
        .as_ref()?
        .spike_note
        .as_deref()
        .map(|spike| format!("Spike: {}", trim_sentence(spike)))
}

fn trim_sentence(text: &str) -> String {
    text.trim()
        .trim_end_matches('.')
        .trim_end_matches('!')
        .trim_end_matches('?')
        .to_string()
}

fn strongest_positive(recommendation: &Recommendation) -> String {
    let mut scores = [
        ("konfor sinyalin yüksek", recommendation.comfort_score),
        ("matchup penceren iyi", recommendation.matchup_score),
        ("takım sinerjin güçlü", recommendation.synergy_score),
        ("rakip kompa cevabın var", recommendation.team_counter_score),
        ("rol uyumun temiz", recommendation.role_fit_score),
        ("meta sinyalin güvenli", recommendation.meta_score),
    ];

    scores.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    scores[0].0.to_string()
}

fn build_lane_plan(recommendation: &Recommendation) -> Option<String> {
    if let Some(plan) = recommendation
        .draft_plan
        .as_ref()
        .and_then(|plan| plan.lane_phase_advice.as_deref())
    {
        return Some(trim_sentence(plan));
    }

    if recommendation.matchup_score >= 0.70 {
        Some(format!(
            "{} ile ilk dalgalarda tempo al; rakibin ana cooldown'ı boştayken kısa takas ara.",
            recommendation.champion_name
        ))
    } else if recommendation.matchup_score <= 0.42 {
        Some(format!(
            "{} ile lane'i kontrollü tut; görüşsüz nehir savaşına zorlanmadan farm ve seviye eşitliği koru.",
            recommendation.champion_name
        ))
    } else {
        Some(format!(
            "{} ile ilk 3 dalgada güvenli tempo kur; gereksiz uzun takas yerine jungle penceresini bekle.",
            recommendation.champion_name
        ))
    }
}

fn build_mid_game_plan(recommendation: &Recommendation) -> Option<String> {
    if let Some(plan) = recommendation.draft_plan.as_ref() {
        if !plan.win_condition.trim().is_empty() {
            // Ground the macro line in the specific comp gap this pick fills, when known.
            let need = plan
                .fills_team_need
                .first()
                .filter(|n| !n.trim().is_empty())
                .map(|n| format!(" Takıma {n} katıyorsun; tempo penceresini buna göre seç."))
                .unwrap_or_default();
            return Some(format!(
                "Orta oyunda {} planını oyna; görüş ve tempo penceresini buna göre kur.{}",
                plan.win_condition, need
            ));
        }
    }

    let phase = recommendation.phase_matchup.unwrap_or([0.5, 0.5, 0.5]);
    if phase[1] >= 0.62 {
        Some(
            "Orta oyunda item spike'ın gelince nehir görüşüyle pick ve obje baskısı kur."
                .to_string(),
        )
    } else if phase[2] > phase[0] + 0.10 {
        Some(
            "Orta oyunda gereksiz 2v2 zorlama; kaynak kaybetmeden geç oyun değerine hazırlan."
                .to_string(),
        )
    } else {
        Some(
            "Orta oyunda dalga it, güvenli görüş al ve takımın hazırken kısa tempo penceresi ara."
                .to_string(),
        )
    }
}

fn build_teamfight_job(recommendation: &Recommendation) -> Option<String> {
    if let Some(plan) = recommendation.draft_plan.as_ref() {
        if !plan.team_role.trim().is_empty() {
            return Some(format!(
                "Takım savaşında görevin: {}. Ana cooldown'ı takım CC'siyle aynı pencereye sakla.",
                plan.team_role
            ));
        }
    }

    if recommendation.synergy_score >= 0.68 || recommendation.team_counter_score >= 0.68 {
        Some("Takım savaşında ilk görevin takım CC'sine hasar eklemek; tek başına erken engage zorlama.".to_string())
    } else if recommendation.risk_score >= 0.60 {
        Some("Takım savaşında güvenli açıdan başla; ana cooldown'larını rakip engage bittikten sonra kullan.".to_string())
    } else {
        Some(
            "Takım savaşında pozisyonunu koru, öncelikli hedefe takımınla aynı anda baskı kur."
                .to_string(),
        )
    }
}

fn build_fallback_plan(recommendation: &Recommendation) -> Option<String> {
    if let Some(risk_summary) = recommendation.risk_summary.as_deref() {
        return Some(format!(
            "Plan bozulursa {} riskini azalt; dalga ve görüş güvenliği olmadan savaş başlatma.",
            trim_sentence(risk_summary)
        ));
    }

    if recommendation.risk_score >= 0.55 {
        Some("Geride kalırsan yan koridorda risksiz kaynak topla, görüşsüz alanda mekanik outplay arama.".to_string())
    } else if recommendation.matchup_score <= 0.42 {
        Some("Lane geride başlarsa minyon kaybını sınırlı tut, takım savaşındaki görevine geçiş yap.".to_string())
    } else {
        Some("Plan yavaşlarsa tempo kaybını zorlamadan toparla; bir sonraki obje öncesi reset ve görüşe öncelik ver.".to_string())
    }
}

fn comparative_why_not(best: &Recommendation, current: &Recommendation) -> String {
    let best_driver = positive_driver_rows(best)
        .first()
        .map(|row| driver_phrase(best, row))
        .unwrap_or_else(|| strongest_positive(best));

    let weakness = weakest_driver(current)
        .map(weakness_phrase)
        .unwrap_or_else(|| "genel skor".to_string());

    format!(
        "{} yerine {}: {} daha güçlü; {} tarafında {} daha zayıf.",
        current.champion_name, best.champion_name, best_driver, current.champion_name, weakness
    )
}

fn weakest_driver(recommendation: &Recommendation) -> Option<&ScoreBreakdownItem> {
    recommendation
        .score_breakdown
        .iter()
        .filter(|row| row.key != "risk_control")
        .min_by(|left, right| {
            left.value
                .partial_cmp(&right.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn finalize_coaching_quality(recommendation: &mut Recommendation) {
    recommendation.why_not = coach_quality::dedup_sentences(&recommendation.why_not);

    // Release-safe tripwire (logs on failure); debug builds still hard-assert so a
    // builder regression fails tests loudly.
    let issue_count = coach_quality::audit_soft(
        &recommendation.decision_sentence,
        recommendation.lane_plan.as_deref(),
        recommendation.teamfight_job.as_deref(),
        &recommendation.why_not,
        &recommendation.champion_name,
    );
    debug_assert!(
        issue_count == 0,
        "DraftBrain generated coaching text that failed quality audit for {}",
        recommendation.champion_name
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DraftPlan, Recommendation, Tier};

    fn sample_recommendation() -> Recommendation {
        Recommendation {
            champion_id: 238,
            champion_key: "Zed".to_string(),
            champion_name: "Zed".to_string(),
            total_score: 0.72,
            comfort_score: 0.88,
            mechanical_comfort: 0.0,
            draft_winrate: None,
            confidence_basis: String::new(),
            matchup_score: 0.78,
            team_counter_score: 0.67,
            synergy_score: 0.58,
            meta_score: 0.62,
            role_fit_score: 0.91,
            risk_score: 0.18,
            reason: "good comfort".to_string(),
            headline: None,
            decision_sentence: String::new(),
            score_breakdown: Vec::new(),
            model_score: 0.0,
            rules_score: 0.0,
            model_version: String::new(),
            build_source: String::new(),
            build_confidence: String::new(),
            build_note: None,
            matchup_confidence: String::new(),
            missing_signals: Vec::new(),
            pro_presence: None,
            lane_plan: None,
            mid_game_plan: None,
            teamfight_plan: None,
            teamfight_job: None,
            fallback_plan: None,
            risk_summary: Some("snowball gecikirse yan koridor baskısı düşer".to_string()),
            why_not: vec!["Needs clean snowball execution.".to_string()],
            data_sources: Vec::new(),
            coach_depth: String::new(),
            core_items: vec![6692, 3142, 3814],
            situational_items: Vec::new(),
            primary_rune_tree: 8100,
            keystone: 8112,
            skill_order: Some("Q-W-E".to_string()),
            summoner_spells: vec![4, 14],
            secondary_runes: vec![8200, 8214, 8236],
            stat_shards: vec![5008, 5008, 5002],
            tier: Tier::A,
            confidence: "high".to_string(),
            games_on_champ: 120,
            wins_on_champ: 70,
            enemy_team_summary: "AP ağırlıklı".to_string(),
            lane_opponent_name: None,
            core_item_name: None,
            draft_plan: None,
            phase_matchup: None,
        }
    }

    fn sample_draft_plan() -> DraftPlan {
        DraftPlan {
            combo_with: Vec::new(),
            win_condition: "mid-game pick ile kartopu".to_string(),
            team_role: "arka hatta tehdit yarat".to_string(),
            damage_profile: "AD burst".to_string(),
            blind_pick_safety: 0.72,
            execution_difficulty: 3,
            threats: vec!["point-and-click lockdown".to_string()],
            fills_team_need: vec!["AD burst".to_string(), "side pressure".to_string()],
            risk_note: Some("CC zincirine karşı açı seçimi ister".to_string()),
            pick_window_note: Some("counter-pick olarak daha değerli".to_string()),
            comp_clash_note: None,
            spike_note: Some("Dirk sonrası ilk roam penceresi önemli".to_string()),
            lane_phase_advice: Some(
                "ilk 3 dalgada farmı koru, seviye 6 sonrası vision ile roam ara".to_string(),
            ),
        }
    }

    #[test]
    fn local_model_pack_has_core_weights() {
        let pack = local_rules_model_pack();
        assert_eq!(pack.version, DRAFT_BRAIN_RULES_VERSION);
        assert!(pack.weights.contains_key("matchup"));
        assert!(pack.weights.contains_key("blind_safety"));
        assert!(pack.weights.contains_key("execution"));
    }

    #[test]
    fn upgrade_adds_decision_and_breakdown() {
        let mut recommendation = sample_recommendation();

        upgrade_recommendation(&mut recommendation, Some(&local_rules_model_pack()));

        assert!(recommendation.decision_sentence.starts_with("Zed seç:"));
        assert!(!recommendation.score_breakdown.is_empty());
        assert_eq!(recommendation.coach_depth, COACH_DEPTH_SHORT_DEEP);
        assert!(recommendation.model_score > 0.0);
        assert!(recommendation.rules_score > 0.0);
        assert!(recommendation.lane_plan.is_some());
        assert!(recommendation.teamfight_job.is_some());
        assert!(recommendation.fallback_plan.is_some());
    }

    #[test]
    fn upgrade_attaches_data_pack_quality_badge() {
        let mut recommendation = sample_recommendation();
        let data_pack = DataPack {
            version: "pack-v1".to_string(),
            patch: Some("16.10".to_string()),
            region: Some("TR".to_string()),
            fallback: false,
            quality: DataPackQuality {
                champion_rates: 172,
                matchups: 1_000,
                builds: 172,
                feedback: 25,
                draft_samples: 50,
            },
            sources: vec!["lolalytics".to_string()],
            confidence: Some("high".to_string()),
            generated_at: Some(1_780_358_400),
        };

        upgrade_recommendation_with_context(
            &mut recommendation,
            Some(&local_rules_model_pack()),
            Some(&data_pack),
        );

        assert_eq!(recommendation.data_sources.len(), 1);
        assert_eq!(recommendation.data_sources[0].source, "lolalytics");
        assert_eq!(recommendation.data_sources[0].confidence, "high");
        assert_eq!(
            recommendation.data_sources[0].updated_at,
            Some(1_780_358_400)
        );
        assert_eq!(recommendation.build_confidence, "high");
        assert_eq!(recommendation.matchup_confidence, "high");
    }

    #[test]
    fn data_pack_parses_backend_quality_shape() {
        let payload = serde_json::json!({
            "version": "data-pack-16.10-tr1",
            "patch": "16.10",
            "region": "tr1",
            "sources": ["cloud_postgres", "riot_match_v5"],
            "quality": {
                "rates": 172,
                "matchups": 1200,
                "builds": 172,
                "feedback": 33,
                "draft_samples": 44
            },
            "fallback": false,
            "confidence": "high",
            "generated_at": 1_780_358_400u32
        })
        .to_string();

        let pack = DataPack::from_json(&payload).expect("backend data pack");

        assert_eq!(pack.quality.champion_rates, 172);
        assert_eq!(pack.quality.matchups, 1200);
        assert_eq!(pack.confidence.as_deref(), Some("high"));
        assert_eq!(pack.generated_at, Some(1_780_358_400));
    }

    #[test]
    fn fallback_data_pack_marks_low_confidence() {
        let mut recommendation = sample_recommendation();
        let data_pack = local_seed_data_pack();

        upgrade_recommendation_with_context(
            &mut recommendation,
            Some(&local_rules_model_pack()),
            Some(&data_pack),
        );

        assert_eq!(recommendation.build_confidence, "low");
        assert_eq!(recommendation.matchup_confidence, "low");
        assert!(recommendation
            .data_sources
            .iter()
            .any(|source| source.source == "manual_seed"));
    }

    #[test]
    fn combine_confidence_is_weakest_link_and_skips_not_applicable() {
        // Strong personal comfort must not survive thin data.
        assert_eq!(combine_confidence(&["high", "medium", "high"]), "medium");
        assert_eq!(combine_confidence(&["high", "low", "high"]), "low");
        // "none"/empty (e.g. blind pick: no opponent) are skipped, not dragged down.
        assert_eq!(combine_confidence(&["high", "high", "none"]), "high");
        assert_eq!(combine_confidence(&["medium", "", ""]), "medium");
        // "heuristic" counts as low (opponent known, no real data).
        assert_eq!(combine_confidence(&["high", "heuristic"]), "low");
        // all unknown → conservative low.
        assert_eq!(combine_confidence(&["none", ""]), "low");
    }

    #[test]
    fn recommendations_are_sorted_after_upgrade() {
        let mut low = sample_recommendation();
        low.champion_name = "Low".to_string();
        low.total_score = 0.30;
        low.comfort_score = 0.30;

        let mut high = sample_recommendation();
        high.champion_name = "High".to_string();
        high.total_score = 0.80;
        high.comfort_score = 0.95;
        high.matchup_score = 0.90;

        let mut recommendations = vec![low, high];
        upgrade_recommendations(&mut recommendations);

        assert_eq!(recommendations[0].champion_name, "High");
        assert!(recommendations[1]
            .why_not
            .iter()
            .any(|note| note.contains("yerine High")));
    }

    #[test]
    fn draft_plan_signals_sharpen_model_score() {
        let mut safe = sample_recommendation();
        safe.draft_plan = Some(sample_draft_plan());
        safe.phase_matchup = Some([0.70, 0.72, 0.56]);

        let mut risky = sample_recommendation();
        let mut plan = sample_draft_plan();
        plan.blind_pick_safety = 0.18;
        plan.execution_difficulty = 5;
        plan.fills_team_need.clear();
        risky.draft_plan = Some(plan);
        risky.phase_matchup = Some([0.30, 0.38, 0.42]);

        upgrade_recommendation(&mut safe, Some(&local_rules_model_pack()));
        upgrade_recommendation(&mut risky, Some(&local_rules_model_pack()));

        assert!(safe.model_score > risky.model_score);
        assert!(safe
            .score_breakdown
            .iter()
            .any(|row| row.key == "blind_safety"));
        assert!(safe
            .decision_sentence
            .contains("Lane: ilk 3 dalgada farmı koru"));
    }

    #[test]
    fn why_not_notes_are_deduplicated() {
        let mut recommendation = sample_recommendation();
        recommendation.why_not = vec![
            "Matchup sinyali düşük.".to_string(),
            "Matchup sinyali düşük".to_string(),
            "Risk yönetimi zayıf.".to_string(),
        ];

        upgrade_recommendation(&mut recommendation, Some(&local_rules_model_pack()));

        let matchup_notes = recommendation
            .why_not
            .iter()
            .filter(|note| note.starts_with("Matchup sinyali"))
            .count();
        assert_eq!(matchup_notes, 1);
    }

    #[test]
    fn coaching_output_passes_quality_audit() {
        let mut recommendation = sample_recommendation();
        recommendation.draft_plan = Some(sample_draft_plan());
        recommendation.phase_matchup = Some([0.70, 0.72, 0.56]);

        upgrade_recommendation(&mut recommendation, Some(&local_rules_model_pack()));

        let issues = coach_quality::audit_coaching(
            &recommendation.decision_sentence,
            recommendation.lane_plan.as_deref(),
            recommendation.teamfight_job.as_deref(),
            &recommendation.why_not,
        );
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn lane_and_macro_prose_are_grounded_in_the_matchup() {
        let mut rec = sample_recommendation();
        rec.lane_opponent_name = Some("Ahri".to_string());
        rec.draft_plan = Some(sample_draft_plan()); // fills_team_need = ["AD burst", …]

        upgrade_recommendation(&mut rec, Some(&local_rules_model_pack()));

        // Matchup grounding: the lane plan names the resolved opponent…
        let lane = rec.lane_plan.as_deref().unwrap_or_default();
        assert!(lane.contains("Ahri"), "lane plan not grounded: {lane}");
        // …and that flows into the decision sentence's "Lane:" clause.
        assert!(
            rec.decision_sentence.contains("Ahri"),
            "decision sentence not grounded: {}",
            rec.decision_sentence
        );
        // Team-need grounding: the macro line names the specific comp gap filled.
        let mid = rec.mid_game_plan.as_deref().unwrap_or_default();
        assert!(mid.contains("AD burst"), "mid plan not grounded: {mid}");
        // Grounding must not break the over-promising / quality guardrail.
        assert!(coach_quality::audit_coaching(
            &rec.decision_sentence,
            rec.lane_plan.as_deref(),
            rec.teamfight_job.as_deref(),
            &rec.why_not,
        )
        .is_empty());
    }

    #[test]
    fn blind_pick_lane_plan_invents_no_opponent() {
        let mut rec = sample_recommendation();
        rec.lane_opponent_name = None; // blind / first-pick / ARAM
        rec.draft_plan = Some(sample_draft_plan());
        upgrade_recommendation(&mut rec, Some(&local_rules_model_pack()));
        let lane = rec.lane_plan.as_deref().unwrap_or_default();
        assert!(
            !lane.contains("karşısında:"),
            "blind pick must not fabricate an opponent frame: {lane}"
        );
    }

    #[test]
    fn macro_plan_is_grounded_in_the_build() {
        let mut rec = sample_recommendation();
        rec.core_item_name = Some("Kraken Slayer".to_string());
        rec.build_source = "seed".to_string();
        rec.draft_plan = Some(sample_draft_plan());
        upgrade_recommendation(&mut rec, Some(&local_rules_model_pack()));
        let mid = rec.mid_game_plan.as_deref().unwrap_or_default();
        assert!(mid.contains("Kraken Slayer"), "build not grounded: {mid}");
        // Honest provenance for a curated seed build.
        assert!(mid.contains("seed build"), "missing provenance: {mid}");
    }

    #[test]
    fn macro_plan_without_build_invents_no_item() {
        let mut rec = sample_recommendation();
        rec.core_item_name = None;
        rec.draft_plan = Some(sample_draft_plan());
        upgrade_recommendation(&mut rec, Some(&local_rules_model_pack()));
        let mid = rec.mid_game_plan.as_deref().unwrap_or_default();
        assert!(
            !mid.contains("İlk item"),
            "no build data must not fabricate an item note: {mid}"
        );
    }

    #[test]
    fn headline_is_a_crisp_capitalized_lead() {
        let mut rec = sample_recommendation();
        rec.draft_plan = Some(sample_draft_plan());
        upgrade_recommendation(&mut rec, Some(&local_rules_model_pack()));
        let headline = rec.headline.as_deref().expect("headline set");
        assert!(!headline.is_empty());
        // Crisper than the full decision sentence (lead only, no combo/lane/risk tail).
        assert!(
            headline.len() < rec.decision_sentence.len(),
            "headline '{headline}' should be shorter than the decision sentence"
        );
        // Capitalized + never over-promising.
        assert!(
            headline.chars().next().unwrap().is_uppercase(),
            "{headline}"
        );
        assert!(
            !coach_quality::has_absolute_language(headline),
            "{headline}"
        );
    }
}
