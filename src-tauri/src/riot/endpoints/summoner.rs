use crate::riot::client::{routing_for_region, RiotClient, SummonerInfo};
use anyhow::Result;

pub async fn get_by_riot_id(
    client: &RiotClient,
    game_name: &str,
    tag_line: &str,
    region: &str,
) -> Result<SummonerInfo> {
    let routing = routing_for_region(region);
    let encoded_name = urlencoding::encode(game_name);
    let encoded_tag = urlencoding::encode(tag_line);
    let url = format!(
        "https://{routing}.api.riotgames.com/riot/account/v1/accounts/by-riot-id/{encoded_name}/{encoded_tag}"
    );

    #[derive(serde::Deserialize)]
    struct AccountDto {
        puuid: String,
        #[serde(rename = "gameName")]
        game_name: String,
        #[serde(rename = "tagLine")]
        tag_line: String,
    }

    let dto: AccountDto = client.get(&url).await?;
    Ok(SummonerInfo {
        puuid: dto.puuid,
        game_name: dto.game_name,
        tag_line: dto.tag_line,
        region: region.to_string(),
    })
}
