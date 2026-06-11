//! Lobby scouting (Faz 3) — enemy playstyle profiling + ban-target reasoning.
//!
//! Pure: takes pre-fetched enemy champion-pool summaries (resolved to a KB
//! archetype string by the command layer) and produces a human-readable scouting
//! report. No LCU/DB/network here — `commands/scouting.rs` passes the data in.
//! Deliberately separate from `ban_advisor` (threat *scoring*); this layer adds
//! the *explanation* / playstyle read.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One enemy's pool input: their most-played champion + how concentrated their
/// pool is, plus the KB archetype string (resolved by the caller). `archetype`
/// is `None` when the champion has no KB entry.
#[derive(Debug, Clone)]
pub struct ScoutPoolInput {
    pub cell_id: i32,
    pub champion_id: u32,
    pub champion_key: String,
    pub play_rate: f32,
    pub game_count: u32,
    pub archetype: Option<String>,
}

/// Playstyle profile for one enemy slot.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct EnemyProfile {
    pub cell_id: i32,
    pub top_champion_id: u32,
    pub top_champion_key: String,
    pub play_rate: f32,
    pub game_count: u32,
    /// "otp" | "ana_havuz" | "flex" — pool concentration.
    pub pool_tendency: String,
    /// TR playstyle tags from the KB archetype (e.g. "erken agresif", "scaling").
    pub playstyle_tags: Vec<String>,
    /// "yüksek" | "orta" | "düşük". TR display string, kept for backward-compat.
    pub threat: String,
    /// Locale-free threat token ("high" | "medium" | "low") mirroring `threat`.
    /// Added for the UI i18n layer; UI maps this via `champSelect.threat.*`, while
    /// `threat` (TR) is retained so existing bindings don't break.
    pub threat_level: String,
    /// One-line TR scouting note.
    pub note: String,
}

/// A ban the scouting layer recommends — with an EXPLANATION, not just a score.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct ScoutBanTarget {
    pub champion_id: u32,
    pub champion_key: String,
    pub reason: String,
    /// "high" | "medium" | "low" — from the sample size behind the read.
    pub confidence: String,
}

/// Enemy team's likely win condition (machine tokens; UI maps to labels). Canonical
/// vocabulary referenced by the i18n drift guard + UI contract, not by lib logic.
#[allow(dead_code)]
pub const SCOUT_WIN_CONDITIONS: [&str; 6] =
    ["pick", "teamfight", "protect", "poke", "split", "mixed"];

/// Aggregate, team-level scouting read — the enemy's likely gameplan + the carry to
/// watch + how to play against it. Complements the per-enemy `EnemyProfile` list.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct TeamScoutRead {
    /// Machine token from `SCOUT_WIN_CONDITIONS`.
    pub win_condition: String,
    /// How many enemies read as high ("yüksek") threat.
    pub high_threat_count: u32,
    /// The single biggest carry threat to watch (None when all reads are low).
    pub primary_threat_key: Option<String>,
    pub primary_threat_cell_id: Option<i32>,
    /// One-line TR counter-strategy against the detected comp.
    pub strategy_note: String,
}

/// Full scouting read for the enemy team.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub struct ScoutingReport {
    pub enemies: Vec<EnemyProfile>,
    pub ban_targets: Vec<ScoutBanTarget>,
    /// Aggregate team-level read (win condition + primary threat + counter-strategy).
    pub team_read: TeamScoutRead,
    /// True when the LCU history was thin (low confidence across the board).
    pub partial: bool,
}

/// KB archetype → (TR playstyle tags, base threat band).
fn playstyle_for(archetype: Option<&str>) -> (Vec<&'static str>, &'static str) {
    match archetype.unwrap_or("") {
        "assassin" => (
            vec!["erken agresif", "snowball", "tek hedef patlatma"],
            "yüksek",
        ),
        "diver" => (vec!["engage", "dalış", "frontline"], "yüksek"),
        "burst_mage" => (vec!["burst", "pick potansiyeli"], "yüksek"),
        "skirmisher" => (vec!["sürekli dövüş", "duello"], "orta"),
        "juggernaut" => (vec!["frontline", "yakın dövüş baskısı"], "orta"),
        "marksman" => (vec!["scaling", "sürekli hasar"], "orta"),
        "control_mage" => (vec!["zone kontrol", "scaling"], "orta"),
        "artillery" => (vec!["poke", "uzun menzil"], "orta"),
        "catcher" => (vec!["pick", "tutuş"], "orta"),
        "vanguard" => (vec!["engage", "frontline", "sert CC"], "orta"),
        "warden" => (vec!["peel", "koruma"], "düşük"),
        "enchanter" => (vec!["koruma", "scaling", "pasif lane"], "düşük"),
        _ => (vec!["dengeli profil"], "orta"),
    }
}

fn pool_tendency(play_rate: f32) -> (&'static str, &'static str) {
    // (machine key, TR label)
    if play_rate >= 0.60 {
        ("otp", "tek şampiyon (OTP)")
    } else if play_rate >= 0.35 {
        ("ana_havuz", "dar ana havuz")
    } else {
        ("flex", "esnek havuz")
    }
}

fn confidence_for(game_count: u32) -> &'static str {
    match game_count {
        0..=2 => "low",
        3..=9 => "medium",
        _ => "high",
    }
}

/// Map the TR threat band to a stable locale-free token for the UI i18n layer.
/// Mirrors the `confidence` token scheme. `threat` (TR) is kept for backward-compat;
/// the UI migrates to this token via `champSelect.threat.*`.
fn threat_token(band: &str) -> &'static str {
    match band {
        "yüksek" => "high",
        "düşük" => "low",
        _ => "medium",
    }
}

fn threat_rank(band: &str) -> u32 {
    match band {
        "yüksek" => 2,
        "orta" => 1,
        _ => 0,
    }
}

/// Classify the enemy team's win condition from its archetype distribution. Needs ≥2
/// archetypes in a category to commit (avoids single-champion noise) → else `mixed`.
fn classify_win_condition(classes: &[String]) -> &'static str {
    let (mut pick, mut teamfight, mut protect, mut poke, mut split) = (0u32, 0, 0, 0, 0);
    for c in classes {
        match c.as_str() {
            "assassin" | "catcher" => pick += 1,
            "vanguard" | "control_mage" | "battle_mage" => teamfight += 1,
            "enchanter" | "warden" => protect += 1,
            "artillery" => poke += 1,
            "juggernaut" | "skirmisher" => split += 1,
            _ => {}
        }
    }
    let scores = [
        ("pick", pick),
        ("teamfight", teamfight),
        ("protect", protect),
        ("poke", poke),
        ("split", split),
    ];
    let (label, count) = scores
        .iter()
        .max_by_key(|(_, c)| *c)
        .copied()
        .unwrap_or(("mixed", 0));
    if count >= 2 {
        label
    } else {
        "mixed"
    }
}

/// One-line TR counter-strategy for the detected enemy win condition.
fn strategy_note_for(win_condition: &str) -> &'static str {
    match win_condition {
        "pick" => "Pick komp: vision basın, izole durmayın, gruplu hareket edin.",
        "teamfight" => "Teamfight komp: AOE engage'lerine girmeyin; peel ve pozisyon önceliği.",
        "poke" => "Poke komp: poke menziline girmeden engage fırsatı yaratın, sustain alın.",
        "protect" => "Protect komp: carry'lerini koruyorlar; frontline'ı aşıp carry'ye ulaşın.",
        "split" => "Split komp: yan koridor baskısına waveclear ve 1-3-1 ile cevap verin.",
        _ => "Karma komp: net win-condition yok; tempo ve obje kontrolüyle baskı kurun.",
    }
}

/// Aggregate the per-enemy reads into a team-level gameplan + the carry to watch.
fn build_team_read(enemies: &[EnemyProfile], classes: &[String]) -> TeamScoutRead {
    let win_condition = classify_win_condition(classes);
    let high_threat_count = enemies.iter().filter(|e| e.threat == "yüksek").count() as u32;

    // Primary threat: highest threat band, then play_rate · games (confident carry).
    let primary = enemies
        .iter()
        .filter(|e| threat_rank(&e.threat) >= 1)
        .max_by(|a, b| {
            threat_rank(&a.threat).cmp(&threat_rank(&b.threat)).then(
                (a.play_rate * a.game_count as f32)
                    .partial_cmp(&(b.play_rate * b.game_count as f32))
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

    TeamScoutRead {
        win_condition: win_condition.to_string(),
        high_threat_count,
        primary_threat_key: primary.map(|e| e.top_champion_key.clone()),
        primary_threat_cell_id: primary.map(|e| e.cell_id),
        strategy_note: strategy_note_for(win_condition).to_string(),
    }
}

/// Build the scouting report. Enemies with `top_champion_id == 0` (no history)
/// are dropped. Ban targets are the high-threat, concentrated pools, explained.
pub fn build_scouting_report(pools: &[ScoutPoolInput]) -> ScoutingReport {
    let mut enemies = Vec::new();
    let mut ban_targets = Vec::new();
    let mut classes: Vec<String> = Vec::new();

    for p in pools.iter().filter(|p| p.champion_id != 0) {
        classes.push(p.archetype.clone().unwrap_or_default());
        let (tags, base_threat) = playstyle_for(p.archetype.as_deref());
        let (tendency_key, tendency_label) = pool_tendency(p.play_rate);
        let pct = (p.play_rate * 100.0).round() as u32;

        // A concentrated pool on a high-threat archetype reads as more dangerous.
        let threat = if base_threat == "yüksek" && tendency_key == "otp" {
            "yüksek"
        } else if base_threat == "düşük" && tendency_key == "flex" {
            "düşük"
        } else {
            base_threat
        };

        let note = format!(
            "{} · {} (son {} maçta %{})",
            p.champion_key, tendency_label, p.game_count, pct
        );

        // Ban-target: a comfort pick they lean on AND that snowballs hard.
        if p.play_rate >= 0.50 && base_threat == "yüksek" && p.game_count >= 3 {
            ban_targets.push((
                p.play_rate,
                ScoutBanTarget {
                    champion_id: p.champion_id,
                    champion_key: p.champion_key.clone(),
                    reason: format!(
                        "Rakibin ana kozu (%{pct}); banı erken tempo ve oyun planını bozar"
                    ),
                    confidence: confidence_for(p.game_count).to_string(),
                },
            ));
        }

        enemies.push(EnemyProfile {
            cell_id: p.cell_id,
            top_champion_id: p.champion_id,
            top_champion_key: p.champion_key.clone(),
            play_rate: p.play_rate,
            game_count: p.game_count,
            pool_tendency: tendency_key.to_string(),
            playstyle_tags: tags.into_iter().map(String::from).collect(),
            threat: threat.to_string(),
            threat_level: threat_token(threat).to_string(),
            note,
        });
    }

    // Strongest comfort pick first; cap at 3, dedup by champion.
    ban_targets.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut seen = std::collections::HashSet::new();
    let ban_targets: Vec<ScoutBanTarget> = ban_targets
        .into_iter()
        .map(|(_, t)| t)
        .filter(|t| seen.insert(t.champion_id))
        .take(3)
        .collect();

    // Partial when we have no enemy with a confident read.
    let partial = enemies.is_empty() || enemies.iter().all(|e| e.game_count < 5);

    let team_read = build_team_read(&enemies, &classes);

    ScoutingReport {
        enemies,
        ban_targets,
        team_read,
        partial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(cell: i32, id: u32, key: &str, play: f32, games: u32, arch: &str) -> ScoutPoolInput {
        ScoutPoolInput {
            cell_id: cell,
            champion_id: id,
            champion_key: key.to_string(),
            play_rate: play,
            game_count: games,
            archetype: (!arch.is_empty()).then(|| arch.to_string()),
        }
    }

    #[test]
    fn otp_assassin_is_high_threat_and_ban_target() {
        let report = build_scouting_report(&[pool(5, 238, "Zed", 0.70, 12, "assassin")]);
        let e = &report.enemies[0];
        assert_eq!(e.pool_tendency, "otp");
        assert_eq!(e.threat, "yüksek");
        assert_eq!(e.threat_level, "high", "TR band → locale-free token");
        assert!(e.playstyle_tags.iter().any(|t| t == "erken agresif"));
        let ban = report
            .ban_targets
            .iter()
            .find(|b| b.champion_id == 238)
            .expect("ban target");
        assert!(ban.reason.contains("ana kozu"));
        assert_eq!(ban.confidence, "high");
        assert!(!report.partial, "12 games → confident read");
    }

    #[test]
    fn enchanter_flex_is_low_threat_no_ban() {
        let report = build_scouting_report(&[pool(5, 117, "Lulu", 0.30, 8, "enchanter")]);
        assert_eq!(report.enemies[0].threat, "düşük");
        assert_eq!(report.enemies[0].threat_level, "low");
        assert_eq!(report.enemies[0].pool_tendency, "flex");
        assert!(
            report.ban_targets.is_empty(),
            "low-threat enchanter is not a ban target"
        );
    }

    #[test]
    fn thin_history_marks_report_partial() {
        let report = build_scouting_report(&[pool(5, 238, "Zed", 0.80, 2, "assassin")]);
        assert!(report.partial, "2 games → partial");
        // play_rate high but game_count < 3 → not yet a ban target.
        assert!(report.ban_targets.is_empty());
        assert_eq!(report.enemies[0].pool_tendency, "otp");
    }

    #[test]
    fn no_history_enemy_is_dropped() {
        let report = build_scouting_report(&[
            pool(5, 0, "", 0.0, 0, ""),
            pool(6, 238, "Zed", 0.65, 10, "assassin"),
        ]);
        assert_eq!(report.enemies.len(), 1, "champion_id 0 must be dropped");
        assert_eq!(report.enemies[0].cell_id, 6);
    }

    #[test]
    fn kbless_champion_gets_neutral_profile() {
        let report = build_scouting_report(&[pool(5, 990001, "Synthetic", 0.50, 6, "")]);
        let e = &report.enemies[0];
        assert_eq!(e.threat, "orta");
        assert_eq!(e.threat_level, "medium");
        assert!(e.playstyle_tags.iter().any(|t| t == "dengeli profil"));
        assert!(
            report.ban_targets.is_empty(),
            "neutral archetype is not auto-banned"
        );
    }

    #[test]
    fn multiple_enemies_ban_targets_sorted_by_play_rate() {
        let report = build_scouting_report(&[
            pool(5, 238, "Zed", 0.55, 10, "assassin"),
            pool(6, 122, "Darius", 0.75, 14, "juggernaut"), // orta threat → not a ban target
            pool(7, 91, "Talon", 0.80, 20, "assassin"),
        ]);
        assert_eq!(report.enemies.len(), 3);
        // Two assassins qualify; sorted by play_rate desc (Talon 0.80 before Zed 0.55).
        assert_eq!(report.ban_targets[0].champion_id, 91);
        assert_eq!(report.ban_targets[1].champion_id, 238);
        assert!(
            !report.ban_targets.iter().any(|b| b.champion_id == 122),
            "orta-threat juggernaut must not be a ban target"
        );
    }

    #[test]
    fn duplicate_top_champion_deduped_in_ban_targets() {
        // Two enemies leaning on the same assassin → listed once, both still profiled.
        let report = build_scouting_report(&[
            pool(5, 238, "Zed", 0.60, 10, "assassin"),
            pool(6, 238, "Zed", 0.70, 12, "assassin"),
        ]);
        let zed_bans = report
            .ban_targets
            .iter()
            .filter(|b| b.champion_id == 238)
            .count();
        assert_eq!(
            zed_bans, 1,
            "duplicate champion appears once in ban targets"
        );
        assert_eq!(report.enemies.len(), 2);
    }

    #[test]
    fn high_playrate_low_games_is_otp_but_not_ban_target() {
        let report = build_scouting_report(&[pool(5, 238, "Zed", 0.90, 2, "assassin")]);
        assert_eq!(report.enemies[0].pool_tendency, "otp");
        assert!(
            report.ban_targets.is_empty(),
            "game_count < 3 → not yet a confident ban target"
        );
        assert!(report.partial, "thin history → partial");
    }

    #[test]
    fn team_read_detects_pick_comp_and_primary_threat() {
        let report = build_scouting_report(&[
            pool(5, 238, "Zed", 0.70, 12, "assassin"),
            pool(6, 91, "Talon", 0.60, 10, "assassin"),
            pool(7, 412, "Thresh", 0.50, 8, "catcher"),
        ]);
        assert_eq!(report.team_read.win_condition, "pick");
        assert!(report.team_read.strategy_note.contains("vision"));
        assert_eq!(
            report.team_read.high_threat_count, 2,
            "two assassins are high"
        );
        // Both assassins high threat → tie-break play_rate·games: Zed 8.4 > Talon 6.0.
        assert_eq!(report.team_read.primary_threat_key.as_deref(), Some("Zed"));
        assert_eq!(report.team_read.primary_threat_cell_id, Some(5));
    }

    #[test]
    fn team_read_mixed_when_no_category_reaches_two() {
        let report = build_scouting_report(&[
            pool(5, 238, "Zed", 0.50, 8, "assassin"),   // pick 1
            pool(6, 117, "Lulu", 0.40, 8, "enchanter"), // protect 1
        ]);
        assert_eq!(report.team_read.win_condition, "mixed");
        assert!(SCOUT_WIN_CONDITIONS.contains(&report.team_read.win_condition.as_str()));
    }

    #[test]
    fn team_read_no_primary_threat_when_all_low() {
        let report = build_scouting_report(&[pool(5, 117, "Lulu", 0.30, 8, "enchanter")]);
        assert!(report.team_read.primary_threat_key.is_none());
        assert_eq!(report.team_read.high_threat_count, 0);
    }

    /// Drift guard: every win-condition token the UI shows must have a
    /// `champSelect.scoutWinCondition.*` label. en parity is in `i18n-parity.test.ts`.
    #[test]
    fn every_win_condition_token_has_an_i18n_label() {
        const TR: &str = include_str!("../../src/i18n/tr.json");
        let tr: serde_json::Value = serde_json::from_str(TR).unwrap();
        let wc = &tr["champSelect"]["scoutWinCondition"];
        for token in SCOUT_WIN_CONDITIONS {
            assert!(!wc[token].is_null(), "win condition '{token}' i18n yok");
        }
    }

    #[test]
    fn ban_targets_capped_at_three() {
        let pools: Vec<ScoutPoolInput> = (0..5)
            .map(|i| {
                pool(
                    5 + i,
                    100 + i as u32,
                    &format!("A{i}"),
                    0.60,
                    10,
                    "assassin",
                )
            })
            .collect();
        let report = build_scouting_report(&pools);
        assert_eq!(report.ban_targets.len(), 3, "ban targets capped at 3");
    }
}
