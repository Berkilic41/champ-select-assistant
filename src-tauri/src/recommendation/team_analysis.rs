use super::champion_types::{type_counter_score, ChampionType};
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
