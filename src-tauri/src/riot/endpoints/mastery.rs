use crate::riot::client::RiotClient;
use anyhow::Result;

pub async fn top_by_puuid(
    client: &RiotClient,
    puuid: &str,
    platform: &str,
    count: u8,
) -> Result<Vec<serde_json::Value>> {
    let url = format!(
        "https://{platform}.api.riotgames.com/lol/champion-mastery/v4/champion-masteries/by-puuid/{puuid}/top?count={count}"
    );
    client.get(&url).await
}
