//! Data Pipeline Ingestion Contract + Last-Good Policy (Sprint, Claude).
//!
//! Two engine-pure pieces, both decoupled from the fetcher/DB/UI:
//!   1. **Canonical rows** — `AggregationResult` → DB-upsert-ready `Canonical*Row`s
//!      (champion_rates / champion_matchups / builds superset: source, patch,
//!      confidence, sample_size). Codex maps these to SQLite/backend upserts.
//!   2. **Last-good cache policy** — pure decision of when a freshly-fetched
//!      candidate should be PROMOTED to the last-good cache, when the existing
//!      cache should be KEPT (regression / staleness guard), and how a high-risk
//!      source is dropped. No fabrication: an unusable candidate with no cache is
//!      `reject` (insufficient), not a fake promote.
//!
//! Tokens (action / source / confidence) are stable machine keys.
#![allow(dead_code)] // consumed by the fetcher/upsert/cache wiring (Codex, later)

use crate::match_v5_aggregator::AggregationResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// A candidate may be this much WORSE in coverage and still promote; beyond it the
/// existing last-good is kept (regression guard).
const COVERAGE_REGRESSION_TOLERANCE: f32 = 0.10;

// ── Canonical upsert rows (ts-rs) ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CanonicalRateRow {
    pub region: String,
    pub patch: String,
    pub champion_id: u32,
    pub position: String,
    pub win_rate: f32,
    pub pick_rate: f32,
    pub ban_rate: f32,
    pub sample_size: u32,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CanonicalMatchupRow {
    pub region: String,
    pub patch: String,
    pub champion_id: u32,
    pub opponent_id: u32,
    pub position: String,
    pub games: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub sample_size: u32,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CanonicalBuildRow {
    pub region: String,
    pub patch: String,
    pub champion_id: u32,
    pub position: String,
    /// core + situational, in priority order.
    pub item_ids: Vec<u32>,
    pub rune_ids: Vec<u32>,
    pub summoner_spells: Vec<u32>,
    pub games: u32,
    pub win_rate: f32,
    pub pick_rate: f32,
    pub sample_size: u32,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CanonicalRowSet {
    pub region: String,
    pub rates: Vec<CanonicalRateRow>,
    pub matchups: Vec<CanonicalMatchupRow>,
    pub builds: Vec<CanonicalBuildRow>,
}

/// Map an aggregation result to DB-upsert-ready canonical rows for a region. Pure;
/// preserves source/patch/confidence/sample_size. Build `pick_rate` is joined from
/// the matching champion-rate row (same champ/pos/patch).
pub fn to_canonical_rows(result: &AggregationResult, region: &str) -> CanonicalRowSet {
    let rate_pick: HashMap<(u32, &str, &str), f32> = result
        .rates
        .iter()
        .map(|r| {
            (
                (r.champion_id, r.position.as_str(), r.patch.as_str()),
                r.pick_rate,
            )
        })
        .collect();

    let rates = result
        .rates
        .iter()
        .map(|r| CanonicalRateRow {
            region: region.to_string(),
            patch: r.patch.clone(),
            champion_id: r.champion_id,
            position: r.position.clone(),
            win_rate: r.win_rate,
            pick_rate: r.pick_rate,
            ban_rate: r.ban_rate,
            sample_size: r.sample_size,
            source: r.source.clone(),
            confidence: r.confidence.clone(),
        })
        .collect();

    let matchups = result
        .matchups
        .iter()
        .map(|m| CanonicalMatchupRow {
            region: region.to_string(),
            patch: m.patch.clone(),
            champion_id: m.champion_id,
            opponent_id: m.opponent_id,
            position: m.position.clone(),
            games: m.games,
            wins: m.wins,
            win_rate: m.win_rate,
            sample_size: m.games,
            source: m.source.clone(),
            confidence: m.confidence.clone(),
        })
        .collect();

    let builds = result
        .builds
        .iter()
        .map(|b| {
            let mut item_ids = b.core_items.clone();
            item_ids.extend(b.situational_items.iter().copied());
            let pick_rate = rate_pick
                .get(&(b.champion_id, b.position.as_str(), b.patch.as_str()))
                .copied()
                .unwrap_or(0.0);
            CanonicalBuildRow {
                region: region.to_string(),
                patch: b.patch.clone(),
                champion_id: b.champion_id,
                position: b.position.clone(),
                item_ids,
                rune_ids: b.rune_ids.clone(),
                summoner_spells: b.summoner_spells.clone(),
                games: b.games,
                win_rate: b.win_rate,
                pick_rate,
                sample_size: b.games,
                source: b.source.clone(),
                confidence: b.confidence.clone(),
            }
        })
        .collect();

    CanonicalRowSet {
        region: region.to_string(),
        rates,
        matchups,
        builds,
    }
}

// ── Last-good cache promotion / rollback policy ─────────────────────────────────

/// Quality summary of a fetched candidate (or the current cached last-good).
#[derive(Debug, Clone)]
pub struct CandidateQuality {
    pub source: String,
    /// "low" | "medium" | "high".
    pub risk_level: String,
    /// Overall coverage in [0, 1].
    pub coverage_score: f32,
    pub sample_size: u32,
    pub fresh: bool,
}

/// Cache decision. `action` ∈ {promote, keep_current, reject}; `promoted` mirrors it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct CachePromotionDecision {
    pub action: String,
    pub promoted: bool,
    pub reason: String,
}

fn decision(action: &str, promoted: bool, reason: &str) -> CachePromotionDecision {
    CachePromotionDecision {
        action: action.to_string(),
        promoted,
        reason: reason.to_string(),
    }
}

/// Decide whether to promote `candidate` to last-good, keep the current cache, or
/// reject (no usable data + no cache). Pure.
///
/// Order: a high-risk source is never promoted over (or into) the cache; an
/// empty/unusable candidate keeps the cache; a significant coverage regression or a
/// stale candidate vs a fresh cache keeps the cache; otherwise promote.
pub fn decide_cache_promotion(
    candidate: &CandidateQuality,
    current_good: Option<&CandidateQuality>,
) -> CachePromotionDecision {
    if candidate.risk_level == "high" {
        return match current_good {
            Some(_) => decision(
                "keep_current",
                false,
                "Yüksek riskli kaynak; mevcut son-iyi cache korunuyor.",
            ),
            None => decision(
                "reject",
                false,
                "Yüksek riskli kaynak + son-iyi cache yok; promote edilmez.",
            ),
        };
    }

    let usable = candidate.coverage_score > 0.0 && candidate.sample_size > 0;
    if !usable {
        return match current_good {
            Some(_) => decision(
                "keep_current",
                false,
                "Aday boş/yetersiz; mevcut cache korunuyor.",
            ),
            None => decision("reject", false, "Aday boş ve son-iyi cache yok."),
        };
    }

    match current_good {
        None => decision(
            "promote",
            true,
            "İlk geçerli veri; son-iyi cache olarak promote edildi.",
        ),
        Some(cur) => {
            if candidate.coverage_score + COVERAGE_REGRESSION_TOLERANCE < cur.coverage_score {
                decision(
                    "keep_current",
                    false,
                    "Aday kapsaması mevcut cache'den belirgin düşük; regresyon koruması.",
                )
            } else if !candidate.fresh && cur.fresh {
                decision(
                    "keep_current",
                    false,
                    "Aday bayat, mevcut cache taze; korunuyor.",
                )
            } else {
                decision(
                    "promote",
                    true,
                    "Aday taze ve kapsamı yeterli; cache promote edildi.",
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::match_v5_aggregator::{
        aggregate_matches, MatchV5, MatchV5Participant,
    };

    fn part(champ: u32, team: u32, pos: &str, win: bool) -> MatchV5Participant {
        MatchV5Participant {
            participant_id: champ,
            champion_id: champ,
            team_id: team,
            team_position: pos.into(),
            win,
            kills: 0,
            deaths: 0,
            assists: 0,
            items: vec![1001, 3006],
            runes: vec![8005],
            summoner_spells: vec![4, 12],
        }
    }

    fn ranked(id: &str, patch: &str) -> MatchV5 {
        let positions = ["TOP", "JUNGLE", "MIDDLE", "BOTTOM", "UTILITY"];
        let mut participants = Vec::new();
        for (i, pos) in positions.iter().enumerate() {
            participants.push(part(i as u32 + 1, 100, pos, true));
            participants.push(part(i as u32 + 11, 200, pos, false));
        }
        MatchV5 {
            match_id: id.into(),
            queue_id: 420,
            patch: patch.into(),
            participants,
            bans: Vec::new(),
        }
    }

    // ── Canonical rows + fixture ────────────────────────────────────────────────

    #[test]
    fn aggregate_to_canonical_preserves_source_patch_confidence() {
        let result = aggregate_matches(&[ranked("m1", "14.11")]);
        let set = to_canonical_rows(&result, "euw1");
        assert!(!set.rates.is_empty());
        let row = &set.rates[0];
        assert_eq!(row.source, "riot_match_v5");
        assert_eq!(row.patch, "14.11");
        assert_eq!(row.region, "euw1");
        assert_eq!(row.confidence, "low"); // 1 game → low sample
        assert_eq!(row.ban_rate, 0.0); // Match-V5 simplified input: no bans
    }

    #[test]
    fn aram_match_is_skipped_in_canonical_rows() {
        let mut aram = ranked("aram1", "14.11");
        aram.queue_id = 450;
        let set = to_canonical_rows(&aggregate_matches(&[aram]), "euw1");
        assert!(set.rates.is_empty() && set.matchups.is_empty() && set.builds.is_empty());
    }

    #[test]
    fn patch_isolation_keeps_rows_separate() {
        let result = aggregate_matches(&[ranked("m1", "14.11"), ranked("m2", "14.10")]);
        let set = to_canonical_rows(&result, "euw1");
        let champ1: Vec<&CanonicalRateRow> = set
            .rates
            .iter()
            .filter(|r| r.champion_id == 1 && r.position == "top")
            .collect();
        // Same champ/pos but two patches → two distinct rows.
        assert_eq!(champ1.len(), 2);
        let patches: Vec<&str> = champ1.iter().map(|r| r.patch.as_str()).collect();
        assert!(patches.contains(&"14.11") && patches.contains(&"14.10"));
    }

    #[test]
    fn canonical_build_carries_items_and_joined_pick_rate() {
        let result = aggregate_matches(&[ranked("m1", "14.11")]);
        let set = to_canonical_rows(&result, "euw1");
        let b = set
            .builds
            .iter()
            .find(|b| b.champion_id == 1 && b.position == "top")
            .unwrap();
        assert!(b.item_ids.contains(&1001) && b.item_ids.contains(&3006));
        // pick_rate joined from the rate row (champ played 1 of role_total).
        let rate = set
            .rates
            .iter()
            .find(|r| r.champion_id == 1 && r.position == "top")
            .unwrap();
        assert!((b.pick_rate - rate.pick_rate).abs() < 1e-6);
    }

    #[test]
    fn emitted_tokens_stay_in_known_vocabulary() {
        // confidence / source / cache action tokens must come from fixed sets.
        let set = to_canonical_rows(&aggregate_matches(&[ranked("m1", "14.11")]), "euw1");
        for r in &set.rates {
            assert!(matches!(r.confidence.as_str(), "high" | "medium" | "low"));
            assert_eq!(r.source, "riot_match_v5");
        }
        for action in [
            decide_cache_promotion(&fresh_good(), None).action,
            decide_cache_promotion(&high_risk(), Some(&fresh_good())).action,
            decide_cache_promotion(&high_risk(), None).action,
        ] {
            assert!(matches!(
                action.as_str(),
                "promote" | "keep_current" | "reject"
            ));
        }
    }

    // ── Last-good cache policy ──────────────────────────────────────────────────

    fn q(risk: &str, coverage: f32, sample: u32, fresh: bool) -> CandidateQuality {
        CandidateQuality {
            source: "riot_match_v5".into(),
            risk_level: risk.into(),
            coverage_score: coverage,
            sample_size: sample,
            fresh,
        }
    }
    fn fresh_good() -> CandidateQuality {
        q("low", 0.9, 500, true)
    }
    fn high_risk() -> CandidateQuality {
        q("high", 0.9, 500, true)
    }

    #[test]
    fn first_good_candidate_is_promoted() {
        let d = decide_cache_promotion(&fresh_good(), None);
        assert_eq!(d.action, "promote");
        assert!(d.promoted);
    }

    #[test]
    fn better_or_equal_candidate_promotes_over_current() {
        let cur = q("low", 0.6, 300, true);
        let d = decide_cache_promotion(&fresh_good(), Some(&cur));
        assert_eq!(d.action, "promote");
    }

    #[test]
    fn high_risk_keeps_current_cache() {
        let d = decide_cache_promotion(&high_risk(), Some(&fresh_good()));
        assert_eq!(d.action, "keep_current");
        assert!(!d.promoted);
    }

    #[test]
    fn high_risk_without_cache_is_rejected() {
        let d = decide_cache_promotion(&high_risk(), None);
        assert_eq!(d.action, "reject");
    }

    #[test]
    fn coverage_regression_keeps_current() {
        let cur = q("low", 0.9, 500, true);
        let worse = q("low", 0.5, 100, true); // 0.4 below → beyond tolerance
        let d = decide_cache_promotion(&worse, Some(&cur));
        assert_eq!(d.action, "keep_current");
    }

    #[test]
    fn stale_candidate_keeps_fresh_cache() {
        let cur = q("low", 0.9, 500, true);
        let stale = q("low", 0.9, 500, false);
        let d = decide_cache_promotion(&stale, Some(&cur));
        assert_eq!(d.action, "keep_current");
    }

    #[test]
    fn empty_candidate_keeps_current_else_rejects() {
        let empty = q("low", 0.0, 0, true);
        assert_eq!(
            decide_cache_promotion(&empty, Some(&fresh_good())).action,
            "keep_current"
        );
        assert_eq!(decide_cache_promotion(&empty, None).action, "reject");
    }

    /// Drift guard: every cache `action` the policy can emit must have a
    /// `dataPipeline.cacheAction.*` label in tr.json (the Settings summary renders
    /// it). Drives every decision branch. en parity is the TS `i18n` test's.
    #[test]
    fn every_cache_action_has_an_i18n_label() {
        const TR: &str = include_str!("../../src/i18n/tr.json");
        let tr: serde_json::Value = serde_json::from_str(TR).expect("tr.json parses");
        let labels = &tr["dataPipeline"]["cacheAction"];

        let good = fresh_good();
        let worse = q("low", 0.5, 100, true);
        let stale = q("low", 0.9, 500, false);
        let empty = q("low", 0.0, 0, true);
        let decisions = [
            decide_cache_promotion(&good, None),               // promote
            decide_cache_promotion(&good, Some(&worse)),       // promote
            decide_cache_promotion(&high_risk(), Some(&good)), // keep_current
            decide_cache_promotion(&high_risk(), None),        // reject
            decide_cache_promotion(&worse, Some(&good)),       // keep_current (regression)
            decide_cache_promotion(&stale, Some(&good)),       // keep_current (stale)
            decide_cache_promotion(&empty, Some(&good)),       // keep_current
            decide_cache_promotion(&empty, None),              // reject
        ];
        for d in &decisions {
            assert!(
                !labels[&d.action].is_null(),
                "cache action '{}' has no dataPipeline.cacheAction label",
                d.action
            );
        }
    }
}
