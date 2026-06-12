use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Default weights are now in ScoringWeights::default() (scoring.rs)

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub enum ComboType {
    EngageFollowup,
    PickPotential,
    ZoneControl,
    PeelChain,
    Wombo,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct ComboHint {
    pub ally_champion_id: u32,
    pub ally_champion_key: String,
    /// Turkish display text shown in the Draft Plan panel.
    pub combo_text: String,
    /// Ability-by-ability mechanical breakdown (e.g. "Malphite R (Unstoppable
    /// Force) AOE knockup → Miss Fortune R (Bullet Time) full channel"). Ability
    /// names stay English per the project language convention (technical terms).
    /// Surfaced as the deep-dive "how to execute" detail under `combo_text`.
    pub ability_ref: String,
    pub combo_type: ComboType,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DraftPlan {
    pub combo_with: Vec<ComboHint>,
    pub win_condition: String,
    pub team_role: String,
    /// Human-readable damage label, e.g. "AP burst" / "AD sustained" / "Mixed".
    pub damage_profile: String,
    /// 0.0–1.0 — how safe this champion is to pick without seeing enemy lineup.
    pub blind_pick_safety: f32,
    /// 1 (easiest) to 5 (hardest).
    pub execution_difficulty: u8,
    pub threats: Vec<String>,
    pub fills_team_need: Vec<String>,
    /// Present only for stretch picks (comfort_score < 0.10 boosted by combo).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_note: Option<String>,
    /// Late-pick counter-pick window note — present when pick_order ≥ 4 and the
    /// candidate counters at least one visible enemy archetype.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pick_window_note: Option<String>,
    /// Structural clash between candidate's win condition and detected enemy comp.
    /// e.g. "Pick-comp'e karşı poke: vision basın, izole durmayın"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comp_clash_note: Option<String>,
    /// Power-spike / item-breakpoint advisory derived from power_curve + archetype.
    /// e.g. "2–3 item sonrası dominant — erken baskıyı savun"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spike_note: Option<String>,
    /// Lane-phase micro-coaching (0-15dk): freeze / push / lvl 2 all-in / lvl 6 roam.
    /// Combines matchup (vs lane opponent) with position+archetype patterns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_phase_advice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct DataSourceBadge {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_size: Option<u32>,
    pub confidence: String,
    pub risk_level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct ScoreBreakdownItem {
    pub key: String,
    pub label: String,
    pub value: f32,
    pub weight: f32,
    pub contribution: f32,
    pub confidence: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../src/types/generated/")]
pub enum Tier {
    S,
    A,
    B,
    C,
}

impl Tier {
    pub fn from_score(score: f32) -> Self {
        if score >= 0.75 {
            Tier::S
        } else if score >= 0.65 {
            Tier::A
        } else if score >= 0.55 {
            Tier::B
        } else {
            Tier::C
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct Recommendation {
    pub champion_id: u32,
    pub champion_key: String,
    pub champion_name: String,
    pub total_score: f32,
    pub comfort_score: f32,
    /// Mechanical comfort from Champion Mastery ALONE (no win-rate), in 0..1.
    /// Paired with `draft_winrate` so the UI separates "can pilot the kit" from
    /// "draft win-rate signal". Set by the engine.
    #[serde(default)]
    pub mechanical_comfort: f32,
    /// Bayesian-shrunk personal win-rate in 0..1, or `None` when the player has no
    /// games on the champion — the honest "no draft-fit signal" state (not a
    /// neutral 0.5). Set by the engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_winrate: Option<f32>,
    /// Lane matchup vs opposite-lane opponent (renamed from `lane_counter_score`).
    pub matchup_score: f32,
    pub team_counter_score: f32,
    pub synergy_score: f32,
    pub meta_score: f32,
    /// How well the champion's roles fit the assigned position.
    pub role_fit_score: f32,
    /// Risk penalty in [0.0, 0.10] (subtracted from total via weights.risk).
    pub risk_score: f32,
    pub reason: String,
    /// Punchy one-line headline for the hero card — champion + the single strongest
    /// grounded driver(s). The fuller `decision_sentence` is the secondary "why".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headline: Option<String>,
    #[serde(default)]
    pub decision_sentence: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub score_breakdown: Vec<ScoreBreakdownItem>,
    #[serde(default)]
    pub model_score: f32,
    #[serde(default)]
    pub rules_score: f32,
    #[serde(default)]
    pub model_version: String,
    /// Provenance of the build fields below: `"seed"` (curated `builds_seed.json`),
    /// `"general"` (archetype heuristic — labeled "Genel öneri" in the UI, no
    /// winrate claim), or `"none"` (no build data). Set by the command layer.
    #[serde(default)]
    pub build_source: String,
    #[serde(default)]
    pub build_confidence: String,
    /// Short honest rationale for the build (core intent + lane-opponent counter).
    /// Set by the command layer; `None` when the archetype is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_note: Option<String>,
    #[serde(default)]
    pub matchup_confidence: String,
    /// Signals that fell back to a neutral placeholder because real data was absent
    /// (`"meta"` / `"matchup"` / `"build"`). Lets the UI say "this pick lacks X data"
    /// honestly instead of implying a computed score. Set by the command layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_signals: Vec<String>,
    /// Pro-play presence (pick% + ban%) from Leaguepedia, when available — drives
    /// the "pro heat" badge alongside the ranked signal. Set by the command layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pro_presence: Option<f32>,
    /// D1 kişisel form sinyali (0..1; 0.5 = rol baseline'ında): bu şampiyondaki
    /// CS/dk + ölüm-oranı deltalarının Bayesian-shrunk okuması. `None` = bu rolde
    /// bu şampiyonla maç yok (dürüst — missing_signals'a düşer). json_api katmanı
    /// kurar ve total_score'a SINIRLI (±0.05) nudge uygular.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_form_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mid_game_plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teamfight_plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teamfight_job: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_plan: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub why_not: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_sources: Vec<DataSourceBadge>,
    #[serde(default)]
    pub coach_depth: String,
    /// Placeholder — will be populated in Sprint 3b from builds_repo
    pub core_items: Vec<u32>,
    pub situational_items: Vec<u32>,
    /// 0 = not set
    pub primary_rune_tree: u32,
    pub keystone: u32,
    /// Skill-order display text (e.g. "Q→W→E"). Absent when no build data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_order: Option<String>,
    /// Summoner spell IDs `[spell1, spell2]` (e.g. `[4, 12]` = Flash + Teleport).
    /// Empty when no build data; UI hides the section.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summoner_spells: Vec<u32>,
    /// Secondary rune path `[tree_id, rune1_id, rune2_id]`. Empty when no data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary_runes: Vec<u32>,
    /// Stat shard IDs `[offense, flex, defense]`. Empty when no data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stat_shards: Vec<u32>,
    pub tier: Tier,
    pub confidence: String, // "high" / "medium" / "low"
    /// What drove `confidence`: "sample_depth" (games on champ), "signal_convergence"
    /// (signals conflicted, so confidence was downgraded), or "games_only". Lets the
    /// UI explain the confidence honestly instead of implying a probability. Set by
    /// the engine.
    #[serde(default)]
    pub confidence_basis: String,
    pub games_on_champ: u32,
    pub wins_on_champ: u32,
    /// Turkish summary of enemy team composition, e.g. "AP ağırlıklı · frontline yok"
    pub enemy_team_summary: String,
    /// Display name of the resolved lane opponent (exact or inferred), when visible.
    /// Lets the sentence builders ground prose in the specific matchup, e.g.
    /// "Ahri karşısında …". `None` for blind / first-pick / ARAM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_opponent_name: Option<String>,
    /// Display name of the first core build item (command-resolved from the item cache).
    /// Lets the macro prose ground the build, e.g. "İlk item Kraken Slayer …". `None`
    /// when there is no build data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_item_name: Option<String>,
    /// Draft IQ analysis; None until DI-4b wires the analyzer into the engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_plan: Option<DraftPlan>,
    /// Phase-based matchup advantage [early, mid, late] in [0.0, 1.0].
    /// Present when opponent laner is visible and KB power-curve data exists. 0.5 = neutral.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_matchup: Option<[f32; 3]>,
}
