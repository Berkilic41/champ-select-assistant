use anyhow::Result;
use serde::Deserialize;

/// One entry from CDragon's champion-summary.json
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdragonChampion {
    pub id: i32,
    #[allow(dead_code)]
    pub alias: String, // champion key (e.g. "Aatrox") — for future key→id mapping
    pub roles: Vec<String>,
}

// ---------------------------------------------------------------------------
// Item data
// ---------------------------------------------------------------------------

// ItemData is now a shared pure DTO in `csa-core`. Re-exported; `fetch_items` below
// still builds it via a struct literal.
pub use csa_core::types::ItemData;

/// DDragon raw item.json structures (only the fields we need)
#[derive(Debug, Deserialize)]
struct RawItemFile {
    data: std::collections::HashMap<String, RawItem>,
}

#[derive(Debug, Deserialize)]
struct RawItem {
    name: String,
    #[serde(default)]
    tags: Vec<String>,
    gold: RawItemGold,
    #[serde(default)]
    into: Vec<String>,
    #[serde(default)]
    maps: std::collections::HashMap<String, bool>,
    #[serde(rename = "requiredAlly", default)]
    required_ally: String,
}

#[derive(Debug, Deserialize)]
struct RawItemGold {
    total: u32,
    #[serde(default)]
    purchasable: bool,
}

/// Fetch and filter completed items from DDragon for the given patch version.
/// Filters: purchasable, Summoner's Rift (map 11), total_gold >= 1800,
/// `into` is null/empty (i.e. the item is fully built, not a component).
pub async fn fetch_items(version: &str) -> anyhow::Result<Vec<ItemData>> {
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let url = format!(
        "https://ddragon.leagueoflegends.com/cdn/{}/data/en_US/item.json",
        version
    );

    let raw: RawItemFile = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let items = raw
        .data
        .into_iter()
        .filter_map(|(id_str, item)| {
            let id: u32 = id_str.parse().ok()?;
            // Must be purchasable
            if !item.gold.purchasable {
                return None;
            }
            // Must be on Summoner's Rift (map id "11")
            if !item.maps.get("11").copied().unwrap_or(false) {
                return None;
            }
            // No required ally (e.g. Kalista's Oathsworn item)
            if !item.required_ally.is_empty() {
                return None;
            }
            // Minimum gold threshold — keeps legendary/mythic items, drops cheap components
            if item.gold.total < 1800 {
                return None;
            }
            let is_completed = item.into.is_empty();
            Some(ItemData {
                id,
                name: item.name,
                tags: item.tags,
                total_gold: item.gold.total,
                is_completed,
            })
        })
        .collect();

    Ok(items)
}

// ---------------------------------------------------------------------------
// Rune data
// ---------------------------------------------------------------------------

// RuneTree + RuneEntry are now shared pure DTOs in `csa-core`. Re-exported; the
// runesReforged fetcher below still builds them via struct literals.
pub use csa_core::types::{RuneEntry, RuneTree};

/// DDragon raw runesReforged.json structures
#[derive(Debug, Deserialize)]
struct RawRuneTree {
    id: u32,
    key: String,
    slots: Vec<RawRuneSlot>,
}

#[derive(Debug, Deserialize)]
struct RawRuneSlot {
    runes: Vec<RawRune>,
}

#[derive(Debug, Deserialize)]
struct RawRune {
    id: u32,
    key: String,
}

/// Fetch rune trees from DDragon for the given patch version.
pub async fn fetch_runes(version: &str) -> anyhow::Result<Vec<RuneTree>> {
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let url = format!(
        "https://ddragon.leagueoflegends.com/cdn/{}/data/en_US/runesReforged.json",
        version
    );

    let raw: Vec<RawRuneTree> = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let trees = raw
        .into_iter()
        .map(|t| RuneTree {
            id: t.id,
            key: t.key,
            slots: t
                .slots
                .into_iter()
                .map(|s| {
                    s.runes
                        .into_iter()
                        .map(|r| RuneEntry {
                            id: r.id,
                            key: r.key,
                        })
                        .collect()
                })
                .collect(),
        })
        .collect();

    Ok(trees)
}

const CDRAGON_URL: &str =
    "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/champion-summary.json";

/// Fetch champion role data from CDragon.
/// Does **not** hold any DB connection — caller should lock DB only after this returns.
pub async fn fetch_champion_meta() -> Result<Vec<CdragonChampion>> {
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let raw: Vec<CdragonChampion> = client
        .get(CDRAGON_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // Filter out the "-1" placeholder entry CDragon includes
    Ok(raw.into_iter().filter(|c| c.id > 0).collect())
}

/// Heuristic: melee if any role is fighter/tank AND attack_range is not explicitly marksman.
pub fn is_melee_from_roles(roles: &[String]) -> bool {
    roles.iter().any(|r| {
        let r = r.to_lowercase();
        r == "fighter" || r == "tank" || r == "assassin"
    }) && !roles.iter().any(|r| r.to_lowercase() == "marksman")
}
