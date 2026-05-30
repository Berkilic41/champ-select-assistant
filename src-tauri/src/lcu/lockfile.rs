use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Lockfile {
    pub name: String,
    pub pid: u32,
    pub port: u16,
    pub password: String,
    pub protocol: String,
}

impl Lockfile {
    pub fn parse(content: &str) -> Result<Self> {
        let parts: Vec<&str> = content.trim().split(':').collect();
        anyhow::ensure!(
            parts.len() == 5,
            "lockfile format geçersiz: {} part beklendi, {} bulundu",
            5,
            parts.len()
        );

        Ok(Lockfile {
            name: parts[0].to_string(),
            pid: parts[1].parse().context("PID parse hatası")?,
            port: parts[2].parse().context("port parse hatası")?,
            password: parts[3].to_string(),
            protocol: parts[4].to_string(),
        })
    }
}

/// League of Legends lockfile'ını bulmaya çalışır.
/// Sırasıyla birkaç yaygın path denenir.
pub fn find_lockfile() -> Result<Lockfile> {
    let candidates = lockfile_candidates();

    for path in &candidates {
        if path.exists() {
            tracing::info!("lockfile bulundu: {}", path.display());
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("lockfile okunamadı: {}", path.display()))?;
            return Lockfile::parse(&content);
        }
    }

    anyhow::bail!(
        "League Client lockfile bulunamadı. League of Legends açık mı? Denenen pathler:\n{}",
        candidates
            .iter()
            .map(|p| format!("  - {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn lockfile_candidates() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from(r"C:\Riot Games\League of Legends\lockfile"),
        PathBuf::from(r"C:\Program Files\Riot Games\League of Legends\lockfile"),
        PathBuf::from(r"C:\Program Files (x86)\Riot Games\League of Legends\lockfile"),
    ];

    // Kullanıcı home dizininden de dene (bazı kurulumlar burada)
    if let Some(home) = dirs_next_home() {
        paths.push(
            home.join("Riot Games")
                .join("League of Legends")
                .join("lockfile"),
        );
    }

    paths
}

fn dirs_next_home() -> Option<PathBuf> {
    std::env::var("USERPROFILE").ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_lockfile() {
        let content = "LeagueClient:12345:50123:testpassword:https";
        let lf = Lockfile::parse(content).unwrap();
        assert_eq!(lf.name, "LeagueClient");
        assert_eq!(lf.pid, 12345);
        assert_eq!(lf.port, 50123);
        assert_eq!(lf.password, "testpassword");
        assert_eq!(lf.protocol, "https");
    }

    #[test]
    fn test_parse_invalid_lockfile() {
        let content = "invalid";
        assert!(Lockfile::parse(content).is_err());
    }
}
