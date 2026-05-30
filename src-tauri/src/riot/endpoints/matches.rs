use crate::riot::client::RiotClient;
use anyhow::Result;

/// Builds the match-list URL without making a network call.
/// `match_type` — optional `type` filter (e.g. `"ranked"`, `"normal"`).
/// When `None` the `type` parameter is omitted entirely.
pub fn build_list_ids_url(
    puuid: &str,
    routing: &str,
    match_type: Option<&str>,
    queue: Option<u32>,
    count: u8,
) -> String {
    let type_param = match_type.map(|t| format!("&type={t}")).unwrap_or_default();
    let queue_param = queue.map(|q| format!("&queue={q}")).unwrap_or_default();
    format!(
        "https://{routing}.api.riotgames.com/lol/match/v5/matches/by-puuid/{puuid}/ids?count={count}{type_param}{queue_param}"
    )
}

/// Fetch match IDs for the given puuid from the Riot match-v5 endpoint.
///
/// - `match_type`: optional `type=` filter (`None` = all queue types,
///   `Some("ranked")` = ranked only, `Some("normal")` = normals, etc.).
/// - `queue`: optional `queue=` filter (e.g. `Some(420)` for Ranked Solo/Duo).
pub async fn list_ids(
    client: &RiotClient,
    puuid: &str,
    routing: &str,
    match_type: Option<&str>,
    queue: Option<u32>,
    count: u8,
) -> Result<Vec<String>> {
    let url = build_list_ids_url(puuid, routing, match_type, queue, count);
    client.get(&url).await
}

#[cfg(test)]
mod tests {
    use super::build_list_ids_url;

    #[test]
    fn queue_type_none_omits_type_param() {
        let url = build_list_ids_url("test-puuid", "europe", None, None, 10);
        assert!(
            !url.contains("type="),
            "match_type: None must not add a 'type' parameter, got: {url}"
        );
        assert!(url.contains("count=10"), "count param must be present");
    }

    #[test]
    fn queue_type_some_ranked_adds_type_param() {
        let url = build_list_ids_url("test-puuid", "europe", Some("ranked"), None, 10);
        assert!(
            url.contains("&type=ranked"),
            "match_type: Some(\"ranked\") must add '&type=ranked', got: {url}"
        );
    }

    #[test]
    fn queue_param_appended_when_some() {
        let url = build_list_ids_url("abc", "americas", Some("ranked"), Some(420), 5);
        assert!(
            url.contains("&queue=420"),
            "queue=420 must appear, got: {url}"
        );
        assert!(
            url.contains("&type=ranked"),
            "type=ranked must appear, got: {url}"
        );
    }

    #[test]
    fn queue_param_absent_when_none() {
        let url = build_list_ids_url("abc", "americas", None, None, 5);
        assert!(
            !url.contains("queue="),
            "queue param must be absent, got: {url}"
        );
        assert!(
            !url.contains("type="),
            "type param must be absent, got: {url}"
        );
    }
}

pub async fn get_detail(
    client: &RiotClient,
    match_id: &str,
    routing: &str,
) -> Result<serde_json::Value> {
    let url = format!("https://{routing}.api.riotgames.com/lol/match/v5/matches/{match_id}");
    client.get(&url).await
}
