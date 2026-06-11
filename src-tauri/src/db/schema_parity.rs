//! Ingestion / scheduler schema guards (Claude QA, test-only).
//!
//! Locks the columns the runtime upsert + scheduler observability need (V014
//! ingestion parity, V015 source_fetch_log). If a future migration change drops
//! one, this turns red — guarding the runtime contracts at the schema level.
//! Read-only on a freshly-migrated temp DB; never touches the upsert/scheduler path.

use crate::db::{open_db, run_migrations};
use tempfile::tempdir;

fn columns(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("table_info");
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1)) // col 1 = name
        .expect("query")
        .filter_map(Result::ok)
        .collect();
    names
}

#[test]
fn v014_ingestion_parity_columns_exist() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("parity.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let expected: &[(&str, &[&str])] = &[
        ("champion_rates", &["region"]),
        (
            "champion_matchups",
            &["region", "confidence", "sample_size"],
        ),
        ("builds", &["region", "games", "confidence"]),
    ];
    for (table, cols) in expected {
        let present = columns(&conn, table);
        for col in *cols {
            assert!(
                present.contains(&col.to_string()),
                "V014 parity column `{table}.{col}` is missing"
            );
        }
    }
}

#[test]
fn v015_source_fetch_log_columns_exist() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("fetchlog.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    // The scheduler observability surface reads these (status/decision + timing).
    let present = columns(&conn, "source_fetch_log");
    for col in [
        "source",
        "status",
        "decision",
        "message",
        "started_at",
        "finished_at",
        "duration_ms",
    ] {
        assert!(
            present.contains(&col.to_string()),
            "V015 source_fetch_log column `{col}` is missing"
        );
    }
}

#[test]
fn v016_match_v5_fetch_history_columns_exist() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("match_fetch_history.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let present = columns(&conn, "match_v5_fetch_history");
    for col in [
        "match_id",
        "region",
        "patch",
        "queue_id",
        "status",
        "discovered_at",
        "fetched_at",
        "processed_at",
        "error",
        "updated_at",
    ] {
        assert!(
            present.contains(&col.to_string()),
            "V016 match_v5_fetch_history column `{col}` is missing"
        );
    }
}

#[test]
fn v017_match_discovery_players_is_hash_only() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("match_discovery.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let present = columns(&conn, "match_discovery_players");
    for col in [
        "puuid_hash",
        "region",
        "source",
        "contribution_count",
        "first_seen_at",
        "last_seen_at",
        "last_crawled_at",
        "crawl_count",
        "updated_at",
    ] {
        assert!(
            present.contains(&col.to_string()),
            "V017 match_discovery_players column `{col}` is missing"
        );
    }
    assert!(
        !present.contains(&"puuid".to_string()),
        "V017 must not persist raw PUUID"
    );
}

/// Privacy keystone of the discovery feature: the persisted crawl table may only
/// hold `puuid_hash`. Locks the *negative space* against a future "helpful" migration
/// that adds `raw_puuid` / `summoner_name` / `riot_id` — the schema-level mirror of
/// `match_discovery_planner::output_exposes_only_hashes_no_raw_puuid`.
#[test]
fn v017_match_discovery_players_persists_no_raw_pii_columns() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("match_discovery_pii.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    // The one-way hash is the single allowed identifier; everything else must be
    // non-PII (region/source/counters/timestamps).
    const PII_TOKENS: &[&str] = &[
        "puuid",
        "summoner",
        "account_id",
        "riot_id",
        "game_name",
        "tag_line",
        "_name",
    ];
    for col in columns(&conn, "match_discovery_players") {
        let lower = col.to_ascii_lowercase();
        if lower == "puuid_hash" {
            continue;
        }
        for token in PII_TOKENS {
            assert!(
                !lower.contains(token),
                "V017 match_discovery_players must not persist raw PII \
                 (column `{col}` contains `{token}`)"
            );
        }
    }
}
