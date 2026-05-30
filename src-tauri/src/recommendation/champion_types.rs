/// Broad champion archetype used for matchup scoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChampionType {
    Fighter,
    Mage,
    Assassin,
    Tank,
    Marksman,
    Support,
}

impl ChampionType {
    /// Map any archetype string — coarse DDragon roles ("fighter", "mage" …) **or**
    /// fine-grained KB archetypes ("juggernaut", "burst_mage", "vanguard" …) — to
    /// the 6 broad `ChampionType` variants used for scoring.
    ///
    /// This is the single source of truth for both taxonomies so that DDragon
    /// `role_map` data and `champions.json` archetype strings always resolve
    /// consistently.
    pub fn from_archetype(archetype: &str) -> Self {
        match archetype.to_lowercase().as_str() {
            // ── Coarse DDragon roles ──────────────────────────────────────────
            "fighter" => ChampionType::Fighter,
            "mage" => ChampionType::Mage,
            "assassin" => ChampionType::Assassin,
            "tank" => ChampionType::Tank,
            "marksman" => ChampionType::Marksman,
            "support" => ChampionType::Support,
            // ── KB fine-grained archetypes (champions.json) ──────────────────
            // Fighters: melee carry / sustained damage
            "juggernaut" | "skirmisher" => ChampionType::Fighter,
            // Fighter-leaning divers also get Fighter (they engage like tanks but
            // deal Fighter-level damage; type_counter_score treats them as Fighter)
            "diver" => ChampionType::Fighter,
            // Mages: all spell-caster sub-classes
            "battle_mage" | "burst_mage" | "control_mage" | "artillery" => ChampionType::Mage,
            // Tanks: protective / front-line
            "vanguard" | "warden" => ChampionType::Tank,
            // Supports: heal/shield or catch/lockdown
            "enchanter" | "catcher" => ChampionType::Support,
            // Unknown strings fall back to Fighter (the most neutral melee type)
            _ => ChampionType::Fighter,
        }
    }

    pub fn from_roles(roles: &[String]) -> Vec<Self> {
        roles.iter().map(|r| Self::from_archetype(r)).collect()
    }

    #[allow(dead_code)]
    pub fn is_melee(&self) -> bool {
        matches!(
            self,
            ChampionType::Fighter | ChampionType::Tank | ChampionType::Assassin
        )
    }
}

/// Returns a counter advantage score [0.0, 1.0] for `attacker` vs `defender`.
/// Higher = attacker has more advantage against defender.
pub fn type_counter_score(attacker: &ChampionType, defender: &ChampionType) -> f32 {
    use ChampionType::*;
    match (attacker, defender) {
        (Assassin, Mage) => 0.80,
        (Assassin, Marksman) => 0.75,
        (Tank, Assassin) => 0.70,
        (Mage, Tank) => 0.60,
        (Support, Assassin) => 0.60,
        (Marksman, Tank) => 0.55,
        (Fighter, Tank) => 0.50,
        _ => 0.30,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assassin_beats_mage() {
        assert!(type_counter_score(&ChampionType::Assassin, &ChampionType::Mage) > 0.5);
    }

    // ── from_archetype: coarse DDragon roles ──────────────────────────────────

    #[test]
    fn from_archetype_coarse_roles_map_correctly() {
        assert_eq!(
            ChampionType::from_archetype("fighter"),
            ChampionType::Fighter
        );
        assert_eq!(ChampionType::from_archetype("mage"), ChampionType::Mage);
        assert_eq!(
            ChampionType::from_archetype("assassin"),
            ChampionType::Assassin
        );
        assert_eq!(ChampionType::from_archetype("tank"), ChampionType::Tank);
        assert_eq!(
            ChampionType::from_archetype("marksman"),
            ChampionType::Marksman
        );
        assert_eq!(
            ChampionType::from_archetype("support"),
            ChampionType::Support
        );
    }

    // ── from_archetype: KB fine-grained archetypes ───────────────────────────

    #[test]
    fn from_archetype_fighter_subtypes() {
        assert_eq!(
            ChampionType::from_archetype("juggernaut"),
            ChampionType::Fighter
        );
        assert_eq!(
            ChampionType::from_archetype("skirmisher"),
            ChampionType::Fighter
        );
        assert_eq!(ChampionType::from_archetype("diver"), ChampionType::Fighter);
    }

    #[test]
    fn from_archetype_mage_subtypes() {
        assert_eq!(
            ChampionType::from_archetype("battle_mage"),
            ChampionType::Mage
        );
        assert_eq!(
            ChampionType::from_archetype("burst_mage"),
            ChampionType::Mage
        );
        assert_eq!(
            ChampionType::from_archetype("control_mage"),
            ChampionType::Mage
        );
        assert_eq!(
            ChampionType::from_archetype("artillery"),
            ChampionType::Mage
        );
    }

    #[test]
    fn from_archetype_tank_subtypes() {
        assert_eq!(ChampionType::from_archetype("vanguard"), ChampionType::Tank);
        assert_eq!(ChampionType::from_archetype("warden"), ChampionType::Tank);
    }

    #[test]
    fn from_archetype_support_subtypes() {
        assert_eq!(
            ChampionType::from_archetype("enchanter"),
            ChampionType::Support
        );
        assert_eq!(
            ChampionType::from_archetype("catcher"),
            ChampionType::Support
        );
    }

    #[test]
    fn from_archetype_unknown_defaults_to_fighter() {
        assert_eq!(
            ChampionType::from_archetype("totally_unknown"),
            ChampionType::Fighter
        );
    }

    #[test]
    fn from_roles_maps_kb_archetypes_in_slice() {
        let roles = vec!["burst_mage".to_string(), "assassin".to_string()];
        let types = ChampionType::from_roles(&roles);
        assert_eq!(types, vec![ChampionType::Mage, ChampionType::Assassin]);
    }

    // ── type_counter_score: key matchup assertions ────────────────────────────

    #[test]
    fn counter_scores_canonical() {
        use ChampionType::*;
        // Assassin hard-counters Mage/Marksman
        assert_eq!(type_counter_score(&Assassin, &Mage), 0.80);
        assert_eq!(type_counter_score(&Assassin, &Marksman), 0.75);
        // Tank shuts down Assassin
        assert_eq!(type_counter_score(&Tank, &Assassin), 0.70);
        // Symmetric fallback is below 0.5
        assert!(type_counter_score(&Fighter, &Fighter) < 0.5);
    }
}
