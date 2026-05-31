//! Meraki Analytics champion-rates fetcher.
//!
//! Pulls the patch-level win/pick/ban table published at
//! `https://cdn.merakianalytics.com/riot/lol/resources/latest/en-US/championrates.json`
//! and upserts each `(champion_id, position)` row into `champion_rates` via
//! [`crate::db::champion_rates_repo::upsert_rate`].
//!
//! Meraki publishes rates as floats in `[0, 1]` and sometimes provides a per-row
//! sample size in the optional `count` field — when present this drives the
//! confidence band; otherwise we fall back to `"low"`.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

use crate::db::champion_rates_repo::ChampionRateRow;
use crate::db::champion_repo::ChampionRecord;

pub const MERAKI_URL: &str =
    "https://cdn.merakianalytics.com/riot/lol/resources/latest/en-US/championrates.json";

const SOURCE: &str = "meraki";
const HTTP_TIMEOUT_SECS: u64 = 15;

// ---------------------------------------------------------------------------
// Serde structs — mirror Meraki's JSON shape
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerakiPositionStats {
    // Meraki sometimes ships position entries with only `playRate` (and 0 at that)
    // for unplayed lanes; the win/ban fields are then absent. Default-on-missing so
    // the whole payload still parses, and skip such empty rows in `build_rate_rows`.
    #[serde(default)]
    pub play_rate: f64,
    #[serde(default)]
    pub win_rate: f64,
    #[serde(default)]
    pub ban_rate: f64,
    #[serde(default)]
    pub count: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MerakiResponse {
    pub patch: String,
    pub data: HashMap<String, HashMap<String, MerakiPositionStats>>,
}

// ---------------------------------------------------------------------------
// Position normalisation
// ---------------------------------------------------------------------------

/// Map Meraki's `TOP`/`JUNGLE`/`MIDDLE`/`BOTTOM`/`SUPPORT` to the lowercase
/// strings used by the LCU (`top`, `jungle`, `middle`, `bottom`, `utility`).
///
/// Note: Meraki labels the support role `SUPPORT`, the LCU labels it `utility`.
/// Returns `None` for unknown values so the caller can skip them.
fn normalize_position(meraki_pos: &str) -> Option<&'static str> {
    match meraki_pos.to_uppercase().as_str() {
        "TOP" => Some("top"),
        "JUNGLE" => Some("jungle"),
        "MIDDLE" => Some("middle"),
        "BOTTOM" => Some("bottom"),
        "SUPPORT" => Some("utility"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Confidence heuristic
// ---------------------------------------------------------------------------

/// Map the optional sample-size (`count`) to a confidence band:
///   * `None`         -> `"low"`
///   * `0..200`       -> `"low"`
///   * `200..=2000`   -> `"medium"`
///   * `> 2000`       -> `"high"`
fn confidence_from_count(count: Option<u64>) -> &'static str {
    match count {
        None => "low",
        Some(n) if n < 200 => "low",
        Some(n) if n <= 2000 => "medium",
        Some(_) => "high",
    }
}

// ---------------------------------------------------------------------------
// HTTP fetch + transform pipeline
// ---------------------------------------------------------------------------

/// Download the Meraki JSON via `rustls-tls`. Network and parsing errors
/// bubble up; callers decide whether to fall back to cached rows.
pub async fn fetch_meraki_rates() -> Result<MerakiResponse> {
    let client = Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .context("reqwest client build failed")?;

    let resp = client
        .get(MERAKI_URL)
        .send()
        .await
        .context("Meraki HTTP request failed")?
        .error_for_status()
        .context("Meraki HTTP status not OK")?;

    let body: MerakiResponse = resp.json().await.context("Meraki JSON parse failed")?;
    Ok(body)
}

/// Pure transformation: parsed Meraki payload + local champion list -> rows.
///
/// Champions present in the payload but missing from the local `champions`
/// table are silently dropped (DDragon may be lagging behind). Positions that
/// don't normalise to a known LCU value are likewise skipped.
pub fn build_rate_rows(
    meraki: &MerakiResponse,
    all_champions: &[ChampionRecord],
) -> Vec<ChampionRateRow> {
    let mut out = Vec::with_capacity(meraki.data.len() * 2);

    for (champ_key, positions) in &meraki.data {
        // Meraki keys its `data` map by champion ID ("266") in current exports, but
        // older ones used the DDragon key ("Aatrox"). Match either form.
        let Some(record) = champ_key
            .parse::<i64>()
            .ok()
            .and_then(|id| all_champions.iter().find(|c| c.champion_id == id))
            .or_else(|| all_champions.iter().find(|c| c.key == *champ_key))
        else {
            continue;
        };

        for (raw_pos, stats) in positions {
            let Some(position) = normalize_position(raw_pos) else {
                continue;
            };
            // Skip entries with no usable signal (Meraki ships {playRate:0} stubs
            // for unplayed lanes). Storing 0% win-rate would poison the meta score.
            if stats.win_rate <= 0.0 {
                continue;
            }
            let confidence = confidence_from_count(stats.count);

            out.push(ChampionRateRow {
                champion_id: record.champion_id as u32,
                position: position.to_string(),
                win_rate: stats.win_rate as f32,
                pick_rate: stats.play_rate as f32,
                ban_rate: stats.ban_rate as f32,
                sample_size: stats.count.unwrap_or(0) as u32,
                patch: meraki.patch.clone(),
                source: SOURCE.to_string(),
                confidence: confidence.to_string(),
            });
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests — pure-function only (no live HTTP).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_champions() -> Vec<ChampionRecord> {
        vec![
            ChampionRecord {
                champion_id: 266,
                key: "Aatrox".into(),
                name: "Aatrox".into(),
                title: "the Darkin Blade".into(),
            },
            ChampionRecord {
                champion_id: 157,
                key: "Yasuo".into(),
                name: "Yasuo".into(),
                title: "the Unforgiven".into(),
            },
        ]
    }

    fn sample_meraki() -> MerakiResponse {
        let mut data = HashMap::new();

        let mut aatrox = HashMap::new();
        aatrox.insert(
            "TOP".to_string(),
            MerakiPositionStats {
                play_rate: 0.083,
                win_rate: 0.512,
                ban_rate: 0.031,
                count: Some(5_000),
            },
        );
        aatrox.insert(
            "MIDDLE".to_string(),
            MerakiPositionStats {
                play_rate: 0.005,
                win_rate: 0.490,
                ban_rate: 0.010,
                count: Some(150),
            },
        );
        data.insert("Aatrox".to_string(), aatrox);

        let mut yasuo = HashMap::new();
        yasuo.insert(
            "MIDDLE".to_string(),
            MerakiPositionStats {
                play_rate: 0.043,
                win_rate: 0.498,
                ban_rate: 0.120,
                count: Some(1_000),
            },
        );
        // SUPPORT must normalise to "utility"; sample_size absent -> "low" confidence
        yasuo.insert(
            "SUPPORT".to_string(),
            MerakiPositionStats {
                play_rate: 0.002,
                win_rate: 0.450,
                ban_rate: 0.001,
                count: None,
            },
        );
        data.insert("Yasuo".to_string(), yasuo);

        // Unknown champion + unknown position must both be skipped
        let mut unknown_champ = HashMap::new();
        unknown_champ.insert(
            "TOP".to_string(),
            MerakiPositionStats {
                play_rate: 0.02,
                win_rate: 0.50,
                ban_rate: 0.0,
                count: Some(300),
            },
        );
        data.insert("FutureChampion".to_string(), unknown_champ);

        let mut aatrox_bad_pos = HashMap::new();
        aatrox_bad_pos.insert(
            "ARENA".to_string(),
            MerakiPositionStats {
                play_rate: 0.02,
                win_rate: 0.50,
                ban_rate: 0.0,
                count: Some(300),
            },
        );
        // Inject the bad position into existing Aatrox entry
        data.get_mut("Aatrox").unwrap().insert(
            "ARENA".to_string(),
            MerakiPositionStats {
                play_rate: 0.02,
                win_rate: 0.50,
                ban_rate: 0.0,
                count: Some(300),
            },
        );

        MerakiResponse {
            patch: "14.10".to_string(),
            data,
        }
    }

    #[test]
    fn normalize_position_known() {
        assert_eq!(normalize_position("TOP"), Some("top"));
        assert_eq!(normalize_position("JUNGLE"), Some("jungle"));
        assert_eq!(normalize_position("MIDDLE"), Some("middle"));
        assert_eq!(normalize_position("BOTTOM"), Some("bottom"));
        assert_eq!(normalize_position("SUPPORT"), Some("utility"));
        // Case-insensitive
        assert_eq!(normalize_position("support"), Some("utility"));
    }

    #[test]
    fn normalize_position_unknown_returns_none() {
        assert_eq!(normalize_position("ARENA"), None);
        assert_eq!(normalize_position("UTILITY"), None);
        assert_eq!(normalize_position(""), None);
    }

    #[test]
    fn confidence_thresholds() {
        assert_eq!(confidence_from_count(None), "low");
        assert_eq!(confidence_from_count(Some(0)), "low");
        assert_eq!(confidence_from_count(Some(199)), "low");
        assert_eq!(confidence_from_count(Some(200)), "medium");
        assert_eq!(confidence_from_count(Some(1_500)), "medium");
        assert_eq!(confidence_from_count(Some(2_000)), "medium");
        assert_eq!(confidence_from_count(Some(2_001)), "high");
        assert_eq!(confidence_from_count(Some(50_000)), "high");
    }

    #[test]
    fn build_rate_rows_skips_unknown_champion_and_position() {
        let meraki = sample_meraki();
        let champs = sample_champions();
        let rows = build_rate_rows(&meraki, &champs);

        // Aatrox: TOP + MIDDLE (ARENA dropped) = 2
        // Yasuo:  MIDDLE + SUPPORT (->utility)  = 2
        // FutureChampion -> dropped
        assert_eq!(
            rows.len(),
            4,
            "Unknown champion key and unknown position must be skipped"
        );
        assert!(rows.iter().all(|r| r.source == "meraki"));
        assert!(rows.iter().all(|r| r.patch == "14.10"));
        assert!(
            rows.iter().any(|r| r.position == "utility"),
            "SUPPORT must be normalised to 'utility'"
        );
        assert!(
            rows.iter().all(|r| r.position != "ARENA"),
            "Unknown positions must be skipped, never emitted verbatim"
        );
    }

    #[test]
    fn build_rate_rows_maps_confidence_and_fields() {
        let meraki = sample_meraki();
        let champs = sample_champions();
        let rows = build_rate_rows(&meraki, &champs);

        let aatrox_top = rows
            .iter()
            .find(|r| r.champion_id == 266 && r.position == "top")
            .expect("Aatrox top row must exist");
        assert_eq!(aatrox_top.confidence, "high");
        assert!((aatrox_top.win_rate - 0.512).abs() < 1e-5);
        assert!((aatrox_top.pick_rate - 0.083).abs() < 1e-5);
        assert!((aatrox_top.ban_rate - 0.031).abs() < 1e-5);
        assert_eq!(aatrox_top.sample_size, 5_000);

        let aatrox_mid = rows
            .iter()
            .find(|r| r.champion_id == 266 && r.position == "middle")
            .expect("Aatrox middle row must exist");
        assert_eq!(
            aatrox_mid.confidence, "low",
            "sample_size=150 < 200 must yield 'low'"
        );

        let yasuo_mid = rows
            .iter()
            .find(|r| r.champion_id == 157 && r.position == "middle")
            .expect("Yasuo middle row must exist");
        assert_eq!(yasuo_mid.confidence, "medium");

        let yasuo_util = rows
            .iter()
            .find(|r| r.champion_id == 157 && r.position == "utility")
            .expect("Yasuo utility row must exist");
        assert_eq!(yasuo_util.confidence, "low", "count=None must yield 'low'");
        assert_eq!(yasuo_util.sample_size, 0);
    }

    #[test]
    fn upsert_and_read_back_via_repo() {
        use crate::db::{champion_rates_repo, open_db, run_migrations};
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let mut conn = open_db(&dir.path().join("meraki.db")).unwrap();
        run_migrations(&mut conn).unwrap();

        let meraki = sample_meraki();
        let champs = sample_champions();
        let rows = build_rate_rows(&meraki, &champs);
        for row in &rows {
            champion_rates_repo::upsert_rate(&conn, row).unwrap();
        }

        // Bulk lookup by position
        let mid_rows = champion_rates_repo::get_all_for_position(&conn, "middle").unwrap();
        assert_eq!(
            mid_rows.len(),
            2,
            "Aatrox + Yasuo middle rows must round-trip"
        );
        assert!(mid_rows.iter().all(|r| r.source == "meraki"));

        // Re-upsert with bumped win_rate to verify ON CONFLICT path
        let mut bumped = rows[0].clone();
        let key = (
            bumped.champion_id,
            bumped.position.clone(),
            bumped.source.clone(),
        );
        bumped.win_rate = 0.999;
        champion_rates_repo::upsert_rate(&conn, &bumped).unwrap();

        // Confirm only one row exists for (champion, position, source) after re-upsert
        let same_pos = champion_rates_repo::get_all_for_position(&conn, &key.1).unwrap();
        let matches: Vec<_> = same_pos
            .iter()
            .filter(|r| r.champion_id == key.0 && r.source == key.2)
            .collect();
        assert_eq!(matches.len(), 1, "Upsert must not duplicate the row");
        assert!(
            (matches[0].win_rate - 0.999).abs() < 1e-5,
            "Upsert must replace win_rate"
        );
    }
}
