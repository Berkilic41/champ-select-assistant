//! JSON-in / JSON-out API surface — the boundary the JS hosts call.
//!
//! Each `*_from_json` function is a pure wrapper: parse input JSON → run the
//! engine → serialize output JSON. The same functions back the `#[wasm_bindgen]`
//! exports (Electron main / Cloudflare Worker) and the native unit tests, so
//! WASM↔native parity is structural, not aspirational.
//!
//! Map keys that are tuples in Rust (`(u32, String)`, `(u32, u32)`) cross the
//! boundary as entry arrays (`meta_rates`, `matchups`) because JSON objects only
//! have string keys.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::build_advisor::{counter_item_advice, heuristic_build};
use crate::champion_types::{archetype_position_fit, flex_positions};
use crate::draft_brain::{
    local_rules_model_pack, local_seed_data_pack, upgrade_recommendation_with_context,
    upgrade_recommendations_with_context, DataPack, ModelPack,
};
use crate::draft_iq::archetype::{ChampionArchetype, DamageProfile, PowerCurve};
use crate::draft_simulator::{
    DamageType, DraftSimInput, DraftSimMove, DraftSimState, SimChampion,
};
use crate::draft_iq::game_plan::{compute_game_plan, GamePlan};
use crate::draft_iq::narrative::{build_matchup_tips, build_rationale};
use crate::draft_iq::DraftKnowledgeBase;
use crate::models::Recommendation;
use crate::feedback_analytics::{analyze_feedback, FeedbackEvent};
use crate::feedback_observability::{
    personalization_status, summarize_observability, FeedbackObservability,
    FeedbackPersonalizationStatus,
};
use crate::feedback_signal::{aggregate_feedback, FeedbackInput, FeedbackSignal};
use crate::pool_builder::suggest_pool;
use crate::pool_coach::{analyze_pool, PoolChampion, PoolCoachInput};
use crate::rate_blend::{self, SourceRate};
use crate::scoring::{
    matchup_score, resolve_lane_opponent_raw, MatchupEntry, MetaRate, ScoringContext,
    ScoringWeights,
};
use crate::team_analysis::{compute_comp_summary, CompSummary, TeamCompBoard, TeamComposition};
use crate::types::{
    ChampSelectState, ChampionRecord, ChampionStats, ItemData, MasteryRow, RuneTree, TeamSlot,
};

/// `meta_rates` entry: tuple key `(champion_id, position)` flattened next to the rate.
/// `Serialize` is additive — `blended_meta_rates_from_json` emits this exact shape,
/// so the host can feed its output straight back into `RecommendationsInput`.
#[derive(Debug, Serialize, Deserialize)]
pub struct MetaRateEntry {
    pub champion_id: u32,
    /// LCU position: "top" | "jungle" | "middle" | "bottom" | "utility".
    pub position: String,
    #[serde(flatten)]
    pub rate: MetaRate,
}

/// `matchups` entry: tuple key `(champion_id, opponent_id)` flattened next to the stats.
#[derive(Debug, Deserialize)]
pub struct MatchupKeyEntry {
    pub champion_id: u32,
    pub opponent_id: u32,
    #[serde(flatten)]
    pub entry: MatchupEntry,
}

/// JSON twin of [`FeedbackSignal`] — `confidence` arrives as a plain string and is
/// mapped onto the engine's `&'static str` ("low" | "medium" | "high"; anything
/// else degrades to "low" rather than failing the whole request).
#[derive(Debug, Deserialize)]
pub struct FeedbackSignalEntry {
    pub champion_id: u32,
    #[serde(default)]
    pub positive: u32,
    #[serde(default)]
    pub negative: u32,
    #[serde(default)]
    pub sample: u32,
    #[serde(default)]
    pub net_sentiment: f32,
    #[serde(default)]
    pub confidence: String,
    #[serde(default)]
    pub suggested_delta: f32,
}

impl FeedbackSignalEntry {
    fn into_signal(self) -> FeedbackSignal {
        let confidence: &'static str = match self.confidence.as_str() {
            "high" => "high",
            "medium" => "medium",
            _ => "low",
        };
        FeedbackSignal {
            champion_id: self.champion_id,
            positive: self.positive,
            negative: self.negative,
            sample: self.sample,
            net_sentiment: self.net_sentiment,
            confidence,
            suggested_delta: self.suggested_delta,
        }
    }
}

/// Everything `engine::compute_recommendations` needs, owned. The host gathers
/// this from its I/O (LCU session, SQLite repos, DDragon cache) and ships one
/// JSON document.
#[derive(Debug, Deserialize)]
pub struct RecommendationsInput {
    pub session: ChampSelectState,
    pub weights: ScoringWeights,
    pub all_champions: Vec<ChampionRecord>,
    #[serde(default)]
    pub mastery: Vec<MasteryRow>,
    #[serde(default)]
    pub stats: Vec<ChampionStats>,
    /// champion_id → CDragon roles ("fighter", "mage", …).
    #[serde(default)]
    pub role_map: HashMap<u32, Vec<String>>,
    #[serde(default)]
    pub meta_rates: Vec<MetaRateEntry>,
    #[serde(default)]
    pub matchups: Option<Vec<MatchupKeyEntry>>,
    /// champion_id → KB power curve. Optional enhancement for matchup fallback.
    #[serde(default)]
    pub power_curves: Option<HashMap<u32, PowerCurve>>,
    #[serde(default)]
    pub feedback_signals: Option<Vec<FeedbackSignalEntry>>,
    #[serde(default)]
    pub items: Vec<ItemData>,
    #[serde(default)]
    pub rune_trees: Vec<RuneTree>,
    /// `builds` table rows for the player's lane, host-sorted `cached_at DESC`
    /// (so first-match == Rust's `ORDER BY cached_at DESC LIMIT 1`). Empty =
    /// no curated builds → archetype heuristic fallback ("general").
    #[serde(default)]
    pub builds: Vec<BuildRowEntry>,
    /// `champion_rates` rows for the synthetic "pro" position. Core filters
    /// `source == "leaguepedia"` and attaches `pick_rate + ban_rate` per rec.
    #[serde(default)]
    pub pro_rows: Vec<ProPresenceRow>,
    /// Raw `draft_brain_packs` payloads (kind = "model_pack" / "data_pack").
    /// Unparseable/absent → local rules / local seed fallback (Rust
    /// `active_model_pack`/`active_data_pack` parity).
    #[serde(default)]
    pub model_pack_payload: Option<String>,
    #[serde(default)]
    pub data_pack_payload: Option<String>,
}

/// One `builds` table row as the host reads it (JSON-string columns kept raw —
/// core parses them exactly like the Rust command layer did).
#[derive(Debug, Deserialize)]
pub struct BuildRowEntry {
    pub champion_id: i64,
    /// JSON-serialised `Vec<u32>`.
    pub item_ids: String,
    /// JSON-serialised `Vec<u32>` — `[keystone_id, primary_tree_id]`.
    pub rune_ids: String,
    /// Archetype of the expected lane opponent; `None` = position default.
    #[serde(default)]
    pub opponent_archetype: Option<String>,
    #[serde(default)]
    pub skill_order: Option<String>,
    /// JSON-serialised `Vec<u32>` — `[spell1_id, spell2_id]`.
    #[serde(default)]
    pub summoner_spells: Option<String>,
    /// JSON-serialised `Vec<u32>` — `[secondary_tree_id, rune1_id, rune2_id]`.
    #[serde(default)]
    pub secondary_runes: Option<String>,
    /// JSON-serialised `Vec<u32>` — `[offense, flex, defense]`.
    #[serde(default)]
    pub stat_shards: Option<String>,
}

/// One `champion_rates` row at position "pro" (Leaguepedia presence source).
#[derive(Debug, Deserialize)]
pub struct ProPresenceRow {
    pub champion_id: u32,
    #[serde(default)]
    pub pick_rate: f32,
    #[serde(default)]
    pub ban_rate: f32,
    #[serde(default)]
    pub source: String,
}

/// Input for `ban_advisor::compute_ban_suggestions`.
#[derive(Debug, Deserialize)]
pub struct BanSuggestionsInput {
    pub session: ChampSelectState,
    pub all_champions: Vec<ChampionRecord>,
    #[serde(default)]
    pub meta_rates: Vec<MetaRateEntry>,
    /// The local player's own champion pool (mastery rows) — used to avoid
    /// suggesting bans the player would rather pick.
    #[serde(default)]
    pub my_pool: Vec<MasteryRow>,
    #[serde(default)]
    pub role_map: HashMap<u32, Vec<String>>,
}

/// Input for `draft_verdict::compute_draft_verdict`.
#[derive(Debug, Deserialize)]
pub struct DraftVerdictInput {
    pub plan: GamePlan,
    pub ally: CompSummary,
    pub enemy: CompSummary,
    #[serde(default)]
    pub lane_matchup: Option<f32>,
}

/// One `(champion_id, position)` sample total — the host's
/// `position_sample_totals` query result, used for the role-share filter.
#[derive(Debug, Deserialize)]
pub struct PositionSampleEntry {
    pub champion_id: u32,
    pub position: String,
    pub sample_size: u32,
}

/// Input for [`blended_meta_rates_from_json`]: raw per-source `champion_rates`
/// rows for ONE lane + cross-lane sample totals for the role-share filter.
#[derive(Debug, Deserialize)]
pub struct BlendedMetaRatesInput {
    pub rows: Vec<SourceRate>,
    /// ALL positions (not just the queried lane) — role share is cross-lane.
    #[serde(default)]
    pub position_samples: Vec<PositionSampleEntry>,
}

fn meta_rates_map(entries: Vec<MetaRateEntry>) -> HashMap<(u32, String), MetaRate> {
    entries
        .into_iter()
        .map(|e| ((e.champion_id, e.position), e.rate))
        .collect()
}

/// Owned, engine-ready form of [`RecommendationsInput`]. Several endpoints
/// (recommendations / champion analysis / counter picks / full draft verdict)
/// take the SAME input document; this struct owns the converted maps and lends
/// out a [`ScoringContext`].
struct EngineInputs {
    session: ChampSelectState,
    weights: ScoringWeights,
    all_champions: Vec<ChampionRecord>,
    mastery: Vec<MasteryRow>,
    stats: Vec<ChampionStats>,
    role_map: HashMap<u32, Vec<String>>,
    meta_rates: HashMap<(u32, String), MetaRate>,
    matchups: Option<HashMap<(u32, u32), MatchupEntry>>,
    power_curves: Option<HashMap<u32, PowerCurve>>,
    feedback_signals: Option<HashMap<u32, FeedbackSignal>>,
    items: Vec<ItemData>,
    rune_trees: Vec<RuneTree>,
}

impl EngineInputs {
    fn from_input(input: RecommendationsInput, kb: &DraftKnowledgeBase) -> Self {
        let meta_rates = meta_rates_map(input.meta_rates);
        let matchups: Option<HashMap<(u32, u32), MatchupEntry>> = input.matchups.map(|v| {
            v.into_iter()
                .map(|m| ((m.champion_id, m.opponent_id), m.entry))
                .collect()
        });
        let feedback_signals: Option<HashMap<u32, FeedbackSignal>> =
            input.feedback_signals.map(|v| {
                v.into_iter()
                    .map(|e| (e.champion_id, e.into_signal()))
                    .collect()
            });
        // power_curves verilmediyse KB arketiplerinden türet (Rust host'un
        // load_scoring_inputs'taki davranışı — JS host KB'yi göremez, KB WASM içinde).
        let power_curves: Option<HashMap<u32, PowerCurve>> = input.power_curves.or_else(|| {
            Some(
                input
                    .all_champions
                    .iter()
                    .filter_map(|c| {
                        kb.get_archetype(&c.key)
                            .map(|a| (c.champion_id as u32, a.power_curve.clone()))
                    })
                    .collect(),
            )
        });
        Self {
            session: input.session,
            weights: input.weights,
            all_champions: input.all_champions,
            mastery: input.mastery,
            stats: input.stats,
            role_map: input.role_map,
            meta_rates,
            matchups,
            power_curves,
            feedback_signals,
            items: input.items,
            rune_trees: input.rune_trees,
        }
    }

    fn ctx(&self) -> ScoringContext<'_> {
        ScoringContext {
            session: &self.session,
            mastery: &self.mastery,
            stats: &self.stats,
            role_map: &self.role_map,
            weights: self.weights,
            meta_rates: &self.meta_rates,
            matchups: self.matchups.as_ref(),
            power_curves: self.power_curves.as_ref(),
            feedback_signals: self.feedback_signals.as_ref(),
        }
    }

    /// Lane key (`load_scoring_inputs` parity): ARAM → synthetic "aram",
    /// otherwise the assigned LCU position lowercase.
    fn my_pos(&self) -> String {
        if self.session.queue_id == 450 {
            "aram".to_string()
        } else {
            self.session.local_player.assigned_position.to_lowercase()
        }
    }
}

/// Post-processing inputs split off [`RecommendationsInput`] before the rest is
/// consumed by [`EngineInputs::from_input`]. These back the Rust command layer's
/// enrichment (champ_select.rs get_recommendations / get_champion_analysis).
struct EnrichmentInputs {
    builds: Vec<BuildRowEntry>,
    pro_rows: Vec<ProPresenceRow>,
    model_pack_payload: Option<String>,
    data_pack_payload: Option<String>,
}

impl EnrichmentInputs {
    fn take(input: &mut RecommendationsInput) -> Self {
        Self {
            builds: std::mem::take(&mut input.builds),
            pro_rows: std::mem::take(&mut input.pro_rows),
            model_pack_payload: input.model_pack_payload.take(),
            data_pack_payload: input.data_pack_payload.take(),
        }
    }

    /// `active_model_pack` parity: cached payload when parseable, else local rules.
    fn model_pack(&self) -> ModelPack {
        self.model_pack_payload
            .as_deref()
            .and_then(|p| ModelPack::from_json(p).ok())
            .unwrap_or_else(local_rules_model_pack)
    }

    /// `active_data_pack` parity: cached payload when parseable, else the honest
    /// local-seed fallback. (Rust additionally built a coverage-based local pack
    /// from DB counts; in the Electron host the scheduler persists exactly that
    /// pack into `draft_brain_packs`, so the cached-payload path covers it.)
    fn data_pack(&self) -> DataPack {
        self.data_pack_payload
            .as_deref()
            .and_then(|p| DataPack::from_json(p).ok())
            .unwrap_or_else(local_seed_data_pack)
    }
}

/// Archetype of the enemy laner at `my_pos`, when visible (champ_select.rs port).
fn resolve_enemy_archetype(
    session: &ChampSelectState,
    all_champions: &[ChampionRecord],
    kb: &DraftKnowledgeBase,
    my_pos: &str,
) -> Option<String> {
    session
        .their_team
        .iter()
        .find(|s| s.assigned_position.to_lowercase() == my_pos && s.champion_id != 0)
        .and_then(|s| {
            all_champions
                .iter()
                .find(|c| c.champion_id == s.champion_id as i64)
                .map(|c| c.key.clone())
        })
        .and_then(|key| kb.get_archetype(&key).map(|a| a.archetype.clone()))
}

/// Attach build data (core items, runes, summoner spells, shards) to a single
/// recommendation (`enrich_build_for_rec` port). Order of preference:
///   1. Matchup-specific curated row (`build_source = "seed"`).
///   2. Position-default curated row (`opponent_archetype` NULL, also "seed").
///   3. General archetype heuristic (`build_source = "general"`).
/// Leaves `build_source = "none"` only when the archetype is unknown.
fn enrich_build(
    rec: &mut Recommendation,
    builds: &[BuildRowEntry],
    enemy_archetype: Option<&str>,
    kb: &DraftKnowledgeBase,
) {
    let cand_arch = kb.get_archetype(&rec.champion_key);
    let row = enemy_archetype
        .and_then(|arch| {
            builds.iter().find(|b| {
                b.champion_id == rec.champion_id as i64
                    && b.opponent_archetype.as_deref() == Some(arch)
            })
        })
        .or_else(|| {
            builds
                .iter()
                .find(|b| b.champion_id == rec.champion_id as i64 && b.opponent_archetype.is_none())
        });
    if let Some(build) = row {
        if let Ok(ids) = serde_json::from_str::<Vec<u32>>(&build.item_ids) {
            rec.core_items = ids.into_iter().take(4).collect();
        }
        if let Ok(rids) = serde_json::from_str::<Vec<u32>>(&build.rune_ids) {
            rec.keystone = rids.first().copied().unwrap_or(0);
            rec.primary_rune_tree = rids.get(1).copied().unwrap_or(0);
        }
        rec.skill_order = build.skill_order.clone();
        if let Some(s) = &build.summoner_spells {
            rec.summoner_spells = serde_json::from_str(s).unwrap_or_default();
        }
        if let Some(s) = &build.secondary_runes {
            rec.secondary_runes = serde_json::from_str(s).unwrap_or_default();
        }
        if let Some(s) = &build.stat_shards {
            rec.stat_shards = serde_json::from_str(s).unwrap_or_default();
        }
        rec.build_source = "seed".to_string();
        rec.build_confidence = "high".to_string();
        rec.build_note = cand_arch.map(|a| build_rationale(a, enemy_archetype, "seed"));
        return;
    }

    // No curated row for this champion → general archetype build.
    if let Some(a) = cand_arch {
        if let Some(b) = heuristic_build(&a.archetype) {
            rec.core_items = b.core_items.into_iter().take(4).collect();
            rec.keystone = b.keystone;
            rec.primary_rune_tree = b.primary_tree;
            rec.secondary_runes = b.secondary_runes;
            rec.stat_shards = b.stat_shards;
            rec.summoner_spells = b.summoner_spells;
            // skill_order intentionally left None — we don't fabricate it.
            rec.build_source = "general".to_string();
            rec.build_confidence = "medium".to_string();
            rec.build_note = Some(build_rationale(a, enemy_archetype, "general"));
        }
    }
}

/// Display name of the first core item, from the host's item cache.
fn resolve_core_item_name(rec: &Recommendation, items: &[ItemData]) -> Option<String> {
    let id = *rec.core_items.first()?;
    items.iter().find(|i| i.id == id).map(|i| i.name.clone())
}

/// Honest "what data is this pick missing" flags (champ_select.rs port). Computed
/// BEFORE `upgrade_*` overwrites the engine's per-champ matchup confidence.
fn compute_missing_signals(
    rec: &Recommendation,
    meta_rates: &HashMap<(u32, String), MetaRate>,
    role: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    if !meta_rates.contains_key(&(rec.champion_id, role.to_string())) {
        out.push("meta".to_string());
    }
    if rec.matchup_confidence == "heuristic" {
        out.push("matchup".to_string());
    }
    if matches!(rec.build_source.as_str(), "general" | "none" | "") {
        out.push("build".to_string());
    }
    out
}

/// Compute champion recommendations. Input/output are JSON strings; see
/// [`RecommendationsInput`] and `models::Recommendation`.
///
/// Includes the Rust command layer's full post-processing: matchup-aware build
/// enrichment, `core_item_name`, `missing_signals`, `pro_presence` and the
/// DraftBrain pack upgrade (model score, tier, score breakdown, plans, why-not).
pub fn recommendations_from_json(input_json: &str) -> Result<String, String> {
    let mut input: RecommendationsInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid recommendations input: {e}"))?;
    let enrich = EnrichmentInputs::take(&mut input);
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let inputs = EngineInputs::from_input(input, &kb);

    let mut recs = crate::engine::compute_recommendations(
        &inputs.ctx(),
        &inputs.all_champions,
        &inputs.items,
        &inputs.rune_trees,
        &kb,
    );

    let my_pos = inputs.my_pos();
    let enemy_archetype =
        resolve_enemy_archetype(&inputs.session, &inputs.all_champions, &kb, &my_pos);
    // Pro-play presence (pick% + ban% from Leaguepedia), stored under the
    // synthetic "pro" position so it never mixes into the ranked per-role blend.
    let pro_presence: HashMap<u32, f32> = enrich
        .pro_rows
        .iter()
        .filter(|r| r.source == "leaguepedia")
        .map(|r| (r.champion_id, r.pick_rate + r.ban_rate))
        .collect();
    for rec in &mut recs {
        enrich_build(rec, &enrich.builds, enemy_archetype.as_deref(), &kb);
        rec.core_item_name = resolve_core_item_name(rec, &inputs.items);
        rec.missing_signals = compute_missing_signals(rec, &inputs.meta_rates, &my_pos);
        rec.pro_presence = pro_presence.get(&rec.champion_id).copied();
    }
    upgrade_recommendations_with_context(
        &mut recs,
        Some(&enrich.model_pack()),
        Some(&enrich.data_pack()),
    );

    serde_json::to_string(&recs).map_err(|e| format!("recommendations serialize failed: {e}"))
}

/// Compute ban suggestions. Output is a JSON array of `ban_advisor::BanSuggestion`.
pub fn ban_suggestions_from_json(input_json: &str) -> Result<String, String> {
    let input: BanSuggestionsInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid ban suggestions input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;

    let meta_rates = meta_rates_map(input.meta_rates);
    let suggestions = crate::ban_advisor::compute_ban_suggestions(
        &input.session,
        &input.all_champions,
        &meta_rates,
        &input.my_pool,
        &input.role_map,
        &kb,
    );
    serde_json::to_string(&suggestions).map_err(|e| format!("ban suggestions serialize failed: {e}"))
}

/// Compute the draft verdict. Output is one `draft_verdict::DraftVerdict` object.
pub fn draft_verdict_from_json(input_json: &str) -> Result<String, String> {
    let input: DraftVerdictInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid draft verdict input: {e}"))?;
    let verdict = crate::draft_verdict::compute_draft_verdict(
        &input.plan,
        &input.ally,
        &input.enemy,
        input.lane_matchup,
    );
    serde_json::to_string(&verdict).map_err(|e| format!("draft verdict serialize failed: {e}"))
}

/// Analyze the player's champion pool. Input is one `pool_coach::PoolCoachInput`;
/// output is one `pool_coach::ChampionPoolPlan`.
pub fn pool_coach_from_json(input_json: &str) -> Result<String, String> {
    let input: crate::pool_coach::PoolCoachInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid pool coach input: {e}"))?;
    let plan = crate::pool_coach::analyze_pool(&input);
    serde_json::to_string(&plan).map_err(|e| format!("pool plan serialize failed: {e}"))
}

/// Build the post-game performance report. Input is a JSON array of
/// `postgame::MatchRow`; output is one `postgame::PerformanceReport`.
pub fn performance_report_from_json(input_json: &str) -> Result<String, String> {
    let matches: Vec<crate::postgame::MatchRow> = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid performance report input: {e}"))?;
    let report = crate::postgame::build_performance_report(&matches);
    serde_json::to_string(&report).map_err(|e| format!("performance report serialize failed: {e}"))
}

/// Compute objective timers / macro state. Input is one
/// `macro_timers::MacroTimerInput`; output is one `macro_timers::MacroState`.
pub fn macro_state_from_json(input_json: &str) -> Result<String, String> {
    let input: crate::macro_timers::MacroTimerInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid macro state input: {e}"))?;
    let state = crate::macro_timers::compute_macro_state(&input);
    serde_json::to_string(&state).map_err(|e| format!("macro state serialize failed: {e}"))
}

/// Parse a raw LCU `/lol-champ-select/v1/session` payload into a serialized
/// `ChampSelectState`. Error message mirrors the Rust host's `parse_session_arg`
/// ("Geçersiz session JSON") so both hosts fail identically.
pub fn parse_session_from_json(input_json: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(input_json).map_err(|_| "Geçersiz session JSON".to_string())?;
    let state =
        crate::session_parse::parse_session(&value).ok_or_else(|| "Geçersiz session JSON".to_string())?;
    serde_json::to_string(&state).map_err(|e| format!("session serialize failed: {e}"))
}

/// Blend per-source `champion_rates` rows into one rate per (champion, position)
/// and apply the cross-lane role-share filter (`ROLE_SHARE_MIN`) — the exact
/// pipeline the Rust host runs in `load_scoring_inputs` (blend_position_rates +
/// role_played_set retain). Output is a `MetaRateEntry` array, sorted by
/// (champion_id, position) for determinism, directly usable as
/// `RecommendationsInput.meta_rates`.
pub fn blended_meta_rates_from_json(input_json: &str) -> Result<String, String> {
    let input: BlendedMetaRatesInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid blended meta rates input: {e}"))?;

    let mut blended = rate_blend::blend_rates(input.rows);
    // Empty position_samples = "no share info, skip the filter" — the ban
    // suggestion path (Rust parity) blends WITHOUT the role-share filter.
    if !input.position_samples.is_empty() {
        let samples: Vec<(u32, String, u32)> = input
            .position_samples
            .into_iter()
            .map(|s| (s.champion_id, s.position, s.sample_size))
            .collect();
        let played = rate_blend::role_played_set(&samples, rate_blend::ROLE_SHARE_MIN);
        blended.retain(|key, _| played.contains(key));
    }

    let mut entries: Vec<MetaRateEntry> = blended
        .into_iter()
        .map(|((champion_id, position), rate)| MetaRateEntry {
            champion_id,
            position,
            rate,
        })
        .collect();
    entries.sort_by(|a, b| {
        (a.champion_id, a.position.as_str()).cmp(&(b.champion_id, b.position.as_str()))
    });
    serde_json::to_string(&entries).map_err(|e| format!("blended meta rates serialize failed: {e}"))
}

/// Aggregate raw `recommendation_feedback` rows (`[{champion_id, verdict}]`) into
/// per-champion signals. Output is a `FeedbackSignal` array in exactly the shape
/// `RecommendationsInput.feedback_signals` accepts back.
pub fn feedback_signals_from_json(input_json: &str) -> Result<String, String> {
    let rows: Vec<FeedbackInput> = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid feedback rows input: {e}"))?;
    let mut signals: Vec<FeedbackSignal> = aggregate_feedback(&rows).into_values().collect();
    signals.sort_by_key(|s| s.champion_id);
    serde_json::to_string(&signals).map_err(|e| format!("feedback signals serialize failed: {e}"))
}

/// The brawl-mode (ARAM/Arena) scoring preset as JSON — single source of truth
/// stays in core (`ScoringWeights::aram()`); the JS host must not copy the values.
pub fn aram_weights_json_string() -> String {
    serde_json::to_string(&ScoringWeights::aram())
        .expect("ScoringWeights serializes infallibly")
}

// ── Champ-select analysis cluster (P1.3b-3) ───────────────────────────────────
// These replicate the Tauri command layer's pure post-I/O logic so the Electron
// host stays I/O-only. The src-tauri copies are legacy: they die with the Tauri
// host at the end of the migration; these are the forward single source.

/// `get_champion_analysis` input marker: the document is a flat
/// [`RecommendationsInput`] with one extra `champion_id` field. NOT modeled with
/// `#[serde(flatten)]` — flatten buffers values as strings and breaks the
/// integer-keyed `role_map` map; instead the same JSON is parsed twice.
#[derive(Debug, Deserialize)]
struct ChampionIdOnly {
    champion_id: u32,
}

/// Input for the session-shaped KB endpoints (team comp / game plan / combo
/// board / lane matchup / counter items). `role_map` and `items` are only
/// consumed by the endpoints that need them.
#[derive(Debug, Deserialize)]
pub struct TeamContextInput {
    pub session: ChampSelectState,
    pub all_champions: Vec<ChampionRecord>,
    #[serde(default)]
    pub role_map: HashMap<u32, Vec<String>>,
    #[serde(default)]
    pub items: Vec<ItemData>,
}

/// Command-layer parity (`get_game_plan` / `get_team_comp` / `get_draft_verdict`):
/// allies = every visible teammate pick + the local hover when nothing is locked;
/// enemies = every visible enemy pick.
fn ally_enemy_ids(session: &ChampSelectState) -> (Vec<u32>, Vec<u32>) {
    let mut ally_ids: Vec<u32> = session
        .my_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();
    if session.local_player.champion_id == 0 && session.local_player.intent_champion_id > 0 {
        ally_ids.push(session.local_player.intent_champion_id);
    }
    let enemy_ids: Vec<u32> = session
        .their_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();
    (ally_ids, enemy_ids)
}

fn archetypes_for<'a>(
    ids: &[u32],
    all_champions: &[ChampionRecord],
    kb: &'a DraftKnowledgeBase,
) -> Vec<&'a ChampionArchetype> {
    ids.iter()
        .filter_map(|&id| {
            all_champions
                .iter()
                .find(|c| c.champion_id == id as i64)
                .and_then(|c| kb.get_archetype(&c.key))
        })
        .collect()
}

/// Full coaching analysis for ONE champion (locked/hovered pick). Output is one
/// `models::Recommendation` or JSON `null` (unknown champion / champion_id 0).
pub fn champion_analysis_from_json(input_json: &str) -> Result<String, String> {
    let ChampionIdOnly { champion_id } = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid champion analysis input: {e}"))?;
    if champion_id == 0 {
        return Ok("null".to_string());
    }
    let mut base: RecommendationsInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid champion analysis input: {e}"))?;
    let enrich = EnrichmentInputs::take(&mut base);
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let inputs = EngineInputs::from_input(base, &kb);

    let mut rec = crate::engine::analyze_champion(
        champion_id,
        &inputs.ctx(),
        &inputs.all_champions,
        &inputs.items,
        &inputs.rune_trees,
        &kb,
    );
    // get_champion_analysis parity: build enrichment + core_item_name + single-rec
    // DraftBrain upgrade (no missing_signals/pro_presence on this path).
    if let Some(rec) = rec.as_mut() {
        let my_pos = inputs.my_pos();
        let enemy_archetype =
            resolve_enemy_archetype(&inputs.session, &inputs.all_champions, &kb, &my_pos);
        enrich_build(rec, &enrich.builds, enemy_archetype.as_deref(), &kb);
        rec.core_item_name = resolve_core_item_name(rec, &inputs.items);
        upgrade_recommendation_with_context(
            rec,
            Some(&enrich.model_pack()),
            Some(&enrich.data_pack()),
        );
    }
    serde_json::to_string(&rec).map_err(|e| format!("champion analysis serialize failed: {e}"))
}

/// Counter-pick hints from the player's mastery pool vs the visible lane
/// opponent. Input is a [`RecommendationsInput`]; the pool is its mastery list.
pub fn counter_picks_from_json(input_json: &str) -> Result<String, String> {
    let input: RecommendationsInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid counter picks input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let inputs = EngineInputs::from_input(input, &kb);

    let pool_ids: Vec<u32> = inputs.mastery.iter().map(|m| m.champion_id as u32).collect();
    let hints =
        crate::counter_pick::compute_counter_picks(&inputs.ctx(), &inputs.all_champions, &kb, &pool_ids);
    serde_json::to_string(&hints).map_err(|e| format!("counter picks serialize failed: {e}"))
}

/// Single decisive read on the draft (`get_draft_verdict` parity): game plan +
/// both comp summaries + the local pick's lane matchup → `DraftVerdict`.
pub fn draft_verdict_full_from_json(input_json: &str) -> Result<String, String> {
    let input: RecommendationsInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid draft verdict input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let inputs = EngineInputs::from_input(input, &kb);

    let (ally_ids, enemy_ids) = ally_enemy_ids(&inputs.session);
    let ally_arch = archetypes_for(&ally_ids, &inputs.all_champions, &kb);
    let enemy_arch = archetypes_for(&enemy_ids, &inputs.all_champions, &kb);

    let plan = compute_game_plan(&ally_arch, &enemy_arch);
    let ally_comp = compute_comp_summary(&ally_ids, &inputs.role_map, &ally_arch);
    let enemy_comp = compute_comp_summary(&enemy_ids, &inputs.role_map, &enemy_arch);

    // Lane matchup only when the local pick exists AND the lane opponent is visible.
    let my_pos = inputs.my_pos();
    let my_pick = if inputs.session.local_player.champion_id > 0 {
        inputs.session.local_player.champion_id
    } else {
        inputs.session.local_player.intent_champion_id
    };
    let opp_visible = inputs
        .session
        .their_team
        .iter()
        .any(|s| s.assigned_position.to_lowercase() == my_pos && s.champion_id > 0);
    let lane_matchup = if my_pick > 0 && opp_visible {
        Some(matchup_score(my_pick, &inputs.ctx()))
    } else {
        None
    };

    let verdict =
        crate::draft_verdict::compute_draft_verdict(&plan, &ally_comp, &enemy_comp, lane_matchup);
    serde_json::to_string(&verdict).map_err(|e| format!("draft verdict serialize failed: {e}"))
}

/// Both teams' composition summaries (`get_team_comp` parity) → `TeamCompBoard`.
pub fn team_comp_from_json(input_json: &str) -> Result<String, String> {
    let input: TeamContextInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid team comp input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;

    let (ally_ids, enemy_ids) = ally_enemy_ids(&input.session);
    let ally_arch = archetypes_for(&ally_ids, &input.all_champions, &kb);
    let enemy_arch = archetypes_for(&enemy_ids, &input.all_champions, &kb);
    let board = TeamCompBoard {
        ally: compute_comp_summary(&ally_ids, &input.role_map, &ally_arch),
        enemy: compute_comp_summary(&enemy_ids, &input.role_map, &enemy_arch),
    };
    serde_json::to_string(&board).map_err(|e| format!("team comp serialize failed: {e}"))
}

/// Team-level macro game plan (`get_game_plan` parity) → `GamePlan`.
pub fn game_plan_from_json(input_json: &str) -> Result<String, String> {
    let input: TeamContextInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid game plan input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;

    let (ally_ids, enemy_ids) = ally_enemy_ids(&input.session);
    let ally_arch = archetypes_for(&ally_ids, &input.all_champions, &kb);
    let enemy_arch = archetypes_for(&enemy_ids, &input.all_champions, &kb);
    let plan = compute_game_plan(&ally_arch, &enemy_arch);
    serde_json::to_string(&plan).map_err(|e| format!("game plan serialize failed: {e}"))
}

/// One ally-combo entry for the synergy board (`get_combo_board` parity).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ComboBoardEntry {
    pub ally_champion_id: u32,
    pub ally_champion_key: String,
    pub name: String,
    /// Turkish combo description (the `tr` field — user-facing, not `ability_ref`).
    pub combo_text: String,
    pub combo_type: String,
    pub strength: f32,
}

/// Known combos between the local pick (locked or hovered) and the locked
/// allies, strongest first. Empty when no pick or no ally combo exists.
pub fn combo_board_from_json(input_json: &str) -> Result<String, String> {
    let input: TeamContextInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid combo board input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let session = &input.session;

    let my_id = if session.local_player.champion_id > 0 {
        session.local_player.champion_id
    } else {
        session.local_player.intent_champion_id
    };
    let empty = || serde_json::to_string::<[ComboBoardEntry]>(&[]).unwrap();
    if my_id == 0 {
        return Ok(empty());
    }
    let Some(my_key) = input
        .all_champions
        .iter()
        .find(|c| c.champion_id == my_id as i64)
        .map(|c| c.key.clone())
    else {
        return Ok(empty());
    };

    let ally: Vec<(u32, String)> = session
        .my_team
        .iter()
        .filter(|s| s.champion_id > 0 && s.champion_id != my_id)
        .filter_map(|s| {
            input
                .all_champions
                .iter()
                .find(|c| c.champion_id == s.champion_id as i64)
                .map(|c| (s.champion_id, c.key.clone()))
        })
        .collect();
    if ally.is_empty() {
        return Ok(empty());
    }
    let ally_keys: Vec<&str> = ally.iter().map(|(_, k)| k.as_str()).collect();

    let mut entries: Vec<ComboBoardEntry> = kb
        .combos
        .find_for_ally(&my_key, &ally_keys)
        .into_iter()
        .filter_map(|combo| {
            let partner = if combo.a.eq_ignore_ascii_case(&my_key) {
                &combo.b
            } else {
                &combo.a
            };
            let (ally_id, ally_key) = ally.iter().find(|(_, k)| partner.eq_ignore_ascii_case(k))?;
            Some(ComboBoardEntry {
                ally_champion_id: *ally_id,
                ally_champion_key: ally_key.clone(),
                name: combo.name.clone(),
                combo_text: combo.tr.clone(),
                combo_type: combo.combo_type.clone(),
                strength: combo.strength,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.strength
            .partial_cmp(&a.strength)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    serde_json::to_string(&entries).map_err(|e| format!("combo board serialize failed: {e}"))
}

/// One champion's KB archetype, for lobby badges (`get_champion_archetypes`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChampionArchetypeInfo {
    pub champion_id: u32,
    pub archetype: String,
}

/// Champion → KB archetype, straight from the embedded KB. Sorted by id.
pub fn champion_archetypes_string() -> Result<String, String> {
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let mut out: Vec<ChampionArchetypeInfo> = kb
        .archetypes
        .values()
        .map(|a| ChampionArchetypeInfo {
            champion_id: a.champion_id,
            archetype: a.archetype.clone(),
        })
        .collect();
    out.sort_by_key(|a| a.champion_id);
    serde_json::to_string(&out).map_err(|e| format!("archetypes serialize failed: {e}"))
}

/// A combo this champion participates in (`get_champion_detail` card).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChampionDetailCombo {
    pub partner_key: String,
    pub name: String,
    pub combo_text: String,
    pub combo_type: String,
    pub strength: f32,
}

/// Full KB profile for one champion (`get_champion_detail` parity).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ChampionDetail {
    pub champion_id: u32,
    pub champion_key: String,
    pub archetype: String,
    pub power_early: f32,
    pub power_mid: f32,
    pub power_late: f32,
    pub win_condition: String,
    pub damage_ad: f32,
    pub damage_ap: f32,
    pub has_hard_cc: bool,
    pub mobility: String,
    pub blind_safety: f32,
    pub execution_difficulty: u8,
    pub utility_tags: Vec<String>,
    pub combos: Vec<ChampionDetailCombo>,
}

/// KB profile for `champion_id`; JSON `null` when the champion has no KB entry.
pub fn champion_detail_string(champion_id: u32) -> Result<String, String> {
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let Some((key, a)) = kb
        .archetypes
        .iter()
        .find(|(_, a)| a.champion_id == champion_id)
    else {
        return Ok("null".to_string());
    };

    let mut combos: Vec<ChampionDetailCombo> = kb
        .combos
        .all_pairs()
        .iter()
        .filter_map(|p| {
            let partner = if p.a.eq_ignore_ascii_case(key) {
                Some(p.b.clone())
            } else if p.b.eq_ignore_ascii_case(key) {
                Some(p.a.clone())
            } else {
                None
            }?;
            Some(ChampionDetailCombo {
                partner_key: partner,
                name: p.name.clone(),
                combo_text: p.tr.clone(),
                combo_type: p.combo_type.clone(),
                strength: p.strength,
            })
        })
        .collect();
    combos.sort_by(|x, y| {
        y.strength
            .partial_cmp(&x.strength)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let detail = ChampionDetail {
        champion_id,
        champion_key: key.clone(),
        archetype: a.archetype.clone(),
        power_early: a.power_curve.early,
        power_mid: a.power_curve.mid,
        power_late: a.power_curve.late,
        win_condition: a.win_condition.clone(),
        damage_ad: a.damage_profile.ad,
        damage_ap: a.damage_profile.ap,
        has_hard_cc: a.cc.has_hard_cc,
        mobility: a.mobility.clone(),
        blind_safety: a.blind_safety,
        execution_difficulty: a.execution_difficulty,
        utility_tags: a.utility_tags.clone(),
        combos,
    };
    serde_json::to_string(&detail).map_err(|e| format!("champion detail serialize failed: {e}"))
}

/// Rich lane-matchup read (`get_lane_matchup` parity).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LaneMatchup {
    pub opponent_key: String,
    pub opponent_name: String,
    pub phase_advantage: [f32; 3],
    pub tips: Vec<String>,
    /// True when the opponent was inferred (Blind/Normal — no LCU positions) by
    /// archetype fit rather than read directly. UI labels it "Tahmini rakip".
    pub inferred: bool,
}

/// Per-phase lane matchup vs the visible (or inferred) lane opponent. JSON
/// `null` when there's no lane (ARAM), no pick, no opponent, or missing KB data.
pub fn lane_matchup_from_json(input_json: &str) -> Result<String, String> {
    let input: TeamContextInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid lane matchup input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let session = &input.session;
    let null = || Ok("null".to_string());

    let my_pos = session.local_player.assigned_position.to_lowercase();
    if my_pos.is_empty() {
        return null(); // ARAM / no lane
    }
    let my_id = if session.local_player.champion_id > 0 {
        session.local_player.champion_id
    } else {
        session.local_player.intent_champion_id
    };
    if my_id == 0 {
        return null();
    }

    let Some((opp_id, inferred)) =
        resolve_lane_opponent_raw(&session.their_team, &input.role_map, &my_pos)
    else {
        return null();
    };

    let Some(my_key) = input
        .all_champions
        .iter()
        .find(|c| c.champion_id == my_id as i64)
        .map(|c| c.key.clone())
    else {
        return null();
    };
    let Some(opp_rec) = input
        .all_champions
        .iter()
        .find(|c| c.champion_id == opp_id as i64)
    else {
        return null();
    };
    let (Some(cand), Some(opp_arch)) = (kb.get_archetype(&my_key), kb.get_archetype(&opp_rec.key))
    else {
        return null();
    };

    let adv = |a: f32, b: f32| -> f32 {
        let s = a + b;
        if s < 0.01 {
            0.5
        } else {
            (a / s).clamp(0.0, 1.0)
        }
    };
    let out = LaneMatchup {
        opponent_key: opp_rec.key.clone(),
        opponent_name: opp_rec.name.clone(),
        phase_advantage: [
            adv(cand.power_curve.early, opp_arch.power_curve.early),
            adv(cand.power_curve.mid, opp_arch.power_curve.mid),
            adv(cand.power_curve.late, opp_arch.power_curve.late),
        ],
        tips: build_matchup_tips(cand, opp_arch, &my_pos),
        inferred,
    };
    serde_json::to_string(&out).map_err(|e| format!("lane matchup serialize failed: {e}"))
}

/// Defensive counter-itemization advice vs the enemy comp (`get_counter_items`).
pub fn counter_items_from_json(input_json: &str) -> Result<String, String> {
    let input: TeamContextInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid counter items input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let session = &input.session;

    let enemy_ids: Vec<u32> = session
        .their_team
        .iter()
        .filter(|s| s.champion_id > 0)
        .map(|s| s.champion_id)
        .collect();
    let enemy_comp = TeamComposition::from_champion_ids(&enemy_ids, &input.role_map);

    let enemy_arch = archetypes_for(&enemy_ids, &input.all_champions, &kb);
    let enemy_sustain = enemy_arch
        .iter()
        .any(|a| a.utility_tags.iter().any(|t| t == "sustain"));
    let enemy_hard_cc: u32 = enemy_arch.iter().map(|a| a.cc.hard_cc_count as u32).sum();

    let my_champ = if session.local_player.champion_id > 0 {
        session.local_player.champion_id
    } else {
        session.local_player.intent_champion_id
    };
    let candidate_squishy_carry = input
        .all_champions
        .iter()
        .find(|c| c.champion_id == my_champ as i64)
        .and_then(|c| kb.get_archetype(&c.key))
        .is_some_and(|a| {
            matches!(
                a.archetype.as_str(),
                "marksman" | "artillery" | "burst_mage" | "control_mage" | "assassin"
            )
        });

    let hints = counter_item_advice(
        &enemy_comp,
        enemy_sustain,
        enemy_hard_cc,
        &input.items,
        candidate_squishy_carry,
    );
    serde_json::to_string(&hints).map_err(|e| format!("counter items serialize failed: {e}"))
}

// ── Pool / feedback insights (P1.3b-6) ────────────────────────────────────────
// Ports of the remaining pure command-layer logic (champ_select.rs:1283
// get_pool_suggestions, pool_coach.rs get_champion_pool_plan, data_quality.rs
// feedback read commands). The host ships DB rows; every decision runs here.

/// Input for `pool_suggestions_json` / `champion_pool_plan_json`. `meta_rates`
/// must already be blended + role-share filtered (use `blended_meta_rates_json`).
#[derive(Debug, Deserialize)]
pub struct PoolInsightsInput {
    /// LCU position: "top" | "jungle" | "middle" | "bottom" | "utility".
    pub role: String,
    #[serde(default)]
    pub mastery: Vec<MasteryRow>,
    /// Match-derived per-champion stats (only `champion_pool_plan` consumes it).
    #[serde(default)]
    pub stats: Vec<ChampionStats>,
    pub all_champions: Vec<ChampionRecord>,
    #[serde(default)]
    pub meta_rates: Vec<MetaRateEntry>,
}

/// Champions to learn for the role (`get_pool_suggestions` parity), excluding
/// the player's owned (mastery) champions.
pub fn pool_suggestions_from_json(input_json: &str) -> Result<String, String> {
    let input: PoolInsightsInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid pool suggestions input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let meta_rates = meta_rates_map(input.meta_rates);

    let owned: Vec<(u32, String)> = input
        .mastery
        .iter()
        .filter_map(|m| {
            input
                .all_champions
                .iter()
                .find(|c| c.champion_id == m.champion_id)
                .map(|c| (m.champion_id as u32, c.key.clone()))
        })
        .collect();

    let candidates: Vec<(u32, String, &ChampionArchetype)> = kb
        .archetypes
        .iter()
        .map(|(key, a)| (a.champion_id, key.clone(), a))
        .collect();

    let suggestions = suggest_pool(
        &input.role,
        &owned,
        &candidates,
        &kb.combos,
        Some(&meta_rates),
    );
    serde_json::to_string(&suggestions)
        .map_err(|e| format!("pool suggestions serialize failed: {e}"))
}

/// Below this sample, meta is unknown → neutral strength (pool_coach.rs parity).
const MIN_META_SAMPLE: u32 = 50;

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn meta_strength_for(meta: &HashMap<(u32, String), MetaRate>, id: u32, role: &str) -> f32 {
    match meta.get(&(id, role.to_string())) {
        Some(r) if r.sample_size >= MIN_META_SAMPLE => {
            let wr = crate::scoring::shrunk_meta_wr(r.win_rate, r.sample_size);
            ((wr - 0.48) / 0.07).clamp(0.0, 1.0)
        }
        _ => 0.5,
    }
}

fn personal_comfort(mastery_points: i64, mastery_level: i64, games: u32) -> f32 {
    if mastery_points <= 0 && mastery_level <= 0 && games == 0 {
        return 0.0;
    }
    let points = (mastery_points.max(0) as f32 / 100_000.0).min(0.65);
    let level = (mastery_level.max(0) as f32 / 7.0).min(1.0) * 0.2;
    let sample = (games as f32 / 50.0).min(1.0) * 0.15;
    clamp01(points + level + sample)
}

fn has_any_tag(arch: &ChampionArchetype, tags: &[&str]) -> bool {
    arch.utility_tags.iter().any(|tag| {
        let tag = tag.to_lowercase();
        tags.iter().any(|needle| tag.contains(needle))
    })
}

fn pool_has_engage(arch: &ChampionArchetype) -> bool {
    let engage_role = arch.engage_role.to_lowercase();
    engage_role != "none" && !engage_role.is_empty()
        || has_any_tag(arch, &["engage", "initiate", "pick"])
        || matches!(
            arch.archetype.as_str(),
            "vanguard" | "diver" | "catcher" | "assassin"
        )
}

fn pool_has_peel(arch: &ChampionArchetype) -> bool {
    let peel = arch.peel_capability.to_lowercase();
    peel != "none" && peel != "low" && !peel.is_empty()
        || has_any_tag(arch, &["peel", "protect", "shield", "disengage"])
        || matches!(arch.archetype.as_str(), "warden" | "enchanter" | "catcher")
}

fn to_pool_champion(
    champion_id: u32,
    champion_key: &str,
    arch: &ChampionArchetype,
    comfort: f32,
    games: u32,
    meta_strength: f32,
) -> PoolChampion {
    PoolChampion {
        champion_id,
        champion_key: champion_key.to_string(),
        archetype: arch.archetype.clone(),
        blind_safety: clamp01(arch.blind_safety),
        execution_difficulty: arch.execution_difficulty.clamp(1, 5),
        power_late: clamp01(arch.power_curve.late),
        engage: pool_has_engage(arch),
        peel: pool_has_peel(arch),
        comfort: clamp01(comfort),
        games,
        meta_strength,
    }
}

/// Champion-pool development plan (`get_champion_pool_plan` parity). The pool is
/// the champions the player MEANINGFULLY plays in this role (mastery level ≥ 4 or
/// ≥ 12k points or ≥ 15 games; role fit via meta share / curated flex / archetype;
/// top-12 by comfort); learn candidates are role-fitting champions NOT owned.
pub fn champion_pool_plan_from_json(input_json: &str) -> Result<String, String> {
    let input: PoolInsightsInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid pool plan input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let role = input.role;
    let meta_rates = meta_rates_map(input.meta_rates);

    let mut key_by_id: HashMap<u32, String> = input
        .all_champions
        .iter()
        .filter_map(|c| u32::try_from(c.champion_id).ok().map(|id| (id, c.key.clone())))
        .collect();
    for (key, arch) in &kb.archetypes {
        key_by_id.entry(arch.champion_id).or_insert_with(|| key.clone());
    }

    let mastery_by_id: HashMap<u32, (i64, i64)> = input
        .mastery
        .iter()
        .filter_map(|m| u32::try_from(m.champion_id).ok().map(|id| (id, (m.level, m.points))))
        .collect();
    let games_by_id: HashMap<u32, u32> = input
        .stats
        .iter()
        .filter_map(|s| u32::try_from(s.champion_id).ok().map(|id| (id, s.games)))
        .collect();

    let personal_ids: std::collections::HashSet<u32> = mastery_by_id
        .keys()
        .copied()
        .chain(games_by_id.keys().copied())
        .collect();

    let role_fits = |id: u32, archetype: &str| -> bool {
        let meta_present = meta_rates
            .get(&(id, role.clone()))
            .is_some_and(|r| r.sample_size >= MIN_META_SAMPLE);
        meta_present
            || flex_positions(id).contains(&role.as_str())
            || archetype_position_fit(archetype, &role)
    };

    const POOL_MAX: usize = 12;
    let mut pool: Vec<PoolChampion> = personal_ids
        .iter()
        .filter_map(|id| {
            let key = key_by_id.get(id)?;
            let arch = kb.get_archetype(key)?;
            if !role_fits(*id, &arch.archetype) {
                return None; // a champ you play in OTHER roles isn't your pool here
            }
            let (level, points) = mastery_by_id.get(id).copied().unwrap_or((0, 0));
            let games = games_by_id.get(id).copied().unwrap_or(0);
            if level < 4 && points < 12_000 && games < 15 {
                return None; // touched once ≠ part of your pool
            }
            Some(to_pool_champion(
                *id,
                key,
                arch,
                personal_comfort(points, level, games),
                games,
                meta_strength_for(&meta_rates, *id, &role),
            ))
        })
        .collect();
    pool.sort_by(|a, b| {
        b.comfort
            .partial_cmp(&a.comfort)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.champion_id.cmp(&b.champion_id))
    });
    pool.truncate(POOL_MAX);

    let mut candidates: Vec<PoolChampion> = kb
        .archetypes
        .iter()
        .filter(|(_, arch)| role_fits(arch.champion_id, &arch.archetype))
        .filter(|(_, arch)| !personal_ids.contains(&arch.champion_id))
        .map(|(key, arch)| {
            let (level, points) = mastery_by_id
                .get(&arch.champion_id)
                .copied()
                .unwrap_or((0, 0));
            let games = games_by_id.get(&arch.champion_id).copied().unwrap_or(0);
            to_pool_champion(
                arch.champion_id,
                key,
                arch,
                personal_comfort(points, level, games),
                games,
                meta_strength_for(&meta_rates, arch.champion_id, &role),
            )
        })
        .collect();
    candidates.sort_by_key(|champ| champ.champion_id);

    let plan = analyze_pool(&PoolCoachInput {
        role,
        pool,
        candidates,
    });
    serde_json::to_string(&plan).map_err(|e| format!("pool plan serialize failed: {e}"))
}

/// Input for `game_review_json` (C1+C2 koç döngüsü): incelenen maç + host'un
/// AYNI rol & queue-grubuyla filtrelediği geçmiş + (varsa) açık hedef.
#[derive(Debug, Deserialize)]
pub struct GameReviewInput {
    #[serde(rename = "match")]
    pub reviewed: crate::postgame::MatchRow,
    #[serde(default)]
    pub history: Vec<crate::postgame::MatchRow>,
    #[serde(default)]
    pub prev_goal: Option<crate::game_review::FocusGoal>,
}

/// Trend raporu (C4): host'un AYNI rol + queue-grubuyla filtrelediği maçlar →
/// sparkline noktaları + yarı-medyan yön hükümleri. Çıktı: `TrendReport`.
pub fn trend_report_from_json(input_json: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct TrendInput {
        #[serde(default)]
        matches: Vec<crate::postgame::MatchRow>,
    }
    let input: TrendInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid trend input: {e}"))?;
    let report = crate::game_review::build_trend_report(&input.matches);
    serde_json::to_string(&report).map_err(|e| format!("trend serialize failed: {e}"))
}

/// Maç sonu karnesi (`build_game_review` sarmalayıcısı). Çıktı: `GameReview`.
pub fn game_review_from_json(input_json: &str) -> Result<String, String> {
    let input: GameReviewInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid game review input: {e}"))?;
    let review = crate::game_review::build_game_review(
        &input.reviewed,
        &input.history,
        input.prev_goal.as_ref(),
    );
    serde_json::to_string(&review).map_err(|e| format!("game review serialize failed: {e}"))
}

/// One raw feedback row with its sync flag (observability input).
#[derive(Debug, Deserialize)]
pub struct FeedbackObservabilityRow {
    pub champion_id: u32,
    pub verdict: String,
    /// false = cloud sync bekliyor (`synced_at IS NULL`).
    #[serde(default)]
    pub synced: bool,
}

/// `get_feedback_observability` output shape (host report twin).
#[derive(Debug, Serialize)]
pub struct FeedbackObservabilityOut {
    pub counters: FeedbackObservability,
    pub status: FeedbackPersonalizationStatus,
}

/// Feedback observability summary (`get_feedback_observability` parity).
pub fn feedback_observability_from_json(input_json: &str) -> Result<String, String> {
    let rows: Vec<FeedbackObservabilityRow> = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid feedback observability input: {e}"))?;
    let pending_sync = rows.iter().filter(|r| !r.synced).count() as u32;
    let inputs: Vec<FeedbackInput> = rows
        .into_iter()
        .map(|r| FeedbackInput {
            champion_id: r.champion_id,
            verdict: r.verdict,
        })
        .collect();
    let counters = summarize_observability(&inputs, pending_sync);
    let status = personalization_status(&counters);
    let out = FeedbackObservabilityOut { counters, status };
    serde_json::to_string(&out)
        .map_err(|e| format!("feedback observability serialize failed: {e}"))
}

#[derive(Debug, Deserialize)]
pub struct FeedbackAnalyticsInput {
    pub events: Vec<FeedbackEvent>,
    /// Host saati (unix saniye) — core saat OKUMAZ (WASM determinizmi).
    pub now_secs: i64,
    #[serde(default)]
    pub window_days: Option<u32>,
}

/// Feedback analytics (`get_feedback_analytics` parity; window default 7 gün).
pub fn feedback_analytics_from_json(input_json: &str) -> Result<String, String> {
    let input: FeedbackAnalyticsInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid feedback analytics input: {e}"))?;
    let analytics = analyze_feedback(
        &input.events,
        input.now_secs,
        input.window_days.unwrap_or(7),
    );
    serde_json::to_string(&analytics)
        .map_err(|e| format!("feedback analytics serialize failed: {e}"))
}

// ── Draft simulation + in-game cluster (P1.3b-7) ──────────────────────────────
// Ports of the LAST pure command-layer logic: commands/draft_simulator.rs
// (helpers + get_draft_simulation / get_draft_fork) and commands/overlay.rs's
// get_ingame_plan / get_macro_state decision paths (live_client parsers live in
// `crate::live_client`). The src-tauri copies are legacy; the Live Client
// network fetch (https://127.0.0.1:2999) stays in the host.

/// `get_draft_simulation` input. `session` accepts BOTH a raw LCU session payload
/// (has `actions`) and an already-serialized `ChampSelectState` — the Tauri
/// command's `parse_session_arg` parity.
#[derive(Debug, Deserialize)]
pub struct DraftSimulationInput {
    pub session: serde_json::Value,
    pub all_champions: Vec<ChampionRecord>,
    #[serde(default)]
    pub candidate_ids: Vec<u32>,
}

/// `get_draft_fork` input (same session contract as [`DraftSimulationInput`]).
#[derive(Debug, Deserialize)]
pub struct DraftForkInput {
    pub session: serde_json::Value,
    pub all_champions: Vec<ChampionRecord>,
    pub option_a_id: u32,
    pub option_b_id: u32,
}

/// `parse_session_arg` parity: raw LCU payload → parser; anything else must
/// deserialize as a `ChampSelectState`.
fn session_from_value(value: serde_json::Value) -> Result<ChampSelectState, String> {
    if value.get("actions").is_some() {
        crate::session_parse::parse_session(&value)
            .ok_or_else(|| "Geçersiz session JSON".to_string())
    } else {
        serde_json::from_value(value).map_err(|_| "Geçersiz session JSON".to_string())
    }
}

/// champion_id↔key resolution maps from the host's champion table
/// (`sim_resolution_maps` parity — there it came from `champion_repo::list_all`).
fn sim_resolution_maps(
    all_champions: &[ChampionRecord],
) -> (HashMap<u32, String>, HashMap<String, u32>) {
    let id_to_key: HashMap<u32, String> = all_champions
        .iter()
        .filter_map(|champ| {
            u32::try_from(champ.champion_id)
                .ok()
                .map(|id| (id, champ.key.clone()))
        })
        .collect();
    let key_to_id: HashMap<String, u32> = id_to_key
        .iter()
        .map(|(id, key)| (key.to_lowercase(), *id))
        .collect();
    (id_to_key, key_to_id)
}

fn sim_damage_type(profile: &DamageProfile) -> DamageType {
    if profile.true_damage >= 0.45
        && profile.true_damage >= profile.ad
        && profile.true_damage >= profile.ap
    {
        DamageType::True
    } else if profile.ad >= 0.35 && profile.ap >= 0.35 {
        DamageType::Mixed
    } else if profile.ap > profile.ad {
        DamageType::Ap
    } else {
        DamageType::Ad
    }
}

fn combo_partner_ids(
    key: &str,
    key_to_id: &HashMap<String, u32>,
    kb: &DraftKnowledgeBase,
) -> Vec<u32> {
    let mut ids: Vec<u32> = kb
        .combos
        .all_pairs()
        .iter()
        .filter_map(|pair| {
            let partner = if pair.a.eq_ignore_ascii_case(key) {
                Some(pair.b.as_str())
            } else if pair.b.eq_ignore_ascii_case(key) {
                Some(pair.a.as_str())
            } else {
                None
            }?;
            key_to_id.get(&partner.to_lowercase()).copied()
        })
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn to_sim_champion(
    champion_id: u32,
    id_to_key: &HashMap<u32, String>,
    key_to_id: &HashMap<String, u32>,
    kb: &DraftKnowledgeBase,
) -> Option<SimChampion> {
    let key = id_to_key.get(&champion_id)?;
    let arch: &ChampionArchetype = kb.get_archetype(key)?;
    Some(SimChampion {
        champion_id,
        champion_key: key.clone(),
        archetype: arch.archetype.clone(),
        damage: sim_damage_type(&arch.damage_profile),
        combo_partner_ids: combo_partner_ids(key, key_to_id, kb),
    })
}

fn slot_pick(slot: &TeamSlot, include_intent: bool) -> Option<u32> {
    if slot.champion_id > 0 {
        Some(slot.champion_id)
    } else if include_intent && slot.intent_champion_id > 0 {
        Some(slot.intent_champion_id)
    } else {
        None
    }
}

fn build_sim_state(
    session: &ChampSelectState,
    id_to_key: &HashMap<u32, String>,
    key_to_id: &HashMap<String, u32>,
    kb: &DraftKnowledgeBase,
) -> DraftSimState {
    let my_team = session
        .my_team
        .iter()
        .filter_map(|slot| {
            let is_local_unlocked = slot.cell_id == session.my_cell_id && !slot.is_locked;
            let champion_id = slot_pick(slot, !is_local_unlocked)?;
            to_sim_champion(champion_id, id_to_key, key_to_id, kb)
        })
        .collect();

    let enemy_team = session
        .their_team
        .iter()
        .filter_map(|slot| {
            let champion_id = slot_pick(slot, true)?;
            to_sim_champion(champion_id, id_to_key, key_to_id, kb)
        })
        .collect();

    DraftSimState {
        my_team,
        enemy_team,
        blind: session.queue_id == 430 || session.local_player.assigned_position.is_empty(),
        first_pick: session.pick_order <= 1,
    }
}

fn unavailable_champion_ids(session: &ChampSelectState) -> std::collections::HashSet<u32> {
    session
        .my_team
        .iter()
        .filter_map(|slot| {
            let is_local_unlocked = slot.cell_id == session.my_cell_id && !slot.is_locked;
            slot_pick(slot, !is_local_unlocked)
        })
        .chain(
            session
                .their_team
                .iter()
                .filter_map(|slot| slot_pick(slot, true)),
        )
        .chain(session.my_bans.iter().copied())
        .chain(session.their_bans.iter().copied())
        .filter(|id| *id > 0)
        .collect()
}

fn candidate_move(
    champion_id: u32,
    session: &ChampSelectState,
    id_to_key: &HashMap<u32, String>,
    key_to_id: &HashMap<String, u32>,
    kb: &DraftKnowledgeBase,
) -> Option<DraftSimMove> {
    to_sim_champion(champion_id, id_to_key, key_to_id, kb).map(|champion| DraftSimMove {
        champion,
        position: Some(session.local_player.assigned_position.clone())
            .filter(|pos| !pos.is_empty()),
    })
}

/// `get_draft_simulation` parity: simulate each candidate pick onto the current
/// draft. Duplicates, id 0 and unavailable (picked/banned) candidates are
/// silently dropped; empty candidates → `[]`.
pub fn draft_simulation_from_json(input_json: &str) -> Result<String, String> {
    let input: DraftSimulationInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid draft simulation input: {e}"))?;
    let session = session_from_value(input.session)?;
    if input.candidate_ids.is_empty() {
        return Ok("[]".to_string());
    }
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;

    let (id_to_key, key_to_id) = sim_resolution_maps(&input.all_champions);
    let sim_state = build_sim_state(&session, &id_to_key, &key_to_id, &kb);
    let unavailable = unavailable_champion_ids(&session);

    let mut seen = std::collections::HashSet::new();
    let candidate_moves: Vec<DraftSimMove> = input
        .candidate_ids
        .into_iter()
        .filter(|id| *id > 0 && !unavailable.contains(id) && seen.insert(*id))
        .filter_map(|id| candidate_move(id, &session, &id_to_key, &key_to_id, &kb))
        .collect();

    let results = crate::draft_simulator::simulate(&DraftSimInput {
        state: sim_state,
        candidate_moves,
    });
    serde_json::to_string(&results).map_err(|e| format!("draft simulation serialize failed: {e}"))
}

/// `get_draft_fork` parity: decisive A-vs-B comparison of two pickable options.
/// JSON `null` when an option is 0 / duplicate / unavailable / not in the KB.
pub fn draft_fork_from_json(input_json: &str) -> Result<String, String> {
    let input: DraftForkInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid draft fork input: {e}"))?;
    if input.option_a_id == 0 || input.option_b_id == 0 || input.option_a_id == input.option_b_id {
        return Ok("null".to_string());
    }

    let session = session_from_value(input.session)?;
    let unavailable = unavailable_champion_ids(&session);
    if unavailable.contains(&input.option_a_id) || unavailable.contains(&input.option_b_id) {
        return Ok("null".to_string());
    }
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;

    let (id_to_key, key_to_id) = sim_resolution_maps(&input.all_champions);
    let sim_state = build_sim_state(&session, &id_to_key, &key_to_id, &kb);
    let Some(option_a) = candidate_move(input.option_a_id, &session, &id_to_key, &key_to_id, &kb)
    else {
        return Ok("null".to_string());
    };
    let Some(option_b) = candidate_move(input.option_b_id, &session, &id_to_key, &key_to_id, &kb)
    else {
        return Ok("null".to_string());
    };

    let fork = crate::draft_fork::compare_fork(&sim_state, &option_a, &option_b);
    serde_json::to_string(&fork).map_err(|e| format!("draft fork serialize failed: {e}"))
}

/// `get_ingame_plan` input: the raw Live Client `allgamedata` payload (fetched by
/// the host) + the host's champion table.
#[derive(Debug, Deserialize)]
pub struct IngamePlanInput {
    pub allgamedata: serde_json::Value,
    pub all_champions: Vec<ChampionRecord>,
}

/// In-game plan from the raw `allgamedata` payload. JSON `null` when there is no
/// active player in the payload or the champion isn't in the KB (quiet, expected
/// — the "no live game" case never reaches core, the host returns null itself).
pub fn ingame_plan_from_json(input_json: &str) -> Result<String, String> {
    let input: IngamePlanInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid ingame plan input: {e}"))?;
    let kb = DraftKnowledgeBase::load().map_err(|e| format!("KB load failed: {e}"))?;
    let plan = crate::live_client::compute_ingame_plan(&input.allgamedata, &input.all_champions, &kb);
    serde_json::to_string(&plan).map_err(|e| format!("ingame plan serialize failed: {e}"))
}

/// `get_macro_state` core half: raw `allgamedata` → objective timers / macro
/// state. The host wraps it as `{live, state}` (`OverlayMacroState` parity) and
/// handles the offline case without calling core.
pub fn macro_state_from_allgamedata_json(input_json: &str) -> Result<String, String> {
    let raw: serde_json::Value = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid allgamedata input: {e}"))?;
    let state =
        crate::macro_timers::compute_macro_state(&crate::live_client::parse_macro_input(&raw));
    serde_json::to_string(&state).map_err(|e| format!("macro state serialize failed: {e}"))
}

// ── Feedback flush policy (P1.3b-8) ───────────────────────────────────────────
// The pure half of commands/feedback_flush.rs (`sync_recommendation_feedback`):
// which queued rows to send (backoff + privacy gate) and how a send result maps
// back onto the row's sync state. The POST loop + SQL stay in the host. The
// idempotency key MUST stay byte-identical across hosts — the backend dedupes on
// it (unique index), so it lives only here.

/// Minimum session-hash length to accept (a real hash, not a raw id) —
/// feedback_flush.rs `MIN_HASH_LEN` parity.
const FLUSH_MIN_HASH_LEN: usize = 16;

/// One queued `recommendation_feedback` row's flush-relevant fields.
#[derive(Debug, Deserialize)]
pub struct FlushRowInput {
    pub id: i64,
    pub champion_id: i64,
    pub feedback: String,
    #[serde(default)]
    pub session_hash: Option<String>,
    pub created_at: i64,
    #[serde(default)]
    pub retry_count: i64,
    #[serde(default)]
    pub next_retry_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackFlushPlanInput {
    pub rows: Vec<FlushRowInput>,
    /// Host saati (unix saniye) — core saat OKUMAZ (WASM determinizmi).
    pub now_secs: i64,
}

/// Per-row flush decision. `action`: "send" (POST it, key attached) |
/// "wait" (backoff not elapsed) | "skip_no_hash" (privacy gate — never sent raw).
#[derive(Debug, Serialize)]
pub struct FlushRowPlan {
    pub id: i64,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

fn flush_state_of(row: &FlushRowInput) -> crate::feedback_sync::FlushState {
    crate::feedback_sync::FlushState {
        synced_at: None,
        retry_count: row.retry_count.max(0) as u32,
        last_error: None,
        next_retry_at: row.next_retry_at,
    }
}

/// Decide, for every unsynced queue row, whether to send / wait / skip it.
pub fn feedback_flush_plan_from_json(input_json: &str) -> Result<String, String> {
    let input: FeedbackFlushPlanInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid feedback flush plan input: {e}"))?;

    let plans: Vec<FlushRowPlan> = input
        .rows
        .iter()
        .map(|row| {
            if !crate::feedback_sync::is_due(&flush_state_of(row), input.now_secs) {
                return FlushRowPlan {
                    id: row.id,
                    action: "wait".to_string(),
                    user_hash: None,
                    idempotency_key: None,
                };
            }
            match row.session_hash.as_deref().map(str::trim) {
                Some(hash) if hash.len() >= FLUSH_MIN_HASH_LEN => FlushRowPlan {
                    id: row.id,
                    action: "send".to_string(),
                    user_hash: Some(row.session_hash.clone().unwrap_or_default()),
                    idempotency_key: Some(crate::feedback_sync::idempotency_key(
                        row.session_hash.as_deref().unwrap_or_default(),
                        row.champion_id.max(0) as u32,
                        &row.feedback,
                        row.created_at,
                    )),
                },
                _ => FlushRowPlan {
                    id: row.id,
                    action: "skip_no_hash".to_string(),
                    user_hash: None,
                    idempotency_key: None,
                },
            }
        })
        .collect();
    serde_json::to_string(&plans).map_err(|e| format!("flush plan serialize failed: {e}"))
}

#[derive(Debug, Deserialize)]
pub struct FeedbackFlushResolveInput {
    #[serde(default)]
    pub retry_count: i64,
    #[serde(default)]
    pub next_retry_at: Option<i64>,
    /// true = POST succeeded.
    pub ok: bool,
    /// Error text when `ok` is false (e.g. "HTTP 503").
    #[serde(default)]
    pub error: Option<String>,
    pub now_secs: i64,
}

/// V013 sync-state columns after one send attempt (`resolve_after_send` parity:
/// success → synced; failure → retry bookkeeping only, the row is never lost).
#[derive(Debug, Serialize)]
pub struct FlushStateOut {
    pub synced_at: Option<i64>,
    pub retry_count: u32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<i64>,
}

/// Map one POST outcome onto the row's new sync state.
pub fn feedback_flush_resolve_from_json(input_json: &str) -> Result<String, String> {
    let input: FeedbackFlushResolveInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid feedback flush resolve input: {e}"))?;
    let prev = crate::feedback_sync::FlushState {
        synced_at: None,
        retry_count: input.retry_count.max(0) as u32,
        last_error: None,
        next_retry_at: input.next_retry_at,
    };
    let result = if input.ok {
        crate::feedback_sync::SendResult::Ok
    } else {
        crate::feedback_sync::SendResult::Failed(
            input.error.unwrap_or_else(|| "bilinmeyen hata".to_string()),
        )
    };
    let st = crate::feedback_sync::resolve_after_send(&prev, result, input.now_secs);
    let out = FlushStateOut {
        synced_at: st.synced_at,
        retry_count: st.retry_count,
        last_error: st.last_error,
        next_retry_at: st.next_retry_at,
    };
    serde_json::to_string(&out).map_err(|e| format!("flush state serialize failed: {e}"))
}

// ── Data-quality read trio (P1.3b-9a) ─────────────────────────────────────────
// Pure halves of data_quality.rs's read-only commands: get_data_source_registry,
// get_pipeline_quality_report, get_data_trajectory. The host ships raw COUNT(*)s
// + the cached-pack row + fetch-log facts; every derivation (source kinds, risk,
// fallback/staleness, quality evaluation, trajectory fusion) runs here.
// `sync_data_pipeline` (the network orchestrator) stays in the host and is NOT
// part of this surface.

/// Raw local-DB coverage counts (`gather_coverage`'s SQL results).
#[derive(Debug, Deserialize)]
pub struct CoverageCountsInput {
    pub total_champions: u32,
    pub champion_rates_count: u32,
    pub matchup_count: u32,
    pub build_count: u32,
    /// DISTINCT champion_id in `builds`.
    pub build_champions: u32,
    /// DISTINCT champion_id in `champion_rates`.
    pub meta_role_champions: u32,
}

/// The cached `draft_brain_packs` data_pack row, when present.
#[derive(Debug, Deserialize)]
pub struct DataPackCacheInput {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// `gather_coverage`'s pure tail (data_quality.rs:1850-1915 verbatim): which
/// source kinds contribute, whether we run on fallback, and what is stale.
fn coverage_from_counts(
    counts: &CoverageCountsInput,
    data_pack: Option<&DataPackCacheInput>,
    now: i64,
) -> (
    crate::draft_brain_data::LocalCoverage,
    bool,
    Vec<String>,
) {
    use crate::draft_brain_data::{DataSourceEntry, DataSourceKind};

    // Bundled seeds + DDragon are always present; rates imply an aggregator.
    let no_local_data = counts.build_count == 0 && counts.matchup_count == 0;
    let mut sources = vec![
        DataSourceEntry::from_kind(
            DataSourceKind::LocalSeed,
            Some(counts.build_count.max(counts.matchup_count)),
            no_local_data,
        ),
        DataSourceEntry::from_kind(DataSourceKind::ManualSeed, Some(counts.matchup_count), false),
        DataSourceEntry::from_kind(DataSourceKind::Ddragon, None, false),
    ];
    if counts.champion_rates_count > 0 {
        sources.push(DataSourceEntry::from_kind(
            DataSourceKind::Meraki,
            Some(counts.champion_rates_count),
            false,
        ));
    }

    let mut stale_sources = Vec::new();
    let fallback_active = match data_pack {
        Some(pack) if pack.source.as_deref() == Some("cloud") => {
            sources.push(DataSourceEntry::from_kind(
                DataSourceKind::CloudPostgres,
                None,
                false,
            ));
            if pack.expires_at.map(|e| e < now).unwrap_or(true) {
                stale_sources.push("data_pack".to_string());
            }
            false
        }
        Some(pack) => {
            // A local/builder pack is cached — still a fallback.
            if pack.expires_at.map(|e| e < now).unwrap_or(true) {
                stale_sources.push("data_pack".to_string());
            }
            true
        }
        None => {
            // Never synced → runtime uses the in-memory local seed pack.
            stale_sources.push("data_pack".to_string());
            true
        }
    };

    let coverage = crate::draft_brain_data::LocalCoverage {
        total_champions: counts.total_champions,
        champion_rates_count: counts.champion_rates_count,
        matchup_count: counts.matchup_count,
        build_count: counts.build_count,
        build_champions: counts.build_champions,
        meta_role_champions: counts.meta_role_champions,
        sources,
    };
    (coverage, fallback_active, stale_sources)
}

#[derive(Debug, Deserialize)]
pub struct DataSourceRegistryInput {
    pub counts: CoverageCountsInput,
    #[serde(default)]
    pub data_pack: Option<DataPackCacheInput>,
    pub now_secs: i64,
}

/// `get_data_source_registry` parity → `DataSourceRegistryReport`.
pub fn data_source_registry_from_json(input_json: &str) -> Result<String, String> {
    let input: DataSourceRegistryInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid data source registry input: {e}"))?;
    let (coverage, fallback_active, stale) =
        coverage_from_counts(&input.counts, input.data_pack.as_ref(), input.now_secs);
    let report = crate::draft_brain_data::compute_registry_report(
        &coverage,
        stale,
        fallback_active,
        input.now_secs.clamp(0, u32::MAX as i64) as u32,
    );
    serde_json::to_string(&report).map_err(|e| format!("registry serialize failed: {e}"))
}

/// One `(source_label, updated_at)` row from the host's per-table
/// `GROUP BY source` queries ("rates:u.gg", "pack:cloud", …). Risk derives here.
#[derive(Debug, Deserialize)]
pub struct PipelineSourceRowInput {
    pub source: String,
    pub updated_at: i64,
}

/// data_quality.rs `source_risk` verbatim — label substring → risk posture.
fn source_risk(source: &str) -> String {
    let source = source.to_lowercase();
    if source.contains("scraper")
        || source.contains("lolalytics")
        || source.contains("op_gg")
        || source.contains("mobalytics")
    {
        "high"
    } else if source.contains("cloud")
        || source.contains("riot")
        || source.contains("meraki")
        || source.contains("u_gg")
        || source.contains("leaguepedia")
        || source.contains("postgres")
    {
        "medium"
    } else {
        "low"
    }
    .to_string()
}

#[derive(Debug, Deserialize)]
pub struct PipelineQualityReportInput {
    pub counts: CoverageCountsInput,
    #[serde(default)]
    pub data_pack: Option<DataPackCacheInput>,
    pub now_secs: i64,
    /// DDragon current patch; "unknown"/empty falls back to `data_patch`.
    #[serde(default)]
    pub current_patch: String,
    /// Dominant patch across rates/builds/matchups (host's UNION query).
    #[serde(default)]
    pub data_patch: Option<String>,
    #[serde(default)]
    pub source_rows: Vec<PipelineSourceRowInput>,
    #[serde(default)]
    pub pack_exists: bool,
}

/// `get_pipeline_quality_report` parity → `PipelineQualityReport`
/// (build_pipeline_quality_input + pipeline_sources + evaluate_pipeline_quality).
pub fn pipeline_quality_report_from_json(input_json: &str) -> Result<String, String> {
    use crate::data_pipeline_quality::{
        evaluate_pipeline_quality, PipelineQualityInput, PipelineSource,
    };

    let input: PipelineQualityReportInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid pipeline quality input: {e}"))?;
    let (coverage, fallback_active, _stale) =
        coverage_from_counts(&input.counts, input.data_pack.as_ref(), input.now_secs);

    let mut sources: Vec<PipelineSource> = input
        .source_rows
        .into_iter()
        .map(|row| PipelineSource {
            risk_level: source_risk(&row.source),
            source: row.source,
            updated_at: row.updated_at,
        })
        .collect();
    if sources.is_empty() {
        sources.push(PipelineSource {
            source: "local_seed".to_string(),
            updated_at: input.now_secs,
            risk_level: "low".to_string(),
        });
    }

    let current_patch = if input.current_patch == "unknown" || input.current_patch.is_empty() {
        input
            .data_patch
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        input.current_patch
    };

    const TARGET_CHAMPIONS: u32 = 172;
    let quality_input = PipelineQualityInput {
        now: input.now_secs,
        current_patch,
        data_patch: input.data_patch,
        target_champions: coverage.total_champions.max(TARGET_CHAMPIONS),
        champion_rate_count: coverage.meta_role_champions,
        matchup_count: coverage.matchup_count,
        build_champion_count: coverage.build_champions,
        meta_role_count: coverage.meta_role_champions,
        sources,
        fallback_available: fallback_active
            || coverage
                .sources
                .iter()
                .any(|source| source.source == "local_seed" || source.source == "manual_seed"),
        last_good_cache_available: input.pack_exists,
    };
    let report = evaluate_pipeline_quality(&quality_input);
    serde_json::to_string(&report).map_err(|e| format!("pipeline report serialize failed: {e}"))
}

/// Background scheduler's last coverage-ramp measurement (None on a host that
/// has no scheduler yet — the trajectory then stays an honest "unknown").
#[derive(Debug, Deserialize)]
pub struct RampStateInput {
    pub ramp_state: String,
    #[serde(default)]
    pub data_growing: bool,
    #[serde(default)]
    pub measured_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DataTrajectoryInput {
    /// `PipelineQualityReport.status` (call `pipeline_quality_report_json` first).
    pub quality_status: String,
    #[serde(default)]
    pub ramp: Option<RampStateInput>,
    #[serde(default)]
    pub riot_key_present: bool,
    #[serde(default)]
    pub has_summoner: bool,
    /// Last successful "match_v5" fetch-log timestamp (epoch secs), when any.
    #[serde(default)]
    pub last_match_v5_success_at: Option<i64>,
    pub now_secs: i64,
}

/// `DataTrajectoryView` twin (serde-only; the ts-rs export stays in the Tauri
/// host until it dies — single TS writer).
#[derive(Debug, Serialize)]
pub struct DataTrajectoryOut {
    pub trajectory: String,
    pub quality_status: String,
    pub ramp_state: String,
    pub data_growing: bool,
    pub measured_at: Option<u32>,
    pub riot_key_present: bool,
    pub match_v5_enabled: bool,
    pub match_v5_last_success_at: Option<u32>,
    pub match_v5_age_secs: Option<u32>,
}

/// `get_data_trajectory` parity: fuse quality status + ramp motion + honest
/// Match-V5/Riot-key badges into one user-facing view.
pub fn data_trajectory_from_json(input_json: &str) -> Result<String, String> {
    let input: DataTrajectoryInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid data trajectory input: {e}"))?;

    let (ramp_state, data_growing, measured_at, trajectory) = match input.ramp {
        Some(r) => {
            let trajectory =
                crate::coverage_ramp::classify_data_trajectory(&input.quality_status, &r.ramp_state);
            (
                r.ramp_state,
                r.data_growing,
                r.measured_at.map(|t| t.max(0) as u32),
                trajectory,
            )
        }
        None => ("unknown".to_string(), false, None, "unknown".to_string()),
    };

    let out = DataTrajectoryOut {
        trajectory,
        quality_status: input.quality_status,
        ramp_state,
        data_growing,
        measured_at,
        riot_key_present: input.riot_key_present,
        match_v5_enabled: input.riot_key_present && input.has_summoner,
        match_v5_last_success_at: input.last_match_v5_success_at.map(|t| t.max(0) as u32),
        match_v5_age_secs: input
            .last_match_v5_success_at
            .map(|t| (input.now_secs - t).max(0) as u32),
    };
    serde_json::to_string(&out).map_err(|e| format!("trajectory serialize failed: {e}"))
}

// ── Cache promotion (sync_data_pipeline'ın saf karar yarısı) ──────────────────
// data_quality.rs: candidate_quality_from_db + current_cache_quality +
// decide_cache_promotion + cache_local_data_pack'in pack üretimi. Host yalnız
// sayımları + cache'lenmiş pack satırını yollar; karar ve yeni pack burada.

/// data_quality.rs `coverage_score_from_counts` verbatim.
fn coverage_score_from_counts(rates: u32, matchups: u32, build_champions: u32) -> f32 {
    const TARGET_CHAMPIONS: u32 = 172;
    let rate_cov = (rates as f32 / TARGET_CHAMPIONS as f32).min(1.0);
    let matchup_cov = (matchups as f32 / 1_000.0).min(1.0);
    let build_cov = (build_champions as f32 / TARGET_CHAMPIONS as f32).min(1.0);
    ((rate_cov + matchup_cov + build_cov) / 3.0).clamp(0.0, 1.0)
}

/// Cached pack row WITH payload (current_cache_quality input).
#[derive(Debug, Deserialize)]
pub struct CachedPackInput {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub payload_json: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CachePromotionInput {
    pub counts: CoverageCountsInput,
    #[serde(default)]
    pub data_pack: Option<CachedPackInput>,
    /// `COUNT(DISTINCT champion_id) FROM builds WHERE source != 'unknown'` —
    /// current_cache_quality'nin kaynak-filtreli sayımı (counts'takinden farklı).
    #[serde(default)]
    pub build_champions_known: u32,
    #[serde(default)]
    pub patch: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    pub now_secs: i64,
}

#[derive(Debug, Serialize)]
pub struct CachePromotionOut {
    pub promoted: bool,
    pub action: String,
    pub reason: String,
    /// Yeni local pack — yalnız `promoted` iken; host draft_brain_packs'e yazar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack_payload_json: Option<String>,
}

/// `local_builder` adayını mevcut son-iyi cache'le kıyasla; promote ise yeni
/// pack'i üret (sync_data_pipeline'ın cache adımı, I/O'suz).
pub fn cache_promotion_from_json(input_json: &str) -> Result<String, String> {
    use crate::ingestion_contract::{decide_cache_promotion, CandidateQuality};

    let input: CachePromotionInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid cache promotion input: {e}"))?;
    let pack_ref = input.data_pack.as_ref().map(|p| DataPackCacheInput {
        source: p.source.clone(),
        expires_at: p.expires_at,
    });
    let (coverage, _fallback, _stale) =
        coverage_from_counts(&input.counts, pack_ref.as_ref(), input.now_secs);

    // candidate_quality_from_db paritesi.
    let high_risk = coverage.sources.iter().any(|e| e.risk_level == "high");
    let candidate = CandidateQuality {
        source: "local_builder".to_string(),
        risk_level: if high_risk { "high" } else { "medium" }.to_string(),
        coverage_score: coverage_score_from_counts(
            coverage.champion_rates_count,
            coverage.matchup_count,
            coverage.build_champions,
        ),
        sample_size: coverage
            .champion_rates_count
            .saturating_add(coverage.matchup_count)
            .saturating_add(coverage.build_count),
        fresh: true,
    };

    // current_cache_quality paritesi (payload parse edilemezse current yok).
    let current: Option<CandidateQuality> = input.data_pack.as_ref().and_then(|p| {
        let payload = p.payload_json.as_deref()?;
        let pack = crate::draft_brain::DataPack::from_json(payload).ok()?;
        let source = p.source.clone().unwrap_or_else(|| "unknown".to_string());
        Some(CandidateQuality {
            risk_level: source_risk(&source),
            source,
            coverage_score: coverage_score_from_counts(
                pack.quality.champion_rates,
                pack.quality.matchups,
                input.build_champions_known,
            ),
            sample_size: pack
                .quality
                .champion_rates
                .saturating_add(pack.quality.matchups)
                .saturating_add(pack.quality.builds),
            fresh: p.expires_at.map(|e| e >= input.now_secs).unwrap_or(false),
        })
    });

    let decision = decide_cache_promotion(&candidate, current.as_ref());
    let (pack_version, pack_payload_json) = if decision.promoted {
        let mut pack =
            crate::draft_brain_data::build_local_data_pack(&coverage, input.patch, input.region);
        pack.generated_at = Some(input.now_secs.clamp(0, u32::MAX as i64) as u32);
        let payload = serde_json::to_string(&pack)
            .map_err(|e| format!("data pack serialize failed: {e}"))?;
        (Some(pack.version), Some(payload))
    } else {
        (None, None)
    };

    let out = CachePromotionOut {
        promoted: decision.promoted,
        action: decision.action,
        reason: decision.reason,
        pack_version,
        pack_payload_json,
    };
    serde_json::to_string(&out).map_err(|e| format!("cache promotion serialize failed: {e}"))
}

// ── Match-V5 ingestion (P1.3b-10) ─────────────────────────────────────────────
// The pure planners + the raw-detail→canonical-rows transform behind
// sync_match_v5_ingestion. Input DTO'ları JSON-twin'dir (planner struct'larına
// serde EKLENMEDİ — düşük churn); host yalnız SQL satırları + ham Riot
// detayları gönderir. PUUID hash'i (FNV-1a 64) tek kaynak olarak burada üretilir.

fn fnv1a64_hex(input: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[derive(Debug, Deserialize)]
pub struct MatchCandidateInput {
    pub match_id: String,
    pub region: String,
    pub patch: String,
    pub queue_id: u32,
    #[serde(default)]
    pub role_hint: Option<String>,
    pub discovered_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct FetchedMatchRecordInput {
    pub match_id: String,
    pub region: String,
    pub patch: String,
    pub status: String,
    pub fetched_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct FrontierSampleInput {
    pub region: String,
    pub patch: String,
    pub role: String,
    #[serde(default)]
    pub champion_id: Option<u32>,
    pub current_samples: u32,
    pub target_samples: u32,
}

#[derive(Debug, Deserialize)]
pub struct MatchFetchPlanInput {
    pub now_secs: i64,
    #[serde(default)]
    pub champ_select_active: bool,
    pub rate_budget: u32,
    pub batch_limit: u32,
    #[serde(default)]
    pub candidates: Vec<MatchCandidateInput>,
    #[serde(default)]
    pub fetched_records: Vec<FetchedMatchRecordInput>,
    #[serde(default)]
    pub frontiers: Vec<FrontierSampleInput>,
}

/// `plan_match_fetch` + `build_match_fetch_coverage_gaps` paritesi: frontier
/// örnekleri → coverage expansion → gap önceliği → fetch planı.
pub fn match_fetch_plan_from_json(input_json: &str) -> Result<String, String> {
    use crate::coverage_expansion_policy::{
        plan_coverage_expansion, CoverageExpansionInput, FrontierSample,
    };
    use crate::match_fetch_planner::{
        plan_match_fetch, CoverageGap, FetchedMatchRecord, MatchCandidate,
        MatchFetchPlannerInput,
    };

    let input: MatchFetchPlanInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid match fetch plan input: {e}"))?;

    let frontiers: Vec<FrontierSample> = input
        .frontiers
        .into_iter()
        .map(|f| FrontierSample {
            region: f.region,
            patch: f.patch,
            role: f.role,
            champion_id: f.champion_id,
            current_samples: f.current_samples,
            target_samples: f.target_samples,
        })
        .collect();
    let total_samples: u32 = frontiers.iter().map(|f| f.current_samples).sum();
    let max_targets = frontiers.len() as u32;
    let expansion = plan_coverage_expansion(&CoverageExpansionInput {
        champ_select_active: input.champ_select_active,
        frontiers,
        player_sample_counts: if total_samples == 0 {
            Vec::new()
        } else {
            vec![total_samples]
        },
        max_targets,
    });
    let coverage_gaps: Vec<CoverageGap> = expansion
        .targets
        .into_iter()
        .map(|t| CoverageGap {
            region: t.frontier.region,
            patch: t.frontier.patch,
            role: t.frontier.role,
            current_samples: t.frontier.current_samples,
            target_samples: t.frontier.target_samples,
            priority: t.priority,
        })
        .collect();

    let plan = plan_match_fetch(&MatchFetchPlannerInput {
        now: input.now_secs,
        champ_select_active: input.champ_select_active,
        rate_budget: input.rate_budget,
        batch_limit: input.batch_limit,
        candidates: input
            .candidates
            .into_iter()
            .map(|c| MatchCandidate {
                match_id: c.match_id,
                region: c.region,
                patch: c.patch,
                queue_id: c.queue_id,
                role_hint: c.role_hint,
                discovered_at: c.discovered_at,
            })
            .collect(),
        fetched_records: input
            .fetched_records
            .into_iter()
            .map(|r| FetchedMatchRecord {
                match_id: r.match_id,
                region: r.region,
                patch: r.patch,
                status: r.status,
                fetched_at: r.fetched_at,
            })
            .collect(),
        coverage_gaps,
    });
    serde_json::to_string(&plan).map_err(|e| format!("match fetch plan serialize failed: {e}"))
}

#[derive(Debug, Deserialize)]
pub struct DiscoverySeedInput {
    pub puuid_hash: String,
    pub region: String,
    pub source: String,
    pub seen_at: i64,
    pub contribution_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct CrawledPlayerInput {
    pub puuid_hash: String,
    pub region: String,
    pub last_crawled_at: i64,
    pub crawl_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct DiscoveredCandidateInput {
    pub match_id: String,
    pub region: String,
    pub source_puuid_hash: String,
    pub discovered_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct KnownMatchInput {
    pub match_id: String,
    pub region: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct MatchDiscoveryPlanInput {
    pub now_secs: i64,
    #[serde(default)]
    pub champ_select_active: bool,
    pub crawl_budget: u32,
    pub max_breadth: u32,
    pub per_player_match_cap: u32,
    #[serde(default)]
    pub seeds: Vec<DiscoverySeedInput>,
    #[serde(default)]
    pub crawled_players: Vec<CrawledPlayerInput>,
    #[serde(default)]
    pub candidate_matches: Vec<DiscoveredCandidateInput>,
    #[serde(default)]
    pub known_matches: Vec<KnownMatchInput>,
}

/// `plan_match_discovery` paritesi (crawl seçimi + yeni maç intake'i).
pub fn match_discovery_plan_from_json(input_json: &str) -> Result<String, String> {
    use crate::match_discovery_planner::{
        plan_match_discovery, CrawledPlayerRecord, DiscoveredMatchCandidate, DiscoverySeed,
        KnownMatchRecord, MatchDiscoveryInput,
    };

    let input: MatchDiscoveryPlanInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid match discovery plan input: {e}"))?;
    let plan = plan_match_discovery(&MatchDiscoveryInput {
        now: input.now_secs,
        champ_select_active: input.champ_select_active,
        crawl_budget: input.crawl_budget,
        max_breadth: input.max_breadth,
        per_player_match_cap: input.per_player_match_cap,
        seeds: input
            .seeds
            .into_iter()
            .map(|s| DiscoverySeed {
                puuid_hash: s.puuid_hash,
                region: s.region,
                source: s.source,
                seen_at: s.seen_at,
                contribution_count: s.contribution_count,
            })
            .collect(),
        crawled_players: input
            .crawled_players
            .into_iter()
            .map(|p| CrawledPlayerRecord {
                puuid_hash: p.puuid_hash,
                region: p.region,
                last_crawled_at: p.last_crawled_at,
                crawl_count: p.crawl_count,
            })
            .collect(),
        candidate_matches: input
            .candidate_matches
            .into_iter()
            .map(|c| DiscoveredMatchCandidate {
                match_id: c.match_id,
                region: c.region,
                source_puuid_hash: c.source_puuid_hash,
                discovered_at: c.discovered_at,
            })
            .collect(),
        known_matches: input
            .known_matches
            .into_iter()
            .map(|k| KnownMatchRecord {
                match_id: k.match_id,
                region: k.region,
                status: k.status,
            })
            .collect(),
    });
    serde_json::to_string(&plan)
        .map_err(|e| format!("match discovery plan serialize failed: {e}"))
}

/// Raw Match-V5 detayları (id'leriyle hizalı) + bölge.
#[derive(Debug, Deserialize)]
pub struct MatchV5IngestInput {
    pub details: Vec<serde_json::Value>,
    /// `details[i]` parse edilemezse fallback/failed id'si.
    pub ids: Vec<String>,
    pub region: String,
}

#[derive(Debug, Serialize)]
pub struct ParsedMatchMeta {
    pub match_id: String,
    pub patch: String,
    pub queue_id: u32,
}

#[derive(Debug, Serialize)]
pub struct ParticipantSeedOut {
    /// Ham PUUID — host crawl çağrısı için kullanır, DİSKE YAZMAZ.
    pub puuid: String,
    /// FNV-1a 64 hex — diske yazılan tek kimlik (privacy).
    pub hash: String,
}

#[derive(Debug, Serialize)]
pub struct MatchV5IngestOut {
    pub row_set: crate::ingestion_contract::CanonicalRowSet,
    pub parsed: Vec<ParsedMatchMeta>,
    /// Parse edilemeyen detayların id'leri ("parse_failed").
    pub failed: Vec<String>,
    pub match_count: u32,
    pub participants: Vec<ParticipantSeedOut>,
}

/// Ham detaylar → MatchV5 parse + aggregate + canonical satırlar + katılımcı
/// seed'leri (sync_match_v5_ingestion'ın saf orta katmanı).
pub fn match_v5_ingest_from_json(input_json: &str) -> Result<String, String> {
    let input: MatchV5IngestInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid match v5 ingest input: {e}"))?;

    let mut parsed_matches = Vec::new();
    let mut failed = Vec::new();
    let mut seen_puuids = std::collections::HashSet::new();
    let mut participants = Vec::new();
    for (idx, detail) in input.details.iter().enumerate() {
        let fallback = input.ids.get(idx).map(String::as_str).unwrap_or("");
        // participant_puuids_from_detail paritesi (trim + dedup).
        for p in detail
            .get("info")
            .and_then(|i| i.get("participants"))
            .and_then(|p| p.as_array())
            .into_iter()
            .flatten()
        {
            if let Some(puuid) = p.get("puuid").and_then(|v| v.as_str()) {
                let puuid = puuid.trim();
                if !puuid.is_empty() && seen_puuids.insert(puuid.to_string()) {
                    participants.push(ParticipantSeedOut {
                        puuid: puuid.to_string(),
                        hash: fnv1a64_hex(puuid),
                    });
                }
            }
        }
        match crate::match_v5_mapper::match_v5_from_detail(detail, fallback) {
            Some(m) => parsed_matches.push(m),
            None => failed.push(fallback.to_string()),
        }
    }
    participants.sort_by(|a, b| a.puuid.cmp(&b.puuid));

    let aggregation = crate::match_v5_aggregator::aggregate_matches(&parsed_matches);
    let row_set = crate::ingestion_contract::to_canonical_rows(&aggregation, &input.region);
    let out = MatchV5IngestOut {
        match_count: aggregation.quality.match_count,
        row_set,
        parsed: parsed_matches
            .iter()
            .map(|m| ParsedMatchMeta {
                match_id: m.match_id.clone(),
                patch: m.patch.clone(),
                queue_id: m.queue_id,
            })
            .collect(),
        failed,
        participants,
    };
    serde_json::to_string(&out).map_err(|e| format!("match v5 ingest serialize failed: {e}"))
}

// ── Pipeline scheduler (arka plan tick'inin saf karar yarısı) ─────────────────
// data_quality.rs `build_pipeline_scheduler_status` paritesi: host yalnız SQL
// okur (fetch-log satırları + rate-window timestamp'leri + matchup sayısı) ve
// champ-select/riot bayraklarını yollar; kaynak listesi, TTL'ler, rate bütçesi
// ve refresh/skip kararları burada. `disabled_sources` host'ta henüz taşınmamış
// kaynaklar içindir (ör. Electron'da u_gg/leaguepedia) — dürüst `skip_disabled`.

/// `source_fetch_log` satırı (source, status, finished_at) — read_fetch_logs çıktısı.
#[derive(Debug, Deserialize)]
pub struct SchedulerLogInput {
    pub source: String,
    pub status: String,
    pub at: i64,
}

#[derive(Debug, Deserialize)]
pub struct PipelineRefreshPlanInput {
    pub now_secs: i64,
    #[serde(default)]
    pub champ_select_active: bool,
    /// Riot key var VE aktif summoner sync'li (match_v5 kaynağını açar).
    #[serde(default)]
    pub riot_enabled: bool,
    /// Edge worker base URL'i yapılandırılmış (cloud_edge kaynağını açar).
    #[serde(default)]
    pub edge_enabled: bool,
    /// `COUNT(*) FROM champion_matchups` — match_v5 warmup/stable TTL seçimi.
    #[serde(default)]
    pub matchup_count: u32,
    #[serde(default)]
    pub logs: Vec<SchedulerLogInput>,
    /// Son rate penceresindeki refresh denemeleri (recent_request_timestamps).
    #[serde(default)]
    pub request_timestamps: Vec<i64>,
    /// Bu host'ta devre dışı kaynaklar (henüz taşınmadı / kapalı).
    #[serde(default)]
    pub disabled_sources: Vec<String>,
}

/// `PipelineSchedulerStatus` twin'i (serde-only; ts-rs Tauri host'ta kalır).
#[derive(Debug, Serialize)]
pub struct PipelineRefreshPlanOut {
    pub champ_select_active: bool,
    pub rate_limit: crate::pipeline_scheduler_policy::RateLimitBudget,
    pub fetch_logs: crate::pipeline_scheduler_policy::FetchLogSummary,
    pub plan: crate::pipeline_scheduler_policy::RefreshPlan,
}

/// Arka plan scheduler tick'inin plan adımı. Sabitler data_quality.rs verbatim:
/// rate penceresi 1 saat / 60 istek; ddragon-meraki-leaguepedia TTL 24 saat,
/// u_gg 6 saat; match_v5 25k matchup hedefine kadar 3 dk (warmup), sonra 1 saat.
pub fn pipeline_refresh_plan_from_json(input_json: &str) -> Result<String, String> {
    use crate::pipeline_scheduler_policy::{
        compute_rate_budget, plan_refresh, summarize_fetch_logs, FetchLogEntry, RateLimitInput,
        RefreshPolicyInput, RefreshSourceInput,
    };

    const RATE_WINDOW_SECS: i64 = 60 * 60;
    const RATE_MAX_REQUESTS: u32 = 60;
    const PACK_TTL_SECS: i64 = 24 * 60 * 60;
    const UGG_TTL_SECS: i64 = 6 * 60 * 60;
    const MATCH_V5_TARGET_MATCHUPS: u32 = 25_000;
    const MATCH_V5_WARMUP_TTL_SECS: i64 = 3 * 60;
    const MATCH_V5_STABLE_TTL_SECS: i64 = 60 * 60;
    /// Edge worker (cloudflare /v1/rates) — Match-V5 türevi, u_gg kadansında.
    const EDGE_TTL_SECS: i64 = 6 * 60 * 60;

    let input: PipelineRefreshPlanInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid pipeline refresh plan input: {e}"))?;
    let now = input.now_secs;

    let logs: Vec<FetchLogEntry> = input
        .logs
        .iter()
        .map(|l| FetchLogEntry {
            source: l.source.clone(),
            status: l.status.clone(),
            at: l.at,
        })
        .collect();
    let fetch_logs = summarize_fetch_logs(&logs, now);
    let rate_limit = compute_rate_budget(&RateLimitInput {
        now,
        window_secs: RATE_WINDOW_SECS,
        max_requests: RATE_MAX_REQUESTS,
        request_timestamps: input.request_timestamps.clone(),
    });

    let match_v5_ttl = if input.matchup_count < MATCH_V5_TARGET_MATCHUPS {
        MATCH_V5_WARMUP_TTL_SECS
    } else {
        MATCH_V5_STABLE_TTL_SECS
    };
    let last_success_at = |key: &str| {
        logs.iter()
            .filter(|e| e.source == key && e.status == "success")
            .map(|e| e.at)
            .max()
    };
    let health_of = |key: &str| {
        fetch_logs
            .sources
            .iter()
            .find(|s| s.source == key)
            .map(|s| s.health.clone())
            .unwrap_or_else(|| "insufficient".to_string())
    };
    let disabled = |key: &str| input.disabled_sources.iter().any(|s| s == key);
    let source = |key: &str, enabled: bool, ttl_secs: i64| RefreshSourceInput {
        source: key.to_string(),
        enabled: enabled && !disabled(key),
        last_fetch_at: last_success_at(key),
        ttl_secs,
        health: health_of(key),
        next_allowed_at: None,
    };

    let plan = plan_refresh(&RefreshPolicyInput {
        now,
        champ_select_active: input.champ_select_active,
        remaining_budget: rate_limit.remaining,
        sources: vec![
            source("ddragon", true, PACK_TTL_SECS),
            source("meraki", true, PACK_TTL_SECS),
            source("u_gg", true, UGG_TTL_SECS),
            source("leaguepedia", true, PACK_TTL_SECS),
            source("match_v5", input.riot_enabled, match_v5_ttl),
            source("cloud_edge", input.edge_enabled, EDGE_TTL_SECS),
        ],
    });

    let out = PipelineRefreshPlanOut {
        champ_select_active: input.champ_select_active,
        rate_limit,
        fetch_logs,
        plan,
    };
    serde_json::to_string(&out).map_err(|e| format!("refresh plan serialize failed: {e}"))
}

// ── Coverage ramp (tick'in before→after ölçümü) ───────────────────────────────
// data_quality.rs `record_tick_ramp` / `measure_live_coverage_ramp`'in saf
// çekirdeği: host iki kez ramp_snapshot SQL'i okur, değerlendirme burada.

/// `ramp_snapshot` twin'i (coverage_ramp::RampSnapshot Rust-only — JSON-twin deseni).
#[derive(Debug, Deserialize)]
pub struct RampSnapshotInput {
    pub taken_at: i64,
    #[serde(default)]
    pub champion_rate_rows: u32,
    #[serde(default)]
    pub matchup_rows: u32,
    #[serde(default)]
    pub build_rows: u32,
    #[serde(default)]
    pub discovered_matches: u32,
    #[serde(default)]
    pub fetched_matches: u32,
    #[serde(default)]
    pub processed_matches: u32,
    #[serde(default)]
    pub failed_matches: u32,
    #[serde(default)]
    pub crawled_players: u32,
}

#[derive(Debug, Deserialize)]
pub struct CoverageRampEvalInput {
    pub before: RampSnapshotInput,
    pub after: RampSnapshotInput,
    #[serde(default)]
    pub champ_select_active: bool,
    #[serde(default)]
    pub crawl_budget: u32,
}

fn ramp_snapshot_from_input(s: &RampSnapshotInput) -> crate::coverage_ramp::RampSnapshot {
    crate::coverage_ramp::RampSnapshot {
        taken_at: s.taken_at,
        champion_rate_rows: s.champion_rate_rows,
        matchup_rows: s.matchup_rows,
        build_rows: s.build_rows,
        discovered_matches: s.discovered_matches,
        fetched_matches: s.fetched_matches,
        processed_matches: s.processed_matches,
        failed_matches: s.failed_matches,
        crawled_players: s.crawled_players,
    }
}

/// before→after snapshot çifti → `CoverageRampReport` (ramp_state / deltas /
/// funnel / observations). Çıktının `ramp_state` + `data_growing` alanları
/// `data_trajectory_json`'ın `ramp` girdisine beslenir.
pub fn coverage_ramp_from_json(input_json: &str) -> Result<String, String> {
    let input: CoverageRampEvalInput = serde_json::from_str(input_json)
        .map_err(|e| format!("invalid coverage ramp input: {e}"))?;
    let report =
        crate::coverage_ramp::evaluate_coverage_ramp(&crate::coverage_ramp::CoverageRampInput {
            before: ramp_snapshot_from_input(&input.before),
            after: ramp_snapshot_from_input(&input.after),
            champ_select_active: input.champ_select_active,
            crawl_budget: input.crawl_budget,
        });
    serde_json::to_string(&report).map_err(|e| format!("coverage ramp serialize failed: {e}"))
}

// ── WASM exports ──────────────────────────────────────────────────────────────
// Thin wrappers; errors surface as rejected JS exceptions with the same message
// the native API returns.

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn recommendations_json(input: &str) -> Result<String, JsValue> {
        super::recommendations_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn ban_suggestions_json(input: &str) -> Result<String, JsValue> {
        super::ban_suggestions_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn draft_verdict_json(input: &str) -> Result<String, JsValue> {
        super::draft_verdict_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn pool_coach_json(input: &str) -> Result<String, JsValue> {
        super::pool_coach_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn performance_report_json(input: &str) -> Result<String, JsValue> {
        super::performance_report_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn macro_state_json(input: &str) -> Result<String, JsValue> {
        super::macro_state_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn parse_session_json(input: &str) -> Result<String, JsValue> {
        super::parse_session_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn blended_meta_rates_json(input: &str) -> Result<String, JsValue> {
        super::blended_meta_rates_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn feedback_signals_json(input: &str) -> Result<String, JsValue> {
        super::feedback_signals_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn aram_weights_json() -> String {
        super::aram_weights_json_string()
    }

    #[wasm_bindgen]
    pub fn champion_analysis_json(input: &str) -> Result<String, JsValue> {
        super::champion_analysis_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn counter_picks_json(input: &str) -> Result<String, JsValue> {
        super::counter_picks_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn draft_verdict_full_json(input: &str) -> Result<String, JsValue> {
        super::draft_verdict_full_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn team_comp_json(input: &str) -> Result<String, JsValue> {
        super::team_comp_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn game_plan_json(input: &str) -> Result<String, JsValue> {
        super::game_plan_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn combo_board_json(input: &str) -> Result<String, JsValue> {
        super::combo_board_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn champion_archetypes_json() -> Result<String, JsValue> {
        super::champion_archetypes_string().map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn champion_detail_json(champion_id: u32) -> Result<String, JsValue> {
        super::champion_detail_string(champion_id).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn lane_matchup_json(input: &str) -> Result<String, JsValue> {
        super::lane_matchup_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn counter_items_json(input: &str) -> Result<String, JsValue> {
        super::counter_items_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn pool_suggestions_json(input: &str) -> Result<String, JsValue> {
        super::pool_suggestions_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn champion_pool_plan_json(input: &str) -> Result<String, JsValue> {
        super::champion_pool_plan_from_json(input).map_err(|e| JsValue::from_str(&e))
    }


    #[wasm_bindgen]
    pub fn feedback_observability_json(input: &str) -> Result<String, JsValue> {
        super::feedback_observability_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn feedback_analytics_json(input: &str) -> Result<String, JsValue> {
        super::feedback_analytics_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn draft_simulation_json(input: &str) -> Result<String, JsValue> {
        super::draft_simulation_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn draft_fork_json(input: &str) -> Result<String, JsValue> {
        super::draft_fork_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn ingame_plan_json(input: &str) -> Result<String, JsValue> {
        super::ingame_plan_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn macro_state_from_allgamedata_json(input: &str) -> Result<String, JsValue> {
        super::macro_state_from_allgamedata_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn feedback_flush_plan_json(input: &str) -> Result<String, JsValue> {
        super::feedback_flush_plan_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn feedback_flush_resolve_json(input: &str) -> Result<String, JsValue> {
        super::feedback_flush_resolve_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn data_source_registry_json(input: &str) -> Result<String, JsValue> {
        super::data_source_registry_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn pipeline_quality_report_json(input: &str) -> Result<String, JsValue> {
        super::pipeline_quality_report_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn data_trajectory_json(input: &str) -> Result<String, JsValue> {
        super::data_trajectory_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn cache_promotion_json(input: &str) -> Result<String, JsValue> {
        super::cache_promotion_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn match_fetch_plan_json(input: &str) -> Result<String, JsValue> {
        super::match_fetch_plan_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn match_discovery_plan_json(input: &str) -> Result<String, JsValue> {
        super::match_discovery_plan_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn match_v5_ingest_json(input: &str) -> Result<String, JsValue> {
        super::match_v5_ingest_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn pipeline_refresh_plan_json(input: &str) -> Result<String, JsValue> {
        super::pipeline_refresh_plan_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn coverage_ramp_json(input: &str) -> Result<String, JsValue> {
        super::coverage_ramp_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn game_review_json(input: &str) -> Result<String, JsValue> {
        super::game_review_from_json(input).map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen]
    pub fn trend_report_json(input: &str) -> Result<String, JsValue> {
        super::trend_report_from_json(input).map_err(|e| JsValue::from_str(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shared fixture: also exercised from Node against the WASM build
    /// (`core/tests/node/parity.mjs`) — keep the two in sync.
    const RECOMMENDATIONS_FIXTURE: &str =
        include_str!("../tests/fixtures/json_api_recommendations_input.json");

    #[test]
    fn recommendations_fixture_produces_ranked_output() {
        let out = recommendations_from_json(RECOMMENDATIONS_FIXTURE).expect("engine should run");
        let recs: serde_json::Value = serde_json::from_str(&out).expect("output must be JSON");
        let arr = recs.as_array().expect("output must be an array");
        assert!(!arr.is_empty(), "fixture should yield at least one recommendation");
        for rec in arr {
            assert!(rec.get("champion_id").is_some(), "recommendation has champion_id");
            assert!(rec.get("total_score").is_some(), "recommendation has total_score");
        }
    }

    #[test]
    fn recommendations_rejects_malformed_input() {
        let err = recommendations_from_json("{not json").unwrap_err();
        assert!(err.contains("invalid recommendations input"));
    }

    /// Post-processing parity: with no curated builds the fixture recs fall back
    /// to the archetype heuristic ("general"), flag the build gap honestly, and
    /// the DraftBrain upgrade always runs (local rules pack when no payload).
    #[test]
    fn recommendations_apply_general_build_and_draft_brain_upgrade() {
        let out = recommendations_from_json(RECOMMENDATIONS_FIXTURE).expect("engine should run");
        let recs: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = recs.as_array().unwrap();
        assert!(arr.len() >= 2, "fixture yields Garen + Malphite");

        for rec in arr {
            assert_eq!(rec["build_source"], "general", "no seed rows → heuristic build");
            assert!(
                !rec["core_items"].as_array().unwrap().is_empty(),
                "general build still carries core items"
            );
            let missing: Vec<&str> = rec["missing_signals"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            assert!(missing.contains(&"build"), "general build is flagged as missing");
            assert_eq!(rec["model_version"], "draft-brain-rules-v2");
            assert!(
                !rec["score_breakdown"].as_array().unwrap().is_empty(),
                "upgrade builds the score breakdown"
            );
        }
        // Comparative why_not lands on every non-best rec.
        assert!(
            !arr[1]["why_not"].as_array().unwrap().is_empty(),
            "runner-up carries a comparative why-not note"
        );
    }

    /// Curated seed rows win over the heuristic: matchup-specific row preferred,
    /// pro presence attached only from leaguepedia rows.
    #[test]
    fn recommendations_prefer_matchup_seed_build_and_attach_pro_presence() {
        let kb = DraftKnowledgeBase::load().unwrap();
        let darius_arch = kb.get_archetype("Darius").expect("Darius in KB").archetype.clone();

        let mut input: serde_json::Value =
            serde_json::from_str(RECOMMENDATIONS_FIXTURE).unwrap();
        input["builds"] = serde_json::json!([
            {
                "champion_id": 86,
                "item_ids": "[3078, 3742, 3065, 3026, 6333]",
                "rune_ids": "[8010, 8000]",
                "opponent_archetype": darius_arch,
                "skill_order": "Q→E→W",
                "summoner_spells": "[4, 12]",
                "secondary_runes": "[8400, 8444, 8453]",
                "stat_shards": "[5008, 5008, 5002]"
            },
            {
                "champion_id": 86,
                "item_ids": "[9999]",
                "rune_ids": "[1, 2]",
                "opponent_archetype": null
            }
        ]);
        input["pro_rows"] = serde_json::json!([
            { "champion_id": 86, "pick_rate": 0.30, "ban_rate": 0.25, "source": "leaguepedia" },
            { "champion_id": 54, "pick_rate": 0.50, "ban_rate": 0.10, "source": "u_gg" }
        ]);

        let out = recommendations_from_json(&input.to_string()).expect("engine should run");
        let recs: serde_json::Value = serde_json::from_str(&out).unwrap();
        let garen = recs
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["champion_id"] == 86)
            .expect("Garen recommended");

        assert_eq!(garen["build_source"], "seed");
        // enrich sets "high", but the upgrade honestly re-derives it from the
        // data pack quality — with the local-seed fallback pack that is "low".
        assert_eq!(garen["build_confidence"], "low");
        // Matchup-specific row (3078 first), NOT the default 9999 row.
        assert_eq!(garen["core_items"][0], 3078);
        assert_eq!(garen["core_items"].as_array().unwrap().len(), 4, "capped at 4");
        assert_eq!(garen["keystone"], 8010);
        assert_eq!(garen["primary_rune_tree"], 8000);
        assert_eq!(garen["skill_order"], "Q→E→W");
        assert_eq!(garen["summoner_spells"], serde_json::json!([4, 12]));
        let missing: Vec<&str> = garen["missing_signals"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        assert!(!missing.contains(&"build"), "seed build is not a missing signal");
        assert!((garen["pro_presence"].as_f64().unwrap() - 0.55).abs() < 1e-5);

        let malphite = recs
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["champion_id"] == 54)
            .expect("Malphite recommended");
        assert!(
            malphite.get("pro_presence").is_none() || malphite["pro_presence"].is_null(),
            "non-leaguepedia pro rows are ignored"
        );
    }

    /// Analysis path parity: single rec is enriched + upgraded (no comparative
    /// why_not — that note only exists relative to a ranked list).
    #[test]
    fn champion_analysis_is_enriched_and_upgraded() {
        let mut input: serde_json::Value =
            serde_json::from_str(RECOMMENDATIONS_FIXTURE).unwrap();
        input["champion_id"] = serde_json::json!(86);

        let out = champion_analysis_from_json(&input.to_string()).expect("analysis should run");
        let rec: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(rec["champion_id"], 86);
        assert_eq!(rec["build_source"], "general", "heuristic build attached");
        assert_eq!(rec["model_version"], "draft-brain-rules-v2");
        assert!(!rec["score_breakdown"].as_array().unwrap().is_empty());
    }

    #[test]
    fn draft_verdict_roundtrip() {
        let input = serde_json::json!({
            "plan": {
                "team_identity": "Teamfight / deathball",
                "identity_note": "5v5 kazanır",
                "timeline": [
                    { "label": "Erken (0-14dk)", "stance": "even", "advice": "", "strength": 0.5 },
                    { "label": "Orta (14-25dk)", "stance": "advantage", "advice": "", "strength": 0.6 },
                    { "label": "Geç (25dk+)", "stance": "advantage", "advice": "", "strength": 0.65 }
                ],
                "win_conditions": ["Objektif etrafında 5v5 al"],
                "objectives": ["dragon"],
                "enemy_threat": "Split push",
                "alt_plan": null,
                "partial": false
            },
            "ally": {
                "tanks": 1, "fighters": 1, "mages": 1, "marksmen": 1, "assassins": 0,
                "supports": 1, "ap_share": 0.45, "ad_share": 0.55,
                "has_engage": true, "has_frontline": true, "has_hard_cc": true,
                "has_peel": true, "gaps": [], "summary": "dengeli"
            },
            "enemy": {
                "tanks": 0, "fighters": 2, "mages": 1, "marksmen": 1, "assassins": 1,
                "supports": 0, "ap_share": 0.35, "ad_share": 0.65,
                "has_engage": false, "has_frontline": false, "has_hard_cc": false,
                "has_peel": false, "gaps": ["Frontline yok"], "summary": "kırılgan"
            },
            "lane_matchup": 0.55
        });
        let out = draft_verdict_from_json(&input.to_string()).expect("verdict should run");
        let verdict: serde_json::Value = serde_json::from_str(&out).expect("output must be JSON");
        assert!(verdict.is_object(), "verdict must be a JSON object");
    }

    #[test]
    fn pool_coach_roundtrip() {
        let input = serde_json::json!({
            "role": "top",
            "pool": [{
                "champion_id": 86, "champion_key": "Garen", "archetype": "juggernaut",
                "blind_safety": 0.8, "execution_difficulty": 1, "power_late": 0.6,
                "engage": false, "peel": false, "comfort": 0.9, "games": 40,
                "meta_strength": 0.55
            }],
            "candidates": []
        });
        let out = pool_coach_from_json(&input.to_string()).expect("pool coach should run");
        let plan: serde_json::Value = serde_json::from_str(&out).expect("output must be JSON");
        assert!(plan.is_object(), "pool plan must be a JSON object");
    }

    #[test]
    fn performance_report_roundtrip() {
        let input = serde_json::json!([{
            "champion_id": 86, "champion_key": "Garen", "position": "top",
            "win": true, "kills": 7, "deaths": 3, "assists": 5,
            "played_at": 1700000000, "duration_secs": 1800, "cs": 180
        }]);
        let out = performance_report_from_json(&input.to_string()).expect("report should run");
        let report: serde_json::Value = serde_json::from_str(&out).expect("output must be JSON");
        assert!(report.is_object(), "report must be a JSON object");
    }

    #[test]
    fn parse_session_handles_raw_lcu_payload() {
        let raw = serde_json::json!({
            "localPlayerCellId": 2,
            "myTeam": [
                {"cellId": 2, "championId": 99, "championPickIntent": 0,
                 "assignedPosition": "middle"}
            ],
            "theirTeam": [],
            "bans": {"myTeamBans": [], "theirTeamBans": []},
            "actions": [],
            "timer": {"phase": "BAN_PICK", "adjustedTimeLeftInPhase": 27000},
            "gameConfig": {"queueId": 420}
        });
        let out = parse_session_from_json(&raw.to_string()).expect("session should parse");
        let state: serde_json::Value = serde_json::from_str(&out).expect("output must be JSON");
        assert_eq!(state["my_cell_id"], 2);
        assert_eq!(state["queue_id"], 420);
        assert_eq!(state["local_player"]["champion_id"], 99);
        assert!(
            state.get("actions").is_none(),
            "parsed state must not look like a raw session"
        );
    }

    #[test]
    fn parse_session_rejects_invalid_payload_with_host_parity_message() {
        assert_eq!(
            parse_session_from_json("{}").unwrap_err(),
            "Geçersiz session JSON"
        );
        assert_eq!(
            parse_session_from_json("{not json").unwrap_err(),
            "Geçersiz session JSON"
        );
    }

    #[test]
    fn blended_meta_rates_blends_and_applies_role_share_filter() {
        // Olaf (2): 199646 top + 2621 bottom games → bottom is ~1% share → dropped.
        // Two agreeing bottom sources for Caitlyn (51) → samples sum.
        let input = serde_json::json!({
            "rows": [
                {"champion_id": 51, "position": "bottom", "win_rate": 0.52, "ban_rate": 0.08, "sample_size": 5000},
                {"champion_id": 51, "position": "bottom", "win_rate": 0.53, "ban_rate": 0.0, "sample_size": 9000},
                {"champion_id": 2, "position": "bottom", "win_rate": 0.55, "ban_rate": 0.0, "sample_size": 2621}
            ],
            "position_samples": [
                {"champion_id": 51, "position": "bottom", "sample_size": 14000},
                {"champion_id": 2, "position": "top", "sample_size": 199646},
                {"champion_id": 2, "position": "bottom", "sample_size": 2621}
            ]
        });
        let out = blended_meta_rates_from_json(&input.to_string()).expect("blend should run");
        let entries: serde_json::Value = serde_json::from_str(&out).expect("output must be JSON");
        let arr = entries.as_array().expect("output must be an array");
        assert_eq!(arr.len(), 1, "off-role Olaf bottom row must be filtered out");
        assert_eq!(arr[0]["champion_id"], 51);
        assert_eq!(arr[0]["position"], "bottom");
        assert_eq!(arr[0]["sample_size"], 14000, "agreeing sources sum evidence");
        assert!(
            (arr[0]["ban_rate"].as_f64().unwrap() - 0.08).abs() < 1e-5,
            "ban rate only from the source that reports one"
        );
    }

    #[test]
    fn feedback_signals_aggregate_raw_rows() {
        let input = serde_json::json!([
            {"champion_id": 86, "verdict": "helpful"},
            {"champion_id": 86, "verdict": "helpful"},
            {"champion_id": 86, "verdict": "not_helpful"},
            {"champion_id": 99, "verdict": "skipped"}
        ]);
        let out = feedback_signals_from_json(&input.to_string()).expect("aggregate should run");
        let signals: serde_json::Value = serde_json::from_str(&out).expect("output must be JSON");
        let arr = signals.as_array().expect("output must be an array");
        assert_eq!(arr.len(), 1, "skipped-only champions emit no signal");
        assert_eq!(arr[0]["champion_id"], 86);
        assert_eq!(arr[0]["positive"], 2);
        assert_eq!(arr[0]["negative"], 1);
        assert!(arr[0]["confidence"].is_string());
    }

    /// The analysis cluster shares the recommendations fixture: every endpoint
    /// must run and produce the right JSON shape from the same input document.
    #[test]
    fn analysis_cluster_runs_on_the_shared_fixture() {
        let base: serde_json::Value = serde_json::from_str(RECOMMENDATIONS_FIXTURE).unwrap();

        // champion_analysis: analyze the first recommended champion.
        let recs: serde_json::Value = serde_json::from_str(
            &recommendations_from_json(RECOMMENDATIONS_FIXTURE).unwrap(),
        )
        .unwrap();
        let top_id = recs[0]["champion_id"].as_u64().expect("rec has id");
        let mut analysis_input = base.clone();
        analysis_input["champion_id"] = top_id.into();
        let analysis: serde_json::Value =
            serde_json::from_str(&champion_analysis_from_json(&analysis_input.to_string()).unwrap())
                .unwrap();
        assert_eq!(analysis["champion_id"].as_u64(), Some(top_id));

        // champion_id 0 → null (command parity).
        let mut zero_input = base.clone();
        zero_input["champion_id"] = 0.into();
        assert_eq!(
            champion_analysis_from_json(&zero_input.to_string()).unwrap(),
            "null"
        );

        // counter_picks: must at least produce a JSON array.
        let counters: serde_json::Value =
            serde_json::from_str(&counter_picks_from_json(RECOMMENDATIONS_FIXTURE).unwrap())
                .unwrap();
        assert!(counters.is_array());

        // draft_verdict_full: object with the verdict fields.
        let verdict: serde_json::Value =
            serde_json::from_str(&draft_verdict_full_from_json(RECOMMENDATIONS_FIXTURE).unwrap())
                .unwrap();
        assert!(verdict.is_object());

        // Session-shaped endpoints reuse the fixture's session/champions/role_map.
        let team_input = serde_json::json!({
            "session": base["session"],
            "all_champions": base["all_champions"],
            "role_map": base["role_map"],
        });
        let board: serde_json::Value =
            serde_json::from_str(&team_comp_from_json(&team_input.to_string()).unwrap()).unwrap();
        assert!(board["ally"].is_object() && board["enemy"].is_object());

        let plan: serde_json::Value =
            serde_json::from_str(&game_plan_from_json(&team_input.to_string()).unwrap()).unwrap();
        assert!(plan["timeline"].is_array());

        let combos: serde_json::Value =
            serde_json::from_str(&combo_board_from_json(&team_input.to_string()).unwrap()).unwrap();
        assert!(combos.is_array());

        let items: serde_json::Value =
            serde_json::from_str(&counter_items_from_json(&team_input.to_string()).unwrap())
                .unwrap();
        assert!(items.is_array());

        // lane_matchup: object or null depending on fixture visibility — must not error.
        let lane = lane_matchup_from_json(&team_input.to_string()).unwrap();
        let lane_v: serde_json::Value = serde_json::from_str(&lane).unwrap();
        assert!(lane_v.is_object() || lane_v.is_null());
    }

    #[test]
    fn kb_endpoints_serve_archetypes_and_details() {
        let archetypes: serde_json::Value =
            serde_json::from_str(&champion_archetypes_string().unwrap()).unwrap();
        let arr = archetypes.as_array().expect("array");
        assert!(arr.len() > 100, "KB 172 şampiyon taşır, {} bulundu", arr.len());
        let first_id = arr[0]["champion_id"].as_u64().unwrap() as u32;

        let detail: serde_json::Value =
            serde_json::from_str(&champion_detail_string(first_id).unwrap()).unwrap();
        assert_eq!(detail["champion_id"].as_u64().unwrap() as u32, first_id);
        assert!(detail["win_condition"].is_string());

        // Unknown champion → null.
        assert_eq!(champion_detail_string(999_999).unwrap(), "null");
    }

    #[test]
    fn pool_and_insight_endpoints_run_on_synthetic_rows() {
        // Garen (86) anlamlı yatırım (lvl 7 / 250k) + top meta → havuzda.
        let pool_input = serde_json::json!({
            "role": "top",
            "mastery": [
                {"champion_id": 86, "mastery_level": 7, "mastery_points": 250000, "last_play_time": null}
            ],
            "stats": [{"champion_id": 86, "games": 40, "wins": 24, "kda_avg": 2.5}],
            "all_champions": [
                {"champion_id": 86, "key": "Garen", "name": "Garen", "title": "t"}
            ],
            "meta_rates": [
                {"champion_id": 86, "position": "top", "win_rate": 0.52, "ban_rate": 0.05, "sample_size": 5000}
            ]
        });
        let plan: serde_json::Value =
            serde_json::from_str(&champion_pool_plan_from_json(&pool_input.to_string()).unwrap())
                .unwrap();
        assert!(plan.is_object());

        let suggestions: serde_json::Value =
            serde_json::from_str(&pool_suggestions_from_json(&pool_input.to_string()).unwrap())
                .unwrap();
        let arr = suggestions.as_array().expect("array");
        assert!(
            arr.iter().all(|s| s["champion_id"] != 86),
            "owned champion must not be a learn suggestion"
        );

        // Observability: 2 satır, 1'i sync bekliyor.
        let obs_input = serde_json::json!([
            {"champion_id": 86, "verdict": "helpful", "synced": true},
            {"champion_id": 86, "verdict": "not_helpful", "synced": false}
        ]);
        let obs: serde_json::Value =
            serde_json::from_str(&feedback_observability_from_json(&obs_input.to_string()).unwrap())
                .unwrap();
        assert_eq!(obs["counters"]["pending_sync"], 1);
        assert!(obs["status"].is_object() || obs["status"].is_string());

        // Analytics: window içinde 1 event.
        let an_input = serde_json::json!({
            "events": [{"champion_id": 86, "champion_key": "Garen", "verdict": "helpful", "created_at": 1700000000}],
            "now_secs": 1700000100,
            "window_days": 7
        });
        let analytics: serde_json::Value =
            serde_json::from_str(&feedback_analytics_from_json(&an_input.to_string()).unwrap())
                .unwrap();
        assert!(analytics.is_object());
    }

    #[test]
    fn aram_weights_match_core_preset() {
        let weights: ScoringWeights =
            serde_json::from_str(&aram_weights_json_string()).expect("weights must be JSON");
        let expected = ScoringWeights::aram();
        assert_eq!(weights.comfort, expected.comfort);
        assert_eq!(weights.matchup, expected.matchup);
        assert_eq!(weights.role_fit, expected.role_fit);
        assert_eq!(weights.risk, expected.risk);
    }

    #[test]
    fn draft_sim_and_fork_run_on_a_synthetic_draft() {
        // Garen (yerel, top, hover) vs Darius görünür rakip; Zed (238) banlı.
        let session = serde_json::json!({
            "my_cell_id": 0,
            "local_player": {
                "cell_id": 0, "champion_id": 0, "intent_champion_id": 86,
                "assigned_position": "top", "is_locked": false
            },
            "my_team": [
                { "cell_id": 0, "champion_id": 0, "intent_champion_id": 86,
                  "assigned_position": "top", "is_locked": false }
            ],
            "their_team": [
                { "cell_id": 5, "champion_id": 122, "intent_champion_id": 0,
                  "assigned_position": "top", "is_locked": true }
            ],
            "my_bans": [238], "their_bans": [],
            "phase": "BAN_PICK", "time_left_ms": 20000, "action_type": "pick",
            "queue_id": 420, "pick_order": 2
        });
        let champions = serde_json::json!([
            { "champion_id": 86, "key": "Garen", "name": "Garen", "title": "t" },
            { "champion_id": 122, "key": "Darius", "name": "Darius", "title": "t" },
            { "champion_id": 238, "key": "Zed", "name": "Zed", "title": "t" }
        ]);

        // 86 geçerli; 0 ve dup atılır; 122 (rakipte) ve 238 (ban) unavailable.
        let sim_input = serde_json::json!({
            "session": &session,
            "all_champions": &champions,
            "candidate_ids": [86, 0, 86, 122, 238]
        });
        let out = draft_simulation_from_json(&sim_input.to_string()).expect("sim should run");
        let results: serde_json::Value = serde_json::from_str(&out).unwrap();
        let arr = results.as_array().expect("array");
        assert_eq!(arr.len(), 1, "yalnız Garen geçerli aday");

        // Boş aday listesi → [].
        let empty = serde_json::json!({
            "session": &session, "all_champions": &champions, "candidate_ids": []
        });
        assert_eq!(draft_simulation_from_json(&empty.to_string()).unwrap(), "[]");

        // Fork: aynı id → null; banlı opsiyon → null; Garen vs Sett benzeri geçerli çift → obje.
        let same = serde_json::json!({
            "session": &session, "all_champions": &champions,
            "option_a_id": 86, "option_b_id": 86
        });
        assert_eq!(draft_fork_from_json(&same.to_string()).unwrap(), "null");

        let banned = serde_json::json!({
            "session": &session, "all_champions": &champions,
            "option_a_id": 86, "option_b_id": 238
        });
        assert_eq!(draft_fork_from_json(&banned.to_string()).unwrap(), "null");

        // Tabloda olmayan id → KB çözülemez → null (komut paritesi).
        let unknown = serde_json::json!({
            "session": &session, "all_champions": &champions,
            "option_a_id": 86, "option_b_id": 999999
        });
        assert_eq!(draft_fork_from_json(&unknown.to_string()).unwrap(), "null");

        // Geçersiz session → host-parite hatası.
        let bad = serde_json::json!({
            "session": {"actions": []}, "all_champions": &champions,
            "candidate_ids": [86]
        });
        assert_eq!(
            draft_simulation_from_json(&bad.to_string()).unwrap_err(),
            "Geçersiz session JSON"
        );
    }

    #[test]
    fn ingame_endpoints_run_on_raw_allgamedata() {
        let allgamedata = serde_json::json!({
            "activePlayer": { "riotIdGameName": "Me" },
            "allPlayers": [
                { "riotIdGameName": "Me", "championName": "Garen",
                  "rawChampionName": "game_character_displayname_Garen",
                  "position": "TOP", "team": "ORDER", "level": 9,
                  "scores": { "kills": 2, "deaths": 1, "assists": 1, "creepScore": 70 } },
                { "riotIdGameName": "Foe", "championName": "Darius",
                  "position": "TOP", "team": "CHAOS" }
            ],
            "gameData": { "gameTime": 600.0 },
            "events": { "Events": [ { "EventName": "DragonKill", "EventTime": 480.0 } ] }
        });

        let plan_input = serde_json::json!({
            "allgamedata": &allgamedata,
            "all_champions": [
                { "champion_id": 86, "key": "Garen", "name": "Garen", "title": "t" },
                { "champion_id": 122, "key": "Darius", "name": "Darius", "title": "t" }
            ]
        });
        let plan: serde_json::Value =
            serde_json::from_str(&ingame_plan_from_json(&plan_input.to_string()).unwrap()).unwrap();
        assert_eq!(plan["champion_key"], "Garen");
        assert_eq!(plan["position"], "top");
        assert_eq!(plan["opponent_key"], "Darius");

        // Şampiyon tablosu boş → KB çözülemez → null (sessiz, beklenen).
        let no_table = serde_json::json!({ "allgamedata": &allgamedata, "all_champions": [] });
        assert_eq!(ingame_plan_from_json(&no_table.to_string()).unwrap(), "null");

        // Macro state doğrudan raw payload'dan: dragon 480'de → 780'de yeniden.
        let state: serde_json::Value = serde_json::from_str(
            &macro_state_from_allgamedata_json(&allgamedata.to_string()).unwrap(),
        )
        .unwrap();
        let dragon = state["objectives"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["objective"] == "dragon")
            .expect("dragon timer");
        assert_eq!(dragon["next_spawn_secs"], 780);
    }

    #[test]
    fn feedback_flush_plan_and_resolve_follow_the_sync_policy() {
        // 1: gönderilebilir (hash ≥16); 2: hash yok → skip; 3: backoff dolmamış → wait.
        let plan_input = serde_json::json!({
            "rows": [
                {"id": 1, "champion_id": 238, "feedback": "helpful",
                 "session_hash": "0123456789abcdef0123", "created_at": 1_800_000_000i64,
                 "retry_count": 0},
                {"id": 2, "champion_id": 238, "feedback": "helpful",
                 "session_hash": null, "created_at": 1_800_000_000i64, "retry_count": 0},
                {"id": 3, "champion_id": 238, "feedback": "helpful",
                 "session_hash": "0123456789abcdef0123", "created_at": 1_800_000_000i64,
                 "retry_count": 1, "next_retry_at": 9_999_999_999i64}
            ],
            "now_secs": 1_800_000_100i64
        });
        let plans: serde_json::Value =
            serde_json::from_str(&feedback_flush_plan_from_json(&plan_input.to_string()).unwrap())
                .unwrap();
        assert_eq!(plans[0]["action"], "send");
        // Anahtar feedback_sync::idempotency_key ile birebir — backend dedup'u buna bağlı.
        assert_eq!(
            plans[0]["idempotency_key"].as_str().unwrap(),
            crate::feedback_sync::idempotency_key(
                "0123456789abcdef0123",
                238,
                "helpful",
                1_800_000_000
            )
        );
        assert_eq!(plans[1]["action"], "skip_no_hash");
        assert_eq!(plans[2]["action"], "wait");

        // Başarı → synced; hata → retry artar, satır kuyruğta kalır (30s backoff).
        let ok: serde_json::Value = serde_json::from_str(
            &feedback_flush_resolve_from_json(
                &serde_json::json!({"retry_count": 0, "ok": true, "now_secs": 5000}).to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(ok["synced_at"], 5000);

        let failed: serde_json::Value = serde_json::from_str(
            &feedback_flush_resolve_from_json(
                &serde_json::json!({
                    "retry_count": 0, "ok": false, "error": "HTTP 503", "now_secs": 5000
                })
                .to_string(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(failed["synced_at"].is_null(), "hata satırı asla synced olmaz");
        assert_eq!(failed["retry_count"], 1);
        assert_eq!(failed["next_retry_at"], 5030);
    }

    #[test]
    fn data_quality_read_trio_runs_on_raw_counts() {
        // Boş DB: pack hiç sync edilmemiş → fallback aktif + data_pack stale.
        let empty = serde_json::json!({
            "counts": {
                "total_champions": 0, "champion_rates_count": 0, "matchup_count": 0,
                "build_count": 0, "build_champions": 0, "meta_role_champions": 0
            },
            "data_pack": null,
            "now_secs": 1_800_000_000i64
        });
        let registry: serde_json::Value =
            serde_json::from_str(&data_source_registry_from_json(&empty.to_string()).unwrap())
                .unwrap();
        assert_eq!(registry["fallback_active"], true);
        assert!(registry["stale_sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "data_pack"));

        // Taze cloud pack + canlı rate satırları → fallback YOK, meraki kaynağı listede.
        let live = serde_json::json!({
            "counts": {
                "total_champions": 172, "champion_rates_count": 800, "matchup_count": 1200,
                "build_count": 300, "build_champions": 150, "meta_role_champions": 160
            },
            "data_pack": { "source": "cloud", "expires_at": 1_800_100_000i64 },
            "now_secs": 1_800_000_000i64
        });
        let live_reg: serde_json::Value =
            serde_json::from_str(&data_source_registry_from_json(&live.to_string()).unwrap())
                .unwrap();
        assert_eq!(live_reg["fallback_active"], false);
        assert!(live_reg["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["source"] == "cloud_postgres"));

        // Pipeline raporu: kaynak satırı yoksa local_seed varsayılanı; status string döner.
        let quality_input = serde_json::json!({
            "counts": empty["counts"],
            "data_pack": null,
            "now_secs": 1_800_000_000i64,
            "current_patch": "",
            "data_patch": null,
            "source_rows": [],
            "pack_exists": false
        });
        let report: serde_json::Value = serde_json::from_str(
            &pipeline_quality_report_from_json(&quality_input.to_string()).unwrap(),
        )
        .unwrap();
        assert!(report["status"].is_string());
        assert!(report["actions"].is_array());

        // Kaynak risk türetimi: lolalytics=high, riot=medium (source_risk paritesi).
        let risky = serde_json::json!({
            "counts": live["counts"],
            "data_pack": null,
            "now_secs": 1_800_000_000i64,
            "current_patch": "16.10",
            "data_patch": "16.10",
            "source_rows": [
                {"source": "rates:lolalytics", "updated_at": 1_799_999_000i64},
                {"source": "matchups:riot_match_v5", "updated_at": 1_799_999_000i64}
            ],
            "pack_exists": true
        });
        let risky_report = pipeline_quality_report_from_json(&risky.to_string()).unwrap();
        assert!(risky_report.contains("\"status\""));

        // Trajectory: ramp yok → dürüst "unknown"; ramp varsa core sınıflandırır.
        let no_ramp = serde_json::json!({
            "quality_status": "healthy", "ramp": null,
            "riot_key_present": false, "has_summoner": false,
            "last_match_v5_success_at": null, "now_secs": 1_800_000_000i64
        });
        let view: serde_json::Value =
            serde_json::from_str(&data_trajectory_from_json(&no_ramp.to_string()).unwrap())
                .unwrap();
        assert_eq!(view["trajectory"], "unknown");
        assert_eq!(view["match_v5_enabled"], false);
        assert!(view["match_v5_age_secs"].is_null());

        let with_ramp = serde_json::json!({
            "quality_status": "healthy",
            "ramp": { "ramp_state": "growing", "data_growing": true, "measured_at": 1_799_999_000i64 },
            "riot_key_present": true, "has_summoner": true,
            "last_match_v5_success_at": 1_799_999_500i64, "now_secs": 1_800_000_000i64
        });
        let ramped: serde_json::Value =
            serde_json::from_str(&data_trajectory_from_json(&with_ramp.to_string()).unwrap())
                .unwrap();
        assert!(ramped["trajectory"].is_string());
        assert_ne!(ramped["trajectory"], "unknown");
        assert_eq!(ramped["match_v5_enabled"], true);
        assert_eq!(ramped["match_v5_age_secs"], 500);
    }

    #[test]
    fn cache_promotion_decides_and_emits_a_pack() {
        // Mevcut cache yok + orta riskli dolu aday → promote + pack üretildi.
        let input = serde_json::json!({
            "counts": {
                "total_champions": 172, "champion_rates_count": 800, "matchup_count": 1200,
                "build_count": 300, "build_champions": 150, "meta_role_champions": 160
            },
            "data_pack": null,
            "build_champions_known": 140,
            "patch": "16.10",
            "region": null,
            "now_secs": 1_800_000_000i64
        });
        let out: serde_json::Value =
            serde_json::from_str(&cache_promotion_from_json(&input.to_string()).unwrap()).unwrap();
        assert_eq!(out["promoted"], true);
        let payload: serde_json::Value =
            serde_json::from_str(out["pack_payload_json"].as_str().unwrap()).unwrap();
        assert_eq!(payload["generated_at"], 1_800_000_000u32);
        assert_eq!(payload["patch"], "16.10");

        // Taze, dolu bir cloud pack varken zayıf aday promote EDİLMEZ.
        let weak = serde_json::json!({
            "counts": {
                "total_champions": 10, "champion_rates_count": 0, "matchup_count": 0,
                "build_count": 0, "build_champions": 0, "meta_role_champions": 0
            },
            "data_pack": {
                "source": "cloud",
                "expires_at": 1_800_100_000i64,
                "payload_json": serde_json::json!({
                    "version": "cloud-data-v1", "patch": "16.10", "region": null,
                    "sources": ["cloud_postgres"],
                    "quality": {"rates": 172, "matchups": 5000, "builds": 172,
                                 "feedback": 0, "draft_samples": 0},
                    "fallback": false
                }).to_string()
            },
            "build_champions_known": 150,
            "patch": "16.10",
            "region": null,
            "now_secs": 1_800_000_000i64
        });
        let kept: serde_json::Value =
            serde_json::from_str(&cache_promotion_from_json(&weak.to_string()).unwrap()).unwrap();
        assert_eq!(kept["promoted"], false);
        assert!(kept["pack_payload_json"].is_null());
        assert!(kept["reason"].is_string());
    }

    #[test]
    fn match_v5_endpoints_plan_and_ingest() {
        // Fetch planı: 2 aday, 1'i zaten processed → yalnız diğeri seçilir.
        let plan_input = serde_json::json!({
            "now_secs": 1_800_000_000i64,
            "rate_budget": 50, "batch_limit": 50,
            "candidates": [
                {"match_id": "EUW1_1", "region": "euw1", "patch": "16.10",
                 "queue_id": 420, "discovered_at": 1_799_999_000i64},
                {"match_id": "EUW1_2", "region": "euw1", "patch": "16.10",
                 "queue_id": 420, "discovered_at": 1_799_999_100i64}
            ],
            "fetched_records": [
                {"match_id": "EUW1_1", "region": "euw1", "patch": "16.10",
                 "status": "processed", "fetched_at": 1_799_999_500i64}
            ],
            "frontiers": [
                {"region": "euw1", "patch": "16.10", "role": "top",
                 "current_samples": 0, "target_samples": 1000}
            ]
        });
        let plan: serde_json::Value =
            serde_json::from_str(&match_fetch_plan_from_json(&plan_input.to_string()).unwrap())
                .unwrap();
        let to_fetch: Vec<&str> = plan["to_fetch"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(to_fetch.contains(&"EUW1_2"));
        assert!(!to_fetch.contains(&"EUW1_1"), "processed maç yeniden çekilmez");

        // Ingest: 1 geçerli detay + 1 bozuk → canonical satırlar + failed id + hash.
        let detail = serde_json::json!({
            "metadata": { "matchId": "EUW1_2" },
            "info": {
                "queueId": 420, "gameVersion": "16.10.1",
                "participants": [
                    {"participantId": 1, "championId": 86, "teamId": 100,
                     "teamPosition": "TOP", "win": true, "kills": 5, "deaths": 2,
                     "assists": 3, "puuid": "puuid-a", "summoner1Id": 4, "summoner2Id": 12,
                     "item0": 3071, "perks": {"styles": [{"selections": [{"perk": 8010}]}]}},
                    {"participantId": 2, "championId": 122, "teamId": 200,
                     "teamPosition": "TOP", "win": false, "kills": 2, "deaths": 5,
                     "assists": 1, "puuid": "puuid-b", "summoner1Id": 4, "summoner2Id": 14,
                     "item0": 6630, "perks": {"styles": [{"selections": [{"perk": 8437}]}]}}
                ]
            }
        });
        let ingest_input = serde_json::json!({
            "details": [detail, {"bozuk": true}],
            "ids": ["EUW1_2", "EUW1_broken"],
            "region": "euw1"
        });
        let out: serde_json::Value =
            serde_json::from_str(&match_v5_ingest_from_json(&ingest_input.to_string()).unwrap())
                .unwrap();
        assert_eq!(out["match_count"], 1);
        assert_eq!(out["failed"][0], "EUW1_broken");
        assert_eq!(out["parsed"][0]["patch"], "16.10");
        assert!(!out["row_set"]["matchups"].as_array().unwrap().is_empty());
        let p0 = &out["participants"][0];
        assert_eq!(p0["puuid"], "puuid-a");
        assert_eq!(
            p0["hash"].as_str().unwrap().len(),
            16,
            "FNV-1a 64 hex hash diske yazılan tek kimlik"
        );

        // Discovery planı: taze seed → crawl listesinde.
        let disc_input = serde_json::json!({
            "now_secs": 1_800_000_000i64,
            "crawl_budget": 15, "max_breadth": 15, "per_player_match_cap": 5,
            "seeds": [
                {"puuid_hash": "abc123", "region": "euw1", "source": "match_participant",
                 "seen_at": 1_799_999_900i64, "contribution_count": 2}
            ],
            "crawled_players": [], "candidate_matches": [], "known_matches": []
        });
        let disc: serde_json::Value = serde_json::from_str(
            &match_discovery_plan_from_json(&disc_input.to_string()).unwrap(),
        )
        .unwrap();
        assert!(disc["to_crawl"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "abc123"));
    }

    #[test]
    fn macro_state_roundtrip() {
        let input = serde_json::json!({
            "game_time_secs": 600,
            "events": [{ "objective": "dragon", "killed_at_secs": 480 }]
        });
        let out = macro_state_from_json(&input.to_string()).expect("macro state should run");
        let state: serde_json::Value = serde_json::from_str(&out).expect("output must be JSON");
        assert!(state.is_object(), "macro state must be a JSON object");
    }

    #[test]
    fn pipeline_refresh_plan_drives_scheduler_decisions() {
        let now = 1_800_000_000i64;
        // Boş log + disabled u_gg/leaguepedia + riot kapalı: ddragon/meraki
        // refresh (insufficient), diğerleri dürüst skip_disabled.
        let input = serde_json::json!({
            "now_secs": now,
            "champ_select_active": false,
            "riot_enabled": false,
            "matchup_count": 0,
            "logs": [],
            "request_timestamps": [],
            "disabled_sources": ["u_gg", "leaguepedia"]
        });
        let out: serde_json::Value =
            serde_json::from_str(&pipeline_refresh_plan_from_json(&input.to_string()).unwrap())
                .unwrap();
        let decision = |key: &str| {
            out["plan"]["decisions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["source"] == key)
                .unwrap_or_else(|| panic!("{key} kararı yok"))["decision"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(decision("ddragon"), "refresh");
        assert_eq!(decision("meraki"), "refresh");
        assert_eq!(decision("u_gg"), "skip_disabled");
        assert_eq!(decision("leaguepedia"), "skip_disabled");
        assert_eq!(decision("match_v5"), "skip_disabled");
        // Edge worker yapılandırılmadı (edge_enabled default false) → dürüst skip.
        assert_eq!(decision("cloud_edge"), "skip_disabled");
        assert_eq!(out["rate_limit"]["max_requests"], 60);

        // Edge yapılandırıldığında (boş log → insufficient) refresh'e girer.
        let edge_on = serde_json::json!({ "now_secs": now, "edge_enabled": true,
            "disabled_sources": ["u_gg", "leaguepedia", "meraki", "ddragon"] });
        let out_edge: serde_json::Value =
            serde_json::from_str(&pipeline_refresh_plan_from_json(&edge_on.to_string()).unwrap())
                .unwrap();
        let edge = out_edge["plan"]["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["source"] == "cloud_edge")
            .unwrap();
        assert_eq!(edge["decision"], "refresh");

        // Taze ddragon başarısı → skip_fresh (TTL 24h içinde, sağlık healthy).
        let fresh = serde_json::json!({
            "now_secs": now,
            "riot_enabled": false,
            "logs": [{ "source": "ddragon", "status": "success", "at": now - 600 }],
            "disabled_sources": ["u_gg", "leaguepedia", "meraki"]
        });
        let out: serde_json::Value =
            serde_json::from_str(&pipeline_refresh_plan_from_json(&fresh.to_string()).unwrap())
                .unwrap();
        let ddragon = out["plan"]["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["source"] == "ddragon")
            .unwrap();
        assert_eq!(ddragon["decision"], "skip_fresh");

        // Champ-select aktif → HER kaynak skip_champ_select (ağ yasak).
        let cs = serde_json::json!({ "now_secs": now, "champ_select_active": true });
        let out: serde_json::Value =
            serde_json::from_str(&pipeline_refresh_plan_from_json(&cs.to_string()).unwrap())
                .unwrap();
        assert_eq!(out["plan"]["champ_select_blocked"], true);
        for d in out["plan"]["decisions"].as_array().unwrap() {
            assert_eq!(d["decision"], "skip_champ_select");
        }
    }

    #[test]
    fn coverage_ramp_endpoint_matches_engine_verdicts() {
        // İlk canlı run anchor'ı (coverage_ramp testleriyle aynı sayılar).
        let input = serde_json::json!({
            "before": { "taken_at": 0, "champion_rate_rows": 0, "matchup_rows": 80,
                        "build_rows": 31 },
            "after": { "taken_at": 3600, "champion_rate_rows": 49, "matchup_rows": 140,
                       "build_rows": 80, "discovered_matches": 24,
                       "processed_matches": 6, "crawled_players": 52 },
            "champ_select_active": false,
            "crawl_budget": 15
        });
        let out: serde_json::Value =
            serde_json::from_str(&coverage_ramp_from_json(&input.to_string()).unwrap()).unwrap();
        assert_eq!(out["ramp_state"], "progressing");
        assert_eq!(out["data_growing"], true);
        assert_eq!(out["deltas"]["matchup_delta"], 60);

        // Champ-select'te ertelenen tick → no_budget → trajectory "deferred".
        let deferred = serde_json::json!({
            "before": { "taken_at": 0 }, "after": { "taken_at": 60 },
            "champ_select_active": true, "crawl_budget": 0
        });
        let out: serde_json::Value =
            serde_json::from_str(&coverage_ramp_from_json(&deferred.to_string()).unwrap())
                .unwrap();
        assert_eq!(out["ramp_state"], "no_budget");
        let trajectory = serde_json::json!({
            "quality_status": "degraded",
            "ramp": { "ramp_state": out["ramp_state"], "data_growing": out["data_growing"],
                      "measured_at": 60 },
            "now_secs": 60
        });
        let view: serde_json::Value =
            serde_json::from_str(&data_trajectory_from_json(&trajectory.to_string()).unwrap())
                .unwrap();
        assert_eq!(view["trajectory"], "deferred");
    }
}
