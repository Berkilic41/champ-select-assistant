//! Post-game / performance-trends coach (Faz 5 v1).
//!
//! Pure (no I/O): takes the player's recent match rows (champion keys resolved by
//! the command layer) and produces ONE decisive "main lesson" plus an honest
//! trend / form / tilt / pool read. Local-first — `commands/postgame.rs` passes
//! the data in.
//!
//! v1 works on the stored `matches` aggregate (W-L / KDA / role). Deeper timeline
//! insights (farm@10, objective-phase deaths, vision score) need match-timeline
//! ingestion and are a future extension — intentionally NOT faked here.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// One recent match. `champion_key` is resolved by the caller. Rows may be in any
/// order — `build_performance_report` sorts by `played_at` DESC internally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRow {
    pub champion_id: u32,
    pub champion_key: String,
    /// LCU position lowercase ("middle"…); "" when unknown.
    pub position: String,
    pub win: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub played_at: i64,
    /// Game length in seconds (for CS/min). 0 when unknown.
    pub duration_secs: u32,
    /// Total creep score; `None` for matches synced before CS ingestion.
    pub cs: Option<u32>,
}

/// Per-champion mini stats for the trends panel.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct ChampionTrend {
    pub champion_id: u32,
    pub champion_key: String,
    pub games: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub avg_kda: f32,
}

/// A single decisive post-game read — the iTero-style "one main lesson" plus the
/// supporting honest signals.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct PerformanceReport {
    pub games: u32,
    pub wins: u32,
    pub losses: u32,
    pub win_rate: f32,
    pub avg_kda: f32,
    /// Average CS per minute over matches that have CS data (None when none do).
    /// Surfaced as an honest stat — NOT turned into a lesson (jungle/support CS is
    /// legitimately low, so a threshold would false-alarm).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_cs_per_min: Option<f32>,
    /// Consecutive losses counting back from the most recent match.
    pub loss_streak: u32,
    /// last-5 win_rate − previous-5 win_rate, in `[-1, 1]`. 0 when < 10 games.
    pub form_delta: f32,
    /// Most-played position ("" when no role data).
    pub main_role: String,
    /// Fraction of role-known games NOT on `main_role` (0..1).
    pub off_role_rate: f32,
    pub top_champions: Vec<ChampionTrend>,
    /// The ONE actionable lesson. Honest, never a guarantee.
    pub main_lesson: String,
    pub tilt_warning: bool,
    /// True when < 5 games (low-confidence read).
    pub partial: bool,
}

const TILT_STREAK: u32 = 3;
const WEAK_CHAMP_MIN_GAMES: u32 = 4;
const WEAK_CHAMP_WR: f32 = 0.40;
const OFF_ROLE_THRESHOLD: f32 = 0.40;
const GOOD_FORM_DELTA: f32 = 0.20;
const LOW_DATA_GAMES: u32 = 5;

fn kda(k: u32, a: u32, d: u32) -> f32 {
    (k + a) as f32 / d.max(1) as f32
}

/// Build the performance report. `matches` must be most-recent-first.
pub fn build_performance_report(matches: &[MatchRow]) -> PerformanceReport {
    let games = matches.len() as u32;
    if games == 0 {
        return PerformanceReport {
            games: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.0,
            avg_kda: 0.0,
            avg_cs_per_min: None,
            loss_streak: 0,
            form_delta: 0.0,
            main_role: String::new(),
            off_role_rate: 0.0,
            top_champions: Vec::new(),
            main_lesson: "Henüz maç verisi yok — Lobi'de \"Maç geçmişini yükle\"ye tıkla."
                .to_string(),
            tilt_warning: false,
            partial: true,
        };
    }

    // Most-recent-first, sorted defensively here (streak/form must not depend on
    // the caller's ordering). `played_at` is the sort key.
    let mut ord: Vec<&MatchRow> = matches.iter().collect();
    ord.sort_by_key(|m| std::cmp::Reverse(m.played_at));

    let wins = ord.iter().filter(|m| m.win).count() as u32;
    let losses = games - wins;
    let win_rate = wins as f32 / games as f32;
    let avg_kda = ord
        .iter()
        .map(|m| kda(m.kills, m.assists, m.deaths))
        .sum::<f32>()
        / games as f32;

    // Average CS/min over matches that actually carry CS data (≥1 min played).
    let cs_paces: Vec<f32> = ord
        .iter()
        .filter_map(|m| match m.cs {
            Some(cs) if m.duration_secs >= 60 => Some(cs as f32 / (m.duration_secs as f32 / 60.0)),
            _ => None,
        })
        .collect();
    let avg_cs_per_min = if cs_paces.is_empty() {
        None
    } else {
        let avg = cs_paces.iter().sum::<f32>() / cs_paces.len() as f32;
        Some((avg * 10.0).round() / 10.0)
    };

    // Loss streak from the most recent match.
    let loss_streak = ord.iter().take_while(|m| !m.win).count() as u32;

    // Form: last-5 vs previous-5 win rate (only when there are ≥10 games).
    let form_delta = if games >= 10 {
        let wr = |slice: &[&MatchRow]| {
            slice.iter().filter(|m| m.win).count() as f32 / slice.len() as f32
        };
        wr(&ord[0..5]) - wr(&ord[5..10])
    } else {
        0.0
    };

    // Role consistency over role-known games.
    let mut role_counts: HashMap<&str, u32> = HashMap::new();
    for m in ord.iter().filter(|m| !m.position.is_empty()) {
        *role_counts.entry(m.position.as_str()).or_insert(0) += 1;
    }
    let role_games: u32 = role_counts.values().sum();
    let (main_role, main_role_count) = role_counts
        .iter()
        .max_by_key(|(_, c)| **c)
        .map(|(r, c)| (r.to_string(), *c))
        .unwrap_or_default();
    let off_role_rate = if role_games > 0 {
        1.0 - (main_role_count as f32 / role_games as f32)
    } else {
        0.0
    };

    // Per-champion aggregation → top 3 by games.
    let mut by_champ: HashMap<u32, (String, u32, u32, f32)> = HashMap::new(); // id → (key, games, wins, kda_sum)
    for m in &ord {
        let e = by_champ
            .entry(m.champion_id)
            .or_insert((m.champion_key.clone(), 0, 0, 0.0));
        e.1 += 1;
        e.2 += m.win as u32;
        e.3 += kda(m.kills, m.assists, m.deaths);
    }
    let mut top_champions: Vec<ChampionTrend> = by_champ
        .into_iter()
        .map(|(id, (key, g, w, kda_sum))| ChampionTrend {
            champion_id: id,
            champion_key: key,
            games: g,
            wins: w,
            win_rate: w as f32 / g as f32,
            avg_kda: kda_sum / g as f32,
        })
        .collect();
    top_champions.sort_by(|a, b| {
        b.games.cmp(&a.games).then(
            b.win_rate
                .partial_cmp(&a.win_rate)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    top_champions.truncate(3);

    let tilt_warning = loss_streak >= TILT_STREAK;
    let main_lesson = main_lesson(
        loss_streak,
        &top_champions,
        off_role_rate,
        form_delta,
        win_rate,
    );

    PerformanceReport {
        games,
        wins,
        losses,
        win_rate,
        avg_kda,
        avg_cs_per_min,
        loss_streak,
        form_delta,
        main_role,
        off_role_rate,
        top_champions,
        main_lesson,
        tilt_warning,
        partial: games < LOW_DATA_GAMES,
    }
}

/// Pick the single most useful lesson, in priority order. Honest, no guarantees.
fn main_lesson(
    loss_streak: u32,
    top_champions: &[ChampionTrend],
    off_role_rate: f32,
    form_delta: f32,
    win_rate: f32,
) -> String {
    if loss_streak >= TILT_STREAK {
        return format!(
            "Üst üste {loss_streak} kayıp — kısa bir ara ver. Tilt'le alınan kararlar seriyi uzatır."
        );
    }
    if let Some(weak) = top_champions
        .iter()
        .find(|c| c.games >= WEAK_CHAMP_MIN_GAMES && c.win_rate < WEAK_CHAMP_WR)
    {
        let wr = (weak.win_rate * 100.0).round() as u32;
        return format!(
            "{}: son maçlarda %{wr} ({} maç) — şu an zorlanıyorsun; güvenli pick'e dön ya da bilinçli pratiğe al.",
            weak.champion_key, weak.games
        );
    }
    if off_role_rate > OFF_ROLE_THRESHOLD {
        let pct = (off_role_rate * 100.0).round() as u32;
        return format!(
            "Maçların ~%{pct}'i ana rolün dışında — tek role tutunmak daha hızlı ve tutarlı gelişim getirir."
        );
    }
    if form_delta > GOOD_FORM_DELTA {
        let pct = (form_delta * 100.0).round() as u32;
        return format!(
            "Form yükselişte (son 5'te +%{pct} WR) — momentumu koru, riskleri ölçülü al."
        );
    }
    if win_rate >= 0.55 {
        "İyi gidiyorsun — en güçlü 2-3 şampiyonuna yoğunlaşıp havuzu derinleştir.".to_string()
    } else {
        "Dengeli bir dönem — en çok oynadığın şampiyonlarda tutarlılığa odaklan.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(
        id: u32,
        key: &str,
        pos: &str,
        win: bool,
        k: u32,
        d: u32,
        a: u32,
        played: i64,
    ) -> MatchRow {
        MatchRow {
            champion_id: id,
            champion_key: key.into(),
            position: pos.into(),
            win,
            kills: k,
            deaths: d,
            assists: a,
            played_at: played,
            duration_secs: 1800,
            cs: None,
        }
    }

    #[test]
    fn empty_history_is_partial_with_guidance() {
        let r = build_performance_report(&[]);
        assert!(r.partial);
        assert_eq!(r.games, 0);
        assert!(r.main_lesson.contains("Maç geçmişini yükle"));
    }

    #[test]
    fn loss_streak_triggers_tilt_lesson() {
        // Most-recent-first: 3 losses then a win.
        let ms = vec![
            m(238, "Zed", "middle", false, 2, 8, 3, 500),
            m(238, "Zed", "middle", false, 1, 7, 2, 400),
            m(99, "Lux", "middle", false, 3, 6, 9, 300),
            m(99, "Lux", "middle", true, 5, 2, 10, 200),
            m(99, "Lux", "middle", true, 6, 3, 8, 100),
        ];
        let r = build_performance_report(&ms);
        assert_eq!(r.loss_streak, 3);
        assert!(r.tilt_warning);
        assert!(
            r.main_lesson.contains("ara ver"),
            "lesson: {}",
            r.main_lesson
        );
    }

    #[test]
    fn weak_champion_lesson_when_low_winrate() {
        // Zed: 5 games, 1 win (20% WR) but no active loss streak (most recent is a win).
        let ms = vec![
            m(238, "Zed", "middle", true, 8, 2, 4, 600),
            m(238, "Zed", "middle", false, 2, 6, 3, 500),
            m(238, "Zed", "middle", false, 1, 7, 2, 400),
            m(238, "Zed", "middle", false, 3, 5, 5, 300),
            m(238, "Zed", "middle", false, 2, 8, 4, 200),
        ];
        let r = build_performance_report(&ms);
        assert_eq!(r.loss_streak, 0, "most recent is a win → no streak");
        assert!(
            r.main_lesson.starts_with("Zed:"),
            "lesson: {}",
            r.main_lesson
        );
        assert_eq!(r.top_champions[0].champion_id, 238);
    }

    #[test]
    fn off_role_lesson_when_spread() {
        // 5 games, only 2 on the main role → off_role_rate 0.6.
        let ms = vec![
            m(1, "A", "middle", true, 5, 3, 5, 500),
            m(2, "B", "top", true, 4, 2, 3, 400),
            m(3, "C", "jungle", true, 6, 4, 7, 300),
            m(1, "A", "middle", false, 3, 5, 4, 200),
            m(4, "D", "bottom", true, 7, 2, 5, 100),
        ];
        let r = build_performance_report(&ms);
        assert!(r.off_role_rate > 0.5, "off_role_rate={}", r.off_role_rate);
        assert!(
            r.main_lesson.contains("ana rolün dışında"),
            "lesson: {}",
            r.main_lesson
        );
    }

    #[test]
    fn cs_per_min_averaged_only_over_matches_with_cs() {
        // Two 20-min games: 200 CS → 10.0/min, 160 CS → 8.0/min; one with no CS data.
        let with_cs = |cs: Option<u32>| MatchRow {
            champion_id: 1,
            champion_key: "A".into(),
            position: "middle".into(),
            win: true,
            kills: 5,
            deaths: 2,
            assists: 5,
            played_at: 100,
            duration_secs: 1200, // 20 min
            cs,
        };
        let ms = vec![with_cs(Some(200)), with_cs(Some(160)), with_cs(None)];
        let r = build_performance_report(&ms);
        assert_eq!(
            r.avg_cs_per_min,
            Some(9.0),
            "(10.0 + 8.0) / 2, None ignored"
        );

        // No CS data anywhere → honest None (never fabricated).
        let none = build_performance_report(&[with_cs(None)]);
        assert_eq!(none.avg_cs_per_min, None);
    }

    #[test]
    fn win_rate_and_kda_computed() {
        let ms = vec![
            m(1, "A", "middle", true, 10, 2, 6, 200),
            m(1, "A", "middle", false, 2, 4, 2, 100),
        ];
        let r = build_performance_report(&ms);
        assert_eq!(r.games, 2);
        assert_eq!(r.wins, 1);
        assert!((r.win_rate - 0.5).abs() < 1e-6);
        // kda: (16/2 + 4/4) / 2 = (8 + 1)/2 = 4.5
        assert!((r.avg_kda - 4.5).abs() < 1e-5, "kda={}", r.avg_kda);
        assert!(r.partial, "2 games < 5 → partial");
    }
}
