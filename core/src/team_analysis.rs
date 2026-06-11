use super::champion_types::{type_counter_score, ChampionType};
use super::draft_iq::archetype::ChampionArchetype;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct TeamComposition {
    pub tanks: u8,
    pub assassins: u8,
    pub mages: u8,
    pub marksmen: u8,
    pub supports: u8,
    pub fighters: u8,
    pub is_ap_heavy: bool,
    pub is_ad_heavy: bool,
}

impl TeamComposition {
    pub fn from_champion_ids(ids: &[u32], role_map: &HashMap<u32, Vec<String>>) -> Self {
        let mut comp = TeamComposition::default();
        let mut ap_count = 0u8;
        let mut ad_count = 0u8;

        for id in ids {
            let types = role_map
                .get(id)
                .map(|roles| ChampionType::from_roles(roles))
                .unwrap_or_default();

            for t in &types {
                match t {
                    ChampionType::Tank => comp.tanks += 1,
                    ChampionType::Assassin => comp.assassins += 1,
                    ChampionType::Mage => {
                        comp.mages += 1;
                        ap_count += 1;
                    }
                    ChampionType::Marksman => {
                        comp.marksmen += 1;
                        ad_count += 1;
                    }
                    ChampionType::Support => comp.supports += 1,
                    ChampionType::Fighter => {
                        comp.fighters += 1;
                        ad_count += 1;
                    }
                }
            }
        }

        comp.is_ap_heavy = ap_count >= 3;
        comp.is_ad_heavy = ad_count >= 3;
        comp
    }

    /// How well `champ_types` counter this composition.
    /// Returns average counter score across all enemy types.
    pub fn counter_score_for(&self, champ_types: &[ChampionType]) -> f32 {
        if champ_types.is_empty() {
            return 0.3;
        }

        let enemy_types = self.to_type_list();
        if enemy_types.is_empty() {
            return 0.3;
        }

        let total: f32 = champ_types
            .iter()
            .flat_map(|attacker| {
                enemy_types
                    .iter()
                    .map(|defender| type_counter_score(attacker, defender))
            })
            .sum();

        total / (champ_types.len() * enemy_types.len()) as f32
    }

    /// Synergy: champ types that complement gaps in the current composition.
    pub fn synergy_score_for(&self, champ_types: &[ChampionType]) -> f32 {
        let mut score = 0.3f32;
        for ct in champ_types {
            score += match ct {
                ChampionType::Tank if self.tanks == 0 => 0.3,
                ChampionType::Support if self.supports == 0 => 0.2,
                ChampionType::Marksman if self.marksmen == 0 => 0.2,
                ChampionType::Mage if self.mages == 0 => 0.15,
                _ => 0.0,
            };
        }
        score.min(1.0)
    }

    /// Turkish one-liner describing the team composition, e.g. "AP ağırlıklı · frontline yok".
    pub fn summary_text(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.is_ap_heavy {
            parts.push("AP ağırlıklı");
        }
        if self.is_ad_heavy {
            parts.push("AD ağırlıklı");
        }
        if self.tanks == 0 {
            parts.push("frontline yok");
        } else if self.tanks >= 3 {
            parts.push("çok tanklı");
        }
        if self.assassins >= 2 {
            parts.push("assassin ağırlıklı");
        }
        if self.supports == 0 {
            parts.push("support yok");
        }
        if self.marksmen >= 2 {
            parts.push("çift ADC");
        }
        if self.fighters >= 3 {
            parts.push("dövüşçü ağırlıklı");
        }
        if parts.is_empty() {
            "dengeli takım".to_string()
        } else {
            parts.join(" · ")
        }
    }

    fn to_type_list(&self) -> Vec<ChampionType> {
        let mut types = Vec::new();
        for _ in 0..self.tanks {
            types.push(ChampionType::Tank);
        }
        for _ in 0..self.assassins {
            types.push(ChampionType::Assassin);
        }
        for _ in 0..self.mages {
            types.push(ChampionType::Mage);
        }
        for _ in 0..self.marksmen {
            types.push(ChampionType::Marksman);
        }
        for _ in 0..self.supports {
            types.push(ChampionType::Support);
        }
        for _ in 0..self.fighters {
            types.push(ChampionType::Fighter);
        }
        types
    }
}

/// Per-team composition summary for the draft board UI. Combines role counts
/// (from `TeamComposition`) with KB-derived team-utility flags (engage /
/// frontline / hard-CC / peel) and damage shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CompSummary {
    pub tanks: u8,
    pub fighters: u8,
    pub mages: u8,
    pub marksmen: u8,
    pub assassins: u8,
    pub supports: u8,
    /// Average AP / AD damage share across known archetypes (0..1).
    pub ap_share: f32,
    pub ad_share: f32,
    pub has_engage: bool,
    pub has_frontline: bool,
    pub has_hard_cc: bool,
    pub has_peel: bool,
    /// Missing team utilities, e.g. "Engage yok".
    pub gaps: Vec<String>,
    pub summary: String,
}

/// Both teams' composition summaries for the draft board.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TeamCompBoard {
    pub ally: CompSummary,
    pub enemy: CompSummary,
}

/// Build a `CompSummary` for one team. `ids` are the locked champion ids,
/// `role_map` drives the role counts, and `archetypes` (KB) drive the utility
/// flags + damage shares. Empty inputs yield an all-false / "dengeli takım"
/// summary with no gaps (nothing known yet).
pub fn compute_comp_summary(
    ids: &[u32],
    role_map: &HashMap<u32, Vec<String>>,
    archetypes: &[&ChampionArchetype],
) -> CompSummary {
    let comp = TeamComposition::from_champion_ids(ids, role_map);

    let has_engage = archetypes.iter().any(|a| {
        a.engage_role == "initiator" || matches!(a.archetype.as_str(), "vanguard" | "diver")
    });
    let has_frontline = archetypes
        .iter()
        .any(|a| matches!(a.archetype.as_str(), "vanguard" | "juggernaut" | "warden"));
    let has_hard_cc = archetypes.iter().any(|a| a.cc.has_hard_cc);
    let has_peel = archetypes
        .iter()
        .any(|a| matches!(a.peel_capability.as_str(), "medium" | "high"));

    let (ap_share, ad_share) = if archetypes.is_empty() {
        (0.0, 0.0)
    } else {
        let n = archetypes.len() as f32;
        (
            archetypes.iter().map(|a| a.damage_profile.ap).sum::<f32>() / n,
            archetypes.iter().map(|a| a.damage_profile.ad).sum::<f32>() / n,
        )
    };

    // Gaps only make sense once at least one champion is known.
    let mut gaps = Vec::new();
    if !archetypes.is_empty() {
        if !has_engage {
            gaps.push("Engage yok".to_string());
        }
        if !has_frontline {
            gaps.push("Frontline yok".to_string());
        }
        if !has_hard_cc {
            gaps.push("Sert CC yok".to_string());
        }
        if !has_peel {
            gaps.push("Peel/koruma yok".to_string());
        }
    }

    CompSummary {
        tanks: comp.tanks,
        fighters: comp.fighters,
        mages: comp.mages,
        marksmen: comp.marksmen,
        assassins: comp.assassins,
        supports: comp.supports,
        ap_share,
        ad_share,
        has_engage,
        has_frontline,
        has_hard_cc,
        has_peel,
        gaps,
        summary: comp.summary_text(),
    }
}

#[cfg(test)]
mod comp_summary_tests {
    use super::*;
    use crate::draft_iq::archetype::{CcProfile, DamageProfile, PowerCurve};

    fn arch(
        archetype: &str,
        engage: &str,
        peel: &str,
        hard_cc: bool,
        ap: f32,
        ad: f32,
    ) -> ChampionArchetype {
        ChampionArchetype {
            champion_id: 1,
            archetype: archetype.into(),
            damage_profile: DamageProfile {
                ad,
                ap,
                true_damage: 0.0,
            },
            cc: CcProfile {
                has_hard_cc: hard_cc,
                hard_cc_count: if hard_cc { 1 } else { 0 },
                primary_cc: vec![],
            },
            mobility: "medium".into(),
            engage_role: engage.into(),
            peel_capability: peel.into(),
            blind_safety: 0.6,
            execution_difficulty: 3,
            win_condition: "teamfight".into(),
            ult_type: "aoe".into(),
            confidence: "high".into(),
            power_curve: PowerCurve {
                early: 0.6,
                mid: 0.6,
                late: 0.6,
            },
            counters_archetypes: vec![],
            utility_tags: vec![],
        }
    }

    #[test]
    fn empty_team_has_no_gaps() {
        // No champions known yet → no archetype-derived gaps (utility flags all
        // false but we don't surface them as "missing" until something is locked).
        let s = compute_comp_summary(&[], &HashMap::new(), &[]);
        assert!(s.gaps.is_empty());
        assert!(!s.has_engage && !s.has_frontline && !s.has_hard_cc && !s.has_peel);
        assert_eq!(s.ap_share, 0.0);
    }

    #[test]
    fn enchanter_flags_peel_and_lists_engage_gap() {
        let a = arch("enchanter", "none", "high", false, 0.8, 0.1);
        let s = compute_comp_summary(&[1], &HashMap::new(), &[&a]);
        assert!(s.has_peel);
        assert!(!s.has_engage);
        assert!(s.gaps.iter().any(|g| g.contains("Engage")));
        assert!(s.ap_share > s.ad_share);
    }

    #[test]
    fn vanguard_flags_engage_frontline_cc() {
        let a = arch("vanguard", "initiator", "low", true, 0.1, 0.6);
        let s = compute_comp_summary(&[1], &HashMap::new(), &[&a]);
        assert!(s.has_engage && s.has_frontline && s.has_hard_cc);
    }
}
