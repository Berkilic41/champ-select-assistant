//! u.gg aggregate-stats source.
//!
//! u.gg pre-computes champion stats and serves them as **static JSON on an open
//! CDN** (`stats2.u.gg`) — no Cloudflare browser gate — which makes it the most
//! reliable reqwest-friendly rich source we have. We read the per-champion
//! `overview` file, which yields, per role:
//!   - rate: `wins / matches` (+ matches as sample size),
//!   - build: core items, rune perks, summoner spells.
//!
//! u.gg encodes everything as positional/nested arrays keyed by
//! `region → rank → role`. We read region `12` (world) and rank `8` (overall —
//! by far the largest sample, so the most statistically robust win-rates). Off-
//! role noise rows are dropped below `MIN_ROLE_MATCHES`.
//!
//! u.gg also serves a per-champion **matchups** file on the same open CDN. Each
//! opponent row decodes as `[opponentId, wins, games, …deltas]`, so
//! `win_rate = wins / games` — complete, huge-sample lane-matchup coverage (our
//! thinnest data) without any new source or headless browser. Rows below
//! `MATCHUP_GAMES_FLOOR` are dropped as noise. A blocked/unreachable fetch returns
//! `Err`; the other sources carry coverage.
#![allow(dead_code)]

use crate::meta::source::FetchCtx;
use crate::recommendation::ingestion_contract::{
    CanonicalBuildRow, CanonicalMatchupRow, CanonicalRateRow, CanonicalRowSet,
};
use anyhow::{anyhow, Result};
use futures_util::stream::{self, StreamExt};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

const STATS_BASE: &str = "https://stats2.u.gg/lol/1.5";
/// u.gg internal data-format version embedded in the path. Bump if u.gg rotates it
/// (a wrong version 404s → source returns `Err` → graceful fallback).
const DATA_VERSION: &str = "1.5.0";
const QUEUE: &str = "ranked_solo_5x5";
/// Region `12` = world (aggregate of all regions).
const REGION_WORLD: &str = "12";
/// Rank `8` = "overall" (all tiers) — the largest sample, most robust win-rates.
const RANK_OVERALL: &str = "8";
const HTTP_TIMEOUT_SECS: u64 = 15;
/// Be polite to the CDN: cap concurrent champion fetches.
const CONCURRENCY: usize = 6;
/// Drop off-role noise (e.g. Lee Sin "support" with a handful of games).
const MIN_ROLE_MATCHES: u64 = 200;
/// Drop low-sample matchup pairs (keep only confident lane matchups).
const MATCHUP_GAMES_FLOOR: u64 = 500;
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120";

/// u.gg role id → LCU position. (1=jungle, 2=support→utility, 3=bottom, 4=top, 5=mid.)
const UGG_ROLES: [(&str, &str); 5] = [
    ("1", "jungle"),
    ("2", "utility"),
    ("3", "bottom"),
    ("4", "top"),
    ("5", "middle"),
];

fn confidence_from_matches(n: u64) -> &'static str {
    if n < 200 {
        "low"
    } else if n <= 2000 {
        "medium"
    } else {
        "high"
    }
}

/// DDragon-style patch ("16.11" / "16.11.1") → u.gg path patch ("16_11").
fn to_ugg_patch(ddragon_patch: &str) -> String {
    let mut parts = ddragon_patch.split('.');
    let major = parts.next().unwrap_or("");
    let minor = parts.next().unwrap_or("");
    format!("{major}_{minor}")
}

/// Pull an array of `u32` ids out of a JSON array value, skipping non-numbers.
fn id_list(v: Option<&Value>) -> Vec<u32> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_u64().map(|n| n as u32))
                .collect()
        })
        .unwrap_or_default()
}

fn build_client() -> Result<Client> {
    Ok(Client::builder()
        .use_rustls_tls()
        .user_agent(UA)
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?)
}

fn overview_url(ugg_patch: &str, champion_id: u32) -> String {
    format!("{STATS_BASE}/overview/{ugg_patch}/{QUEUE}/{champion_id}/{DATA_VERSION}.json")
}

/// Pure decode of one champion's `overview` JSON → canonical rate + build rows.
///
/// `region_tag`/`patch` are the values STORED on the rows (DDragon-style patch,
/// e.g. "16.11"); the u.gg path patch is a separate concern. Emits one row per
/// role whose sample clears `MIN_ROLE_MATCHES`. No matchups (u.gg overview has no
/// decodable per-opponent win-rate).
fn parse_overview(
    v: &Value,
    champion_id: u32,
    patch: &str,
    region_tag: &str,
) -> (Vec<CanonicalRateRow>, Vec<CanonicalBuildRow>) {
    let mut rates = Vec::new();
    let mut builds = Vec::new();

    let Some(rank_node) = v.get(REGION_WORLD).and_then(|r| r.get(RANK_OVERALL)) else {
        return (rates, builds);
    };

    for (role_key, position) in UGG_ROLES {
        // role node = [dataArray, timestamp]; dataArray is the 13-element block.
        let Some(d) = rank_node.get(role_key).and_then(|n| n.get(0)) else {
            continue;
        };

        // d[6] = [wins, matches].
        let wins = d.get(6).and_then(|x| x.get(0)).and_then(Value::as_u64);
        let matches = d.get(6).and_then(|x| x.get(1)).and_then(Value::as_u64);
        let (Some(wins), Some(matches)) = (wins, matches) else {
            continue;
        };
        if matches < MIN_ROLE_MATCHES || matches == 0 {
            continue;
        }
        let win_rate = wins as f32 / matches as f32;
        let sample = matches as u32;
        let confidence = confidence_from_matches(matches).to_string();

        rates.push(CanonicalRateRow {
            region: region_tag.to_string(),
            patch: patch.to_string(),
            champion_id,
            position: position.to_string(),
            win_rate,
            // u.gg overview gives no cross-champion pick/ban rate in a decodable
            // slot — leave honest 0 (Meraki/Match-V5 carry pick/ban).
            pick_rate: 0.0,
            ban_rate: 0.0,
            sample_size: sample,
            source: "u_gg".to_string(),
            confidence: confidence.clone(),
        });

        // Build: core items d[3][2], rune perks d[0][4], summoner spells d[1][2].
        let item_ids = id_list(d.get(3).and_then(|x| x.get(2)));
        let rune_ids = id_list(d.get(0).and_then(|x| x.get(4)));
        let summoner_spells = id_list(d.get(1).and_then(|x| x.get(2)));
        if !item_ids.is_empty() || !rune_ids.is_empty() {
            builds.push(CanonicalBuildRow {
                region: region_tag.to_string(),
                patch: patch.to_string(),
                champion_id,
                position: position.to_string(),
                item_ids,
                rune_ids,
                summoner_spells,
                games: sample,
                win_rate,
                pick_rate: 0.0,
                sample_size: sample,
                source: "u_gg".to_string(),
                confidence,
            });
        }
    }

    (rates, builds)
}

async fn fetch_overview(client: &Client, ugg_patch: &str, champion_id: u32) -> Result<Value> {
    let resp = client
        .get(overview_url(ugg_patch, champion_id))
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

fn matchups_url(ugg_patch: &str, champion_id: u32) -> String {
    format!("{STATS_BASE}/matchups/{ugg_patch}/{QUEUE}/{champion_id}/{DATA_VERSION}.json")
}

async fn fetch_matchups(client: &Client, ugg_patch: &str, champion_id: u32) -> Result<Value> {
    let resp = client
        .get(matchups_url(ugg_patch, champion_id))
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

/// Pure decode of one champion's `matchups` JSON → canonical matchup rows. Each
/// opponent entry is `[opponentId, wins, games, …deltas]`, so
/// `win_rate = wins / games`. Emits per role present (region 12 / rank 8), one row
/// per opponent clearing `MATCHUP_GAMES_FLOOR`.
fn parse_matchups(
    v: &Value,
    champion_id: u32,
    patch: &str,
    region_tag: &str,
) -> Vec<CanonicalMatchupRow> {
    let mut out = Vec::new();
    let Some(rank_node) = v.get(REGION_WORLD).and_then(|r| r.get(RANK_OVERALL)) else {
        return out;
    };

    for (role_key, position) in UGG_ROLES {
        // role node = [matchupArray, timestamp]; matchupArray is per-opponent rows.
        let Some(rows) = rank_node
            .get(role_key)
            .and_then(|n| n.get(0))
            .and_then(Value::as_array)
        else {
            continue;
        };

        for row in rows {
            let opponent_id = row.get(0).and_then(Value::as_u64);
            let wins = row.get(1).and_then(Value::as_u64);
            let games = row.get(2).and_then(Value::as_u64);
            let (Some(opponent_id), Some(wins), Some(games)) = (opponent_id, wins, games) else {
                continue;
            };
            if games < MATCHUP_GAMES_FLOOR || games == 0 || opponent_id == 0 {
                continue;
            }
            let win_rate = wins as f32 / games as f32;
            out.push(CanonicalMatchupRow {
                region: region_tag.to_string(),
                patch: patch.to_string(),
                champion_id,
                opponent_id: opponent_id as u32,
                position: position.to_string(),
                games: games as u32,
                wins: wins as u32,
                win_rate,
                sample_size: games as u32,
                source: "u_gg".to_string(),
                confidence: confidence_from_matches(games).to_string(),
            });
        }
    }

    out
}

/// u.gg may lag the live patch by one or two; probe a seed champion across the
/// current patch and the two before it, and use the first that serves data.
async fn resolve_working_patch(client: &Client, ctx: &FetchCtx) -> Result<String> {
    let seed = ctx
        .champions
        .first()
        .ok_or_else(|| anyhow!("u.gg: boş şampiyon listesi"))?;
    let (major, minor) = {
        let mut p = ctx.patch.split('.');
        let major: i64 = p.next().unwrap_or("").parse().unwrap_or(0);
        let minor: i64 = p.next().unwrap_or("").parse().unwrap_or(0);
        (major, minor)
    };
    for back in 0..3 {
        if minor - back < 0 {
            break;
        }
        let candidate = format!("{}_{}", major, minor - back);
        if fetch_overview(client, &candidate, seed.champion_id as u32)
            .await
            .is_ok()
        {
            return Ok(candidate);
        }
    }
    // Last resort: the naive conversion (lets the per-champion 404s drop quietly).
    Err(anyhow!(
        "u.gg: '{}' patch'i ve 2 öncesi için veri yok (CDN güncel değil veya engelli)",
        to_ugg_patch(&ctx.patch)
    ))
}

pub async fn fetch(ctx: &FetchCtx) -> Result<CanonicalRowSet> {
    let client = build_client()?;
    let ugg_patch = resolve_working_patch(&client, ctx).await?;
    let patch = ctx.patch.clone();
    let region_tag = ctx.region.clone();

    type ChampRows = (
        Vec<CanonicalRateRow>,
        Vec<CanonicalBuildRow>,
        Vec<CanonicalMatchupRow>,
    );
    let parsed: Vec<Option<ChampRows>> = stream::iter(ctx.champions.iter().cloned())
        .map(|champ| {
            let client = client.clone();
            let ugg_patch = ugg_patch.clone();
            let patch = patch.clone();
            let region_tag = region_tag.clone();
            async move {
                let id = champ.champion_id as u32;
                // Overview (rates + builds) is required; matchups are best-effort.
                let Ok(ov) = fetch_overview(&client, &ugg_patch, id).await else {
                    return None; // champ missing on this patch → skip, others carry
                };
                let (rates, builds) = parse_overview(&ov, id, &patch, &region_tag);
                let matchups = match fetch_matchups(&client, &ugg_patch, id).await {
                    Ok(mu) => parse_matchups(&mu, id, &patch, &region_tag),
                    Err(_) => Vec::new(),
                };
                Some((rates, builds, matchups))
            }
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    let mut rates = Vec::new();
    let mut builds = Vec::new();
    let mut matchups = Vec::new();
    let mut ok = 0usize;
    for entry in parsed.into_iter().flatten() {
        ok += 1;
        rates.extend(entry.0);
        builds.extend(entry.1);
        matchups.extend(entry.2);
    }
    if ok == 0 {
        return Err(anyhow!("u.gg: hiçbir şampiyon verisi alınamadı"));
    }

    Ok(CanonicalRowSet {
        region: region_tag,
        rates,
        matchups,
        builds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed real-shape sample: region 12 → rank 8 → role 1 (jungle).
    const OVERVIEW_FIXTURE: &str = r#"{
      "12": { "8": { "1": [[
        [26,17,8000,8300,[8010,8299,8304,8347,9104,9111]],
        [9801,5080,[4,11]],
        [63,40,[1102,2003]],
        [61,39,[6692,6610,3047]],
        [285,157,["E","Q","W"],"QWE"],
        [[[6333,85,153]],[],[],[],[],[]],
        [5081,9803],
        false,
        [101,58,["5005","5008","5011"]],
        [],[],[],[828128.16]
      ],"2026-06-07T12:06:15Z"] } },
      "1": { "8": { "1": [[],""] } }
    }"#;

    #[test]
    fn to_ugg_patch_maps_major_minor() {
        assert_eq!(to_ugg_patch("16.11"), "16_11");
        assert_eq!(to_ugg_patch("16.11.1"), "16_11");
        assert_eq!(to_ugg_patch("9.1"), "9_1");
    }

    #[test]
    fn parse_overview_decodes_rate_and_build() {
        let v: Value = serde_json::from_str(OVERVIEW_FIXTURE).unwrap();
        let (rates, builds) = parse_overview(&v, 64, "16.11", "all");

        assert_eq!(rates.len(), 1);
        let r = &rates[0];
        assert_eq!(r.champion_id, 64);
        assert_eq!(r.position, "jungle");
        assert_eq!(r.source, "u_gg");
        assert_eq!(r.sample_size, 9803);
        assert!((r.win_rate - 5081.0 / 9803.0).abs() < 1e-6);
        assert_eq!(r.confidence, "high"); // 9803 > 2000
        assert_eq!(r.patch, "16.11");
        assert_eq!(r.region, "all");
        assert_eq!(r.ban_rate, 0.0);

        assert_eq!(builds.len(), 1);
        let b = &builds[0];
        assert_eq!(b.item_ids, vec![6692, 6610, 3047]); // core items d[3][2]
        assert_eq!(b.rune_ids, vec![8010, 8299, 8304, 8347, 9104, 9111]); // perks d[0][4]
        assert_eq!(b.summoner_spells, vec![4, 11]); // spells d[1][2]
        assert_eq!(b.games, 9803);
        assert_eq!(b.source, "u_gg");
    }

    // Real-shape matchups: region 12 → rank 8 → role 1; rows = [oppId, wins, games, …deltas].
    const MATCHUPS_FIXTURE: &str = r#"{
      "12": { "8": { "1": [[
        [104, 28682, 56764, 6413418, 1668825],
        [234, 22438, 44539, 6151228, -173641],
        [99, 5, 9, 0, 0]
      ],"2026-06-07T12:06:15Z"] } }
    }"#;

    #[test]
    fn parse_matchups_decodes_winrate_and_drops_low_sample() {
        let v: Value = serde_json::from_str(MATCHUPS_FIXTURE).unwrap();
        let rows = parse_matchups(&v, 64, "16.11", "all");
        // opp 99 has 9 games < floor → dropped; 104 + 234 kept.
        assert_eq!(rows.len(), 2);
        let warwick = rows.iter().find(|r| r.opponent_id == 104).unwrap();
        assert_eq!(warwick.champion_id, 64);
        assert_eq!(warwick.position, "jungle");
        assert_eq!(warwick.games, 56764);
        assert_eq!(warwick.wins, 28682);
        assert!((warwick.win_rate - 28682.0 / 56764.0).abs() < 1e-6);
        assert_eq!(warwick.source, "u_gg");
        assert_eq!(warwick.confidence, "high");
        assert!(
            rows.iter().all(|r| r.opponent_id != 99),
            "low-sample dropped"
        );
    }

    #[test]
    fn parse_overview_drops_low_sample_and_missing_region() {
        // matches below MIN_ROLE_MATCHES → dropped.
        let low = r#"{"12":{"8":{"1":[[
            [26,17,8000,8300,[8010]],[10,5,[4,11]],[1,1,[1102]],[1,1,[6692]],
            [1,1,["Q"],"Q"],[[],[],[],[],[],[]],[5,10],false,[1,1,["5005"]],[],[],[],[0]
        ],"t"]}}}"#;
        let v: Value = serde_json::from_str(low).unwrap();
        let (rates, builds) = parse_overview(&v, 1, "16.11", "all");
        assert!(
            rates.is_empty() && builds.is_empty(),
            "10 matches < 200 floor"
        );

        // region 12 absent → nothing, no panic.
        let none: Value = serde_json::from_str(r#"{"1":{"8":{"1":[[],""]}}}"#).unwrap();
        let (rates, builds) = parse_overview(&none, 1, "16.11", "all");
        assert!(rates.is_empty() && builds.is_empty());
    }
}
