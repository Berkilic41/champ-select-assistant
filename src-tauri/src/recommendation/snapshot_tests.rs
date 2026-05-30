//! Snapshot tests for `compute_recommendations`.
//!
//! Each test loads a real LCU session fixture, runs the engine with an empty
//! mastery/stats/meta context, and asserts the ordered champion output.
//! Uses `insta` — on first run `cargo insta review` accepts the snapshots.
//! Floating-point scores are excluded from snapshots for cross-platform stability;
//! only the champion ranking order and tier assignments are captured.

#[cfg(test)]
mod snapshot_tests {
    use crate::db::champion_repo::ChampionRecord;
    use crate::db::mastery_repo::MasteryRow;
    use crate::ddragon::cdragon::{ItemData, RuneTree};
    use crate::lcu::session::parse_session;
    use crate::recommendation::draft_iq::DraftKnowledgeBase;
    use crate::recommendation::scoring::{MetaRate, ScoringContext, ScoringWeights};
    use crate::recommendation::{compute_recommendations, Recommendation};
    use serde::Serialize;
    use std::collections::HashMap;

    // ─── Champion pool ────────────────────────────────────────────────────────
    // A small but realistic pool covering all roles. Enough candidates so that
    // the exclusion set (banned/picked) still leaves meaningful options.

    fn champion_pool() -> Vec<ChampionRecord> {
        let raw: &[(i64, &str, &str)] = &[
            (86, "Garen", "Garen"),
            (157, "Yasuo", "Yasuo"),
            (238, "Zed", "Zed"),
            (64, "LeeSin", "Lee Sin"),
            (99, "Lux", "Lux"),
            (412, "Thresh", "Thresh"),
            (222, "Jinx", "Jinx"),
            (122, "Darius", "Darius"),
            (103, "Ahri", "Ahri"),
            (51, "Caitlyn", "Caitlyn"),
            (111, "Nautilus", "Nautilus"),
            (104, "Graves", "Graves"),
            (254, "Vi", "Vi"),
            (245, "Ekko", "Ekko"),
            (60, "Elise", "Elise"),
            (39, "Irelia", "Irelia"),
            (11, "MasterYi", "Master Yi"),
            (22, "Ashe", "Ashe"),
            (53, "Blitzcrank", "Blitzcrank"),
            (35, "Shaco", "Shaco"),
        ];
        raw.iter()
            .map(|(id, key, name)| ChampionRecord {
                champion_id: *id,
                key: key.to_string(),
                name: name.to_string(),
                title: String::new(),
            })
            .collect()
    }

    // ─── Snapshot-friendly summary ────────────────────────────────────────────

    #[derive(Debug, Serialize)]
    struct RecSnapshot {
        rank: usize,
        champion_key: String,
        tier: String,
        confidence: String,
    }

    fn to_snapshot(recs: &[Recommendation]) -> Vec<RecSnapshot> {
        recs.iter()
            .enumerate()
            .map(|(i, r)| RecSnapshot {
                rank: i + 1,
                champion_key: r.champion_key.clone(),
                tier: format!("{:?}", r.tier),
                confidence: r.confidence.clone(),
            })
            .collect()
    }

    // ─── Synthetic mastery ────────────────────────────────────────────────────
    // All pool champions get Level 5 mastery so comfort_score > 0.10 and they
    // clear the stretch-pick gate. Without mastery, every champion scores 0
    // comfort and is filtered out. Varying mastery allows realistic ranking.

    fn mastery_for_pool() -> Vec<MasteryRow> {
        let pool = champion_pool();
        pool.iter()
            .enumerate()
            .map(|(i, c)| MasteryRow {
                champion_id: c.champion_id,
                // Alternate between high/medium mastery to create variance
                level: if i % 3 == 0 {
                    7
                } else if i % 3 == 1 {
                    5
                } else {
                    3
                },
                points: if i % 3 == 0 {
                    150_000
                } else if i % 3 == 1 {
                    50_000
                } else {
                    10_000
                },
                last_play_time: None,
            })
            .collect()
    }

    // ─── Shared test helper ───────────────────────────────────────────────────

    fn run_engine(fixture_json: &str) -> Vec<Recommendation> {
        let v: serde_json::Value =
            serde_json::from_str(fixture_json).expect("fixture JSON geçersiz");
        let session = parse_session(&v).expect("fixture parse edilemedi");

        let role_map: HashMap<u32, Vec<String>> = HashMap::new();
        let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
        let mastery = mastery_for_pool();

        let ctx = ScoringContext {
            session: &session,
            mastery: &mastery,
            stats: &[],
            role_map: &role_map,
            weights: ScoringWeights::default(),
            meta_rates: &meta_rates,
            matchups: None,
            power_curves: None,
        };

        let champions = champion_pool();
        let items: Vec<ItemData> = vec![];
        let rune_trees: Vec<RuneTree> = vec![];
        let kb = DraftKnowledgeBase::load().expect("Draft IQ KB yüklenemedi");

        compute_recommendations(&ctx, &champions, &items, &rune_trees, &kb)
    }

    // ─── Fixtures ─────────────────────────────────────────────────────────────

    #[test]
    fn snapshot_ban_acting() {
        let recs = run_engine(include_str!("../../tests/fixtures/ban_acting.json"));
        let snap = to_snapshot(&recs);
        insta::assert_json_snapshot!("ban_acting", snap);
    }

    #[test]
    fn snapshot_pick_acting() {
        let recs = run_engine(include_str!("../../tests/fixtures/pick_acting.json"));
        let snap = to_snapshot(&recs);
        insta::assert_json_snapshot!("pick_acting", snap);
    }

    #[test]
    fn snapshot_pick_watching() {
        let recs = run_engine(include_str!("../../tests/fixtures/pick_watching.json"));
        let snap = to_snapshot(&recs);
        insta::assert_json_snapshot!("pick_watching", snap);
    }

    #[test]
    fn snapshot_finalization() {
        let recs = run_engine(include_str!("../../tests/fixtures/finalization.json"));
        let snap = to_snapshot(&recs);
        insta::assert_json_snapshot!("finalization", snap);
    }

    #[test]
    fn snapshot_aram_pick() {
        let recs = run_engine(include_str!("../../tests/fixtures/aram_pick.json"));
        let snap = to_snapshot(&recs);
        insta::assert_json_snapshot!("aram_pick", snap);
    }
}
