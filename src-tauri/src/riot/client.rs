use super::rate_limiter::{new_limiter, Limiter};
use anyhow::{bail, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct RiotClient {
    api_key: String,
    proxy_base: Option<String>,
    http: Client,
    limiter: Limiter,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SummonerInfo {
    pub puuid: String,
    pub game_name: String,
    pub tag_line: String,
    pub region: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResult {
    pub synced: u32,
    pub skipped: u32,
    pub errors: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChampionStats {
    pub champion_id: i64,
    pub games: u32,
    pub wins: u32,
    pub kda_avg: f32,
}

/// Maps a platform region (e.g. "tr1") to a regional routing value (e.g. "europe").
pub fn routing_for_region(region: &str) -> &'static str {
    match region.to_lowercase().as_str() {
        "euw1" | "eune1" | "tr1" | "ru" => "europe",
        "na1" | "la1" | "la2" => "americas",
        "kr" | "jp1" => "asia",
        _ => "europe",
    }
}

impl RiotClient {
    pub fn new(api_key: String) -> Self {
        let http = Client::builder()
            .use_rustls_tls()
            .build()
            .expect("reqwest client oluşturulamadı");
        RiotClient {
            api_key,
            proxy_base: None,
            http,
            limiter: new_limiter(20),
        }
    }

    pub fn new_with_proxy(proxy_base: String) -> Self {
        let http = Client::builder()
            .use_rustls_tls()
            .build()
            .expect("reqwest client oluşturulamadı");
        RiotClient {
            api_key: String::new(),
            proxy_base: Some(proxy_base),
            http,
            limiter: new_limiter(20),
        }
    }

    fn effective_url(&self, url: &str) -> String {
        match &self.proxy_base {
            Some(base) => {
                // "https://tr1.api.riotgames.com/lol/..." → "{base}/tr1.api.riotgames.com/lol/..."
                let stripped = url.strip_prefix("https://").unwrap_or(url);
                format!("{}/{}", base.trim_end_matches('/'), stripped)
            }
            None => url.to_string(),
        }
    }

    pub async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        self.limiter.until_ready().await;
        let effective = self.effective_url(url);
        let mut builder = self.http.get(&effective);
        if self.proxy_base.is_none() {
            builder = builder.header("X-Riot-Token", &self.api_key);
        }
        let resp = builder.send().await?;

        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            let wait = resp
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(5);
            tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;
            // single retry after back-off
            self.limiter.until_ready().await;
            let mut builder2 = self.http.get(&effective);
            if self.proxy_base.is_none() {
                builder2 = builder2.header("X-Riot-Token", &self.api_key);
            }
            let resp2 = builder2.send().await?;
            if !resp2.status().is_success() {
                bail!("Riot API error {} (after retry): {}", resp2.status(), url);
            }
            return Ok(resp2.json::<T>().await?);
        }

        if !resp.status().is_success() {
            bail!("Riot API error {}: {}", resp.status(), url);
        }

        Ok(resp.json::<T>().await?)
    }
}
