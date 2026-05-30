pub mod cdragon;

use anyhow::Result;

#[derive(Debug)]
pub struct DdragonCache {
    pub version: Option<String>,
}

impl DdragonCache {
    pub fn new() -> Self {
        DdragonCache { version: None }
    }

    pub fn current_version(&self) -> &str {
        self.version.as_deref().unwrap_or("unknown")
    }
}

/// Data Dragon'dan champion listesini çek. (version, id, key, name, title) tuple listesi döner.
/// DB bağlantısı almaz — await sırasında Connection tutmamak için ayrıldı.
/// Returns the fetched patch version alongside the champion list so the caller can cache it.
pub async fn fetch_champion_list() -> Result<(String, Vec<(i64, String, String, String)>)> {
    let client = reqwest::Client::builder().use_rustls_tls().build()?;

    let versions: Vec<String> = client
        .get("https://ddragon.leagueoflegends.com/api/versions.json")
        .send()
        .await?
        .json()
        .await?;
    let version = versions
        .first()
        .ok_or_else(|| anyhow::anyhow!("DDragon version yok"))?;

    let url = format!("https://ddragon.leagueoflegends.com/cdn/{version}/data/en_US/champion.json");
    let data: serde_json::Value = client.get(&url).send().await?.json().await?;

    let fetched_version = version.clone();
    let mut list = Vec::new();
    if let Some(champions) = data["data"].as_object() {
        for (_, champ) in champions {
            // DDragon: "key" = numeric id string, "id" = key string ("Aatrox")
            let id: i64 = champ["key"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let key = champ["id"].as_str().unwrap_or("").to_string();
            let name = champ["name"].as_str().unwrap_or("").to_string();
            let title = champ["title"].as_str().unwrap_or("").to_string();
            if id > 0 && !key.is_empty() {
                list.push((id, key, name, title));
            }
        }
    }
    Ok((fetched_version, list))
}
