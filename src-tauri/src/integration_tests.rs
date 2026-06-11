//! Cross-module integration tests (Faz D).
//!
//! These exercise real multi-module flows that the per-module unit tests don't:
//!   1. the multi-source aggressive-data path through a **real SQLite DB** —
//!      per-source rate rows persist independently, then blend into one
//!      cross-validated `MetaRate`;
//!   2. real LCU `/lol-champ-select/v1/session` fixtures through the public
//!      `parse_session` contract.
//!
//! In-crate (not under `tests/`) because the app's modules are private to the lib,
//! so an external test crate couldn't reach them.
#![cfg(test)]

use crate::db::champion_rates_repo::{self, ChampionRateRow};
use crate::db::{open_db, run_migrations};
use crate::lcu::parse_session;
use crate::recommendation::rate_blend::{blend_rates, SourceRate};
use tempfile::tempdir;

fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("fixture {name} parse: {e}"))
}

fn rate(source: &str, win_rate: f32, sample: u32) -> ChampionRateRow {
    ChampionRateRow {
        champion_id: 64,
        position: "jungle".to_string(),
        win_rate,
        pick_rate: 0.1,
        ban_rate: if source == "meraki" { 0.08 } else { 0.0 },
        sample_size: sample,
        patch: "16.11".to_string(),
        source: source.to_string(),
        confidence: "high".to_string(),
    }
}

#[test]
fn multi_source_rates_persist_per_source_and_blend() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("pipeline.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    // Two independent sources agree on Lee Sin jungle.
    for r in [rate("meraki", 0.52, 5000), rate("u_gg", 0.53, 9000)] {
        champion_rates_repo::upsert_rate_with_region(&conn, &r, "all").unwrap();
    }

    // Both rows coexist (UNIQUE includes `source`) — the whole point of multi-source.
    let rows = champion_rates_repo::get_all_for_position(&conn, "jungle").unwrap();
    assert_eq!(rows.len(), 2, "per-source rows persist independently");

    let blended = blend_rates(
        rows.into_iter()
            .map(|r| SourceRate {
                champion_id: r.champion_id,
                position: r.position,
                win_rate: r.win_rate,
                ban_rate: r.ban_rate,
                sample_size: r.sample_size,
            })
            .collect(),
    );
    let m = &blended[&(64, "jungle".to_string())];
    assert_eq!(m.sample_size, 14_000, "agreeing sources combine evidence");
    let expected_wr = (0.52 * 5000.0 + 0.53 * 9000.0) / 14_000.0;
    assert!((m.win_rate - expected_wr).abs() < 1e-4);
    assert!(
        (m.ban_rate - 0.08).abs() < 1e-5,
        "ban-rate comes from Meraki only (u.gg's 0 is 'unknown')"
    );
}

#[test]
fn upsert_is_idempotent_per_source() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("idem.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    champion_rates_repo::upsert_rate_with_region(&conn, &rate("u_gg", 0.50, 100), "all").unwrap();
    // Re-upsert the same (champion, position, source) with a new win-rate.
    champion_rates_repo::upsert_rate_with_region(&conn, &rate("u_gg", 0.99, 200), "all").unwrap();

    let rows = champion_rates_repo::get_all_for_position(&conn, "jungle").unwrap();
    assert_eq!(rows.len(), 1, "ON CONFLICT replaces, never duplicates");
    assert!((rows[0].win_rate - 0.99).abs() < 1e-5);
    assert_eq!(rows[0].sample_size, 200);
}

#[test]
fn pick_acting_fixture_yields_a_populated_session() {
    let state = parse_session(&fixture("pick_acting.json")).expect("pick_acting → Some");
    assert!(
        !state.my_team.is_empty() && !state.their_team.is_empty(),
        "both teams populated"
    );
    assert!(!state.phase.is_empty(), "phase set");
}

#[test]
fn all_real_session_fixtures_parse_without_panic() {
    for name in [
        "pick_acting.json",
        "pick_watching.json",
        "blind_pick.json",
        "aram_pick.json",
        "ban_acting.json",
        "finalization.json",
    ] {
        // Must never panic on a real LCU shape; Some/None are both valid per phase.
        let _ = parse_session(&fixture(name));
    }
}

#[test]
fn aram_fixture_carries_the_aram_queue_id() {
    if let Some(state) = parse_session(&fixture("aram_pick.json")) {
        assert_eq!(
            state.queue_id, 450,
            "ARAM queue id drives the scoring profile"
        );
    }
}
