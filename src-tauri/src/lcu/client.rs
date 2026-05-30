use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use reqwest::{Client, ClientBuilder};
use serde::de::DeserializeOwned;
use std::time::Duration;

use super::lockfile::Lockfile;

#[derive(Debug, Clone)]
pub struct LcuClient {
    http: Client,
    base_url: String,
    auth_header: String,
}

impl LcuClient {
    pub fn new(lf: &Lockfile) -> Result<Self> {
        let http = ClientBuilder::new()
            .danger_accept_invalid_certs(true) // LCU self-signed cert
            .timeout(Duration::from_secs(5))
            .build()?;

        let creds = general_purpose::STANDARD.encode(format!("riot:{}", lf.password));
        let auth_header = format!("Basic {}", creds);
        let base_url = format!("{}://127.0.0.1:{}", lf.protocol, lf.port);

        Ok(Self {
            http,
            base_url,
            auth_header,
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(&url)
            .header("Authorization", &self.auth_header)
            .header("Accept", "application/json")
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await?;
        Ok(response)
    }

    pub async fn get_raw(&self, path: &str) -> Result<serde_json::Value> {
        self.get(path).await
    }

    pub async fn patch_json(&self, path: &str, body: &serde_json::Value) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        self.http
            .patch(&url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
