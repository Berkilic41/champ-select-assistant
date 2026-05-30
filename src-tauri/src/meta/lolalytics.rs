// dead_code is allowed at the module level (see meta/mod.rs).

use anyhow::Result;
use reqwest::Client;

const LOLALYTICS_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120";

/// Lolalytics'ten belirli bir şampiyonun matchup win rate verilerini dene.
/// Cloudflare koruma varsa Err döner — engine fallback yaparak devam eder.
pub async fn fetch_matchups(_champion_key: &str, _position: &str) -> Result<Vec<MatchupEntry>> {
    let _client = Client::builder()
        .use_rustls_tls()
        .user_agent(LOLALYTICS_UA)
        .build()?;

    // HTML sayfa döner, JSON extract etmek gerekir — Sprint 4 basit yaklaşım:
    // Şimdilik başarısız dön, gelecekte full scraper implement edilir.
    anyhow::bail!("Lolalytics scraper henüz implementasyonsuz — CDragon fallback kullanılıyor")
}

#[derive(Debug)]
pub struct MatchupEntry {
    pub opponent_key: String,
    pub win_rate: f32,
    pub games: u32,
}
