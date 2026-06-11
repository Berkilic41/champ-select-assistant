//! Draft verdict — a single decisive read on the draft.
//!
//! Pure synthesis of already-computed outputs (`GamePlan` timeline + both teams'
//! `CompSummary` + optional lane matchup). Where `game_plan` answers "how do we
//! win", this answers "are we ahead, and should we worry". No new model logic,
//! no I/O. Language is measured (the dodge note explicitly defers the call to
//! the player).

use super::draft_iq::game_plan::GamePlan;
use super::team_analysis::CompSummary;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DraftVerdict {
    /// "favorable" | "even" | "risky".
    pub favorability: String,
    /// 0..1 for a UI bar (0.5 = even).
    pub score: f32,
    pub headline: String,
    pub reasons: Vec<String>,
    /// True only on a severe, multi-signal disadvantage.
    pub dodge_consider: bool,
    pub dodge_note: Option<String>,
    /// One-line "do this first".
    pub top_action: String,
    /// Ally composition gaps worth communicating to the team.
    pub team_needs: Vec<String>,
}

pub fn compute_draft_verdict(
    plan: &GamePlan,
    ally: &CompSummary,
    enemy: &CompSummary,
    lane_matchup: Option<f32>,
) -> DraftVerdict {
    // Phase signal: net advantage across the timeline bands.
    let adv = plan
        .timeline
        .iter()
        .filter(|b| b.stance == "advantage")
        .count() as i32;
    let dis = plan
        .timeline
        .iter()
        .filter(|b| b.stance == "disadvantage")
        .count() as i32;
    let phase_sig = (adv - dis).signum();

    // Comp signal: fewer ally gaps than the enemy is good for us.
    let comp_sig = (enemy.gaps.len() as i32 - ally.gaps.len() as i32).signum();

    // Lane signal.
    let lane_sig = match lane_matchup {
        Some(m) if m > 0.55 => 1,
        Some(m) if m < 0.45 => -1,
        _ => 0,
    };

    let sum = phase_sig + comp_sig + lane_sig; // -3..3
    let favorability = if sum >= 2 {
        "favorable"
    } else if sum <= -2 {
        "risky"
    } else {
        "even"
    };
    let score = ((sum + 3) as f32 / 6.0).clamp(0.0, 1.0);

    let headline = match favorability {
        "favorable" => "Draft lehte görünüyor",
        "risky" => "Zorlu draft — dikkatli oyna",
        _ => "Dengeli draft",
    }
    .to_string();

    let mut reasons: Vec<String> = Vec::new();
    match phase_sig {
        1 => reasons.push("Güç eğrisi fazlarda lehte".to_string()),
        -1 => reasons.push("Fazların çoğunda rakip önde".to_string()),
        _ => {}
    }
    match comp_sig {
        1 => reasons.push("Kompozisyonun rakipten daha bütün".to_string()),
        -1 => reasons.push("Kompozisyonunda belirgin eksikler var".to_string()),
        _ => {}
    }
    match lane_sig {
        1 => reasons.push("Lane eşleşmen lehte".to_string()),
        -1 => reasons.push("Lane eşleşmen zor".to_string()),
        _ => {}
    }
    if reasons.is_empty() {
        reasons.push("Belirgin avantaj/dezavantaj yok — tempoya göre oyna".to_string());
    }

    // Dodge only on a severe, multi-signal disadvantage. The note enumerates the
    // concrete triggers (counts) and explicitly defers the call to the player.
    let lane_bad = matches!(lane_matchup, Some(m) if m < 0.45);
    let dodge_consider = favorability == "risky" && ally.gaps.len() >= 3 && (dis >= 2 || lane_bad);
    let dodge_note = if dodge_consider {
        let mut triggers = vec![format!("{} takım eksiği", ally.gaps.len())];
        if dis >= 2 {
            triggers.push(format!("{dis} fazda geride"));
        }
        if lane_bad {
            triggers.push("lane zor".to_string());
        }
        Some(format!(
            "Ciddi dezavantaj ({}). Dodge'u değerlendirebilirsin — kararı sen ver.",
            triggers.join(" · ")
        ))
    } else {
        None
    };

    let top_action = plan
        .win_conditions
        .first()
        .cloned()
        .unwrap_or_else(|| match favorability {
            "favorable" => "Avantajını obje/tempoyla somutlaştır".to_string(),
            "risky" => "Güvenli oyna, hata bekle ve scale et".to_string(),
            _ => "Fazlara göre tempo ayarla".to_string(),
        });

    DraftVerdict {
        favorability: favorability.to_string(),
        score,
        headline,
        reasons,
        dodge_consider,
        dodge_note,
        top_action,
        team_needs: ally.gaps.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft_iq::game_plan::PhaseBand;

    fn band(stance: &str) -> PhaseBand {
        PhaseBand {
            label: String::new(),
            stance: stance.to_string(),
            advice: String::new(),
            strength: 0.5,
        }
    }

    fn plan(stances: [&str; 3], win: Vec<&str>) -> GamePlan {
        GamePlan {
            team_identity: String::new(),
            identity_note: String::new(),
            timeline: vec![band(stances[0]), band(stances[1]), band(stances[2])],
            win_conditions: win.into_iter().map(String::from).collect(),
            objectives: vec![],
            enemy_threat: String::new(),
            alt_plan: None,
            partial: false,
        }
    }

    fn comp(gaps: Vec<&str>) -> CompSummary {
        CompSummary {
            tanks: 0,
            fighters: 0,
            mages: 0,
            marksmen: 0,
            assassins: 0,
            supports: 0,
            ap_share: 0.0,
            ad_share: 0.0,
            has_engage: false,
            has_frontline: false,
            has_hard_cc: false,
            has_peel: false,
            gaps: gaps.into_iter().map(String::from).collect(),
            summary: String::new(),
        }
    }

    #[test]
    fn strong_draft_is_favorable_no_dodge() {
        let p = plan(["advantage", "advantage", "even"], vec!["Tempo bas"]);
        let ally = comp(vec![]); // no gaps
        let enemy = comp(vec!["Engage yok", "Frontline yok"]); // enemy weaker
        let v = compute_draft_verdict(&p, &ally, &enemy, Some(0.70));
        assert_eq!(v.favorability, "favorable");
        assert!(!v.dodge_consider);
        assert_eq!(v.top_action, "Tempo bas");
        assert!(v.score > 0.8);
    }

    #[test]
    fn weak_draft_is_risky_with_dodge() {
        let p = plan(["disadvantage", "disadvantage", "disadvantage"], vec![]);
        let ally = comp(vec![
            "Engage yok",
            "Frontline yok",
            "Sert CC yok",
            "Peel/koruma yok",
        ]);
        let enemy = comp(vec![]);
        let v = compute_draft_verdict(&p, &ally, &enemy, Some(0.30));
        assert_eq!(v.favorability, "risky");
        assert!(v.dodge_consider);
        let note = v.dodge_note.expect("dodge note present");
        assert!(
            note.contains("4 takım eksiği"),
            "enumerates gap count: {note}"
        );
        assert!(
            note.contains("3 fazda geride"),
            "enumerates phase count: {note}"
        );
        assert!(note.contains("lane zor"), "enumerates bad lane: {note}");
        assert!(note.contains("kararı sen ver"), "defers to player: {note}");
        assert_eq!(v.team_needs.len(), 4);
    }

    #[test]
    fn mixed_signals_are_even_and_safe() {
        let p = plan(["advantage", "disadvantage", "even"], vec!["X"]);
        let ally = comp(vec!["Engage yok"]);
        let enemy = comp(vec!["Frontline yok"]);
        let v = compute_draft_verdict(&p, &ally, &enemy, None);
        assert_eq!(v.favorability, "even");
        assert!(!v.dodge_consider);
    }
}
