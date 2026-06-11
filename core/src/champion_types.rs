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

/// Conservative KB-archetype → lane fit. Returns `true` when a champion of this
/// fine-grained archetype is commonly/plausibly played in `position` (lowercase
/// LCU position). Used to AVOID demoting valid off-meta-role picks (e.g. a mage
/// support like Brand, or an APC mid/bot) that the coarse CDragon role table
/// would otherwise flag as "off-role".
///
/// Unknown archetypes return `false` so the caller falls back to the CDragon
/// role check alone (no special exemption).
pub fn archetype_position_fit(archetype: &str, position: &str) -> bool {
    let pos = position.to_lowercase();
    match archetype.to_lowercase().as_str() {
        "enchanter" | "catcher" | "warden" => pos == "utility",
        // Mages flex across mid / support (APC) / bot (APC).
        "control_mage" | "burst_mage" | "battle_mage" => {
            pos == "middle" || pos == "utility" || pos == "bottom"
        }
        "artillery" => pos == "middle" || pos == "bottom" || pos == "utility",
        "marksman" => pos == "bottom",
        "assassin" => pos == "middle" || pos == "jungle",
        "skirmisher" | "diver" => pos == "top" || pos == "jungle" || pos == "middle",
        "juggernaut" => pos == "top" || pos == "jungle",
        "vanguard" => pos == "top" || pos == "jungle" || pos == "utility",
        _ => false,
    }
}

/// Extra lane positions for well-known flex picks whose single KB archetype is too
/// narrow. `archetype_position_fit` is correct for the ~120 mono-role champions but
/// infers role purely from archetype, so it wrongly excludes champions played in a
/// role their archetype doesn't imply — e.g. Senna (`marksman` → only bottom) is
/// mostly a support, Kindred (`marksman`) is a jungler, Galio (`vanguard`) is mid,
/// the `specialist` archetype (Teemo/Gnar/Heimerdinger) maps to nothing at all.
///
/// This **augments** (never narrows) the archetype fit: the role filters accept a
/// champion when meta data, archetype fit, **or** this override allows the lane. It
/// is keyed by the stable Riot `champion_id` (all three filter call-sites have it).
/// Only the position(s) NOT already covered by `archetype_position_fit` are listed,
/// to keep the map a precise, auditable supplement. Positions are standard public
/// primary/secondary roles (u.gg / lolalytics), not invented. Mono-role champions
/// (e.g. Olaf, Master Yi, Sion) are intentionally absent → they stay role-gated.
pub fn flex_positions(champion_id: u32) -> &'static [&'static str] {
    match champion_id {
        3 => &["middle"],                    // Galio — vanguard, mainly mid
        8 => &["top"],                       // Vladimir — control_mage, also top
        9 => &["jungle"],                    // Fiddlesticks — burst_mage, jungler
        10 => &["top", "middle"],            // Kayle — marksman tag, top/mid
        17 => &["top", "middle"],            // Teemo — specialist
        29 => &["jungle"],                   // Twitch — marksman, also jungle
        30 => &["jungle"],                   // Karthus — artillery, jungler
        35 => &["utility"],                  // Shaco — assassin, also support
        42 => &["middle"],                   // Corki — marksman, mainly mid
        43 => &["middle"],                   // Karma — enchanter, also mid
        50 => &["top"],                      // Swain — control_mage, also top
        60 => &["jungle"],                   // Elise — catcher, jungler
        67 => &["top"],                      // Vayne — marksman, also top
        74 => &["top", "middle", "utility"], // Heimerdinger — specialist
        79 => &["middle"],                   // Gragas — vanguard, also mid
        80 => &["utility"],                  // Pantheon — diver, also support
        84 => &["top"],                      // Akali — assassin, also top
        85 => &["top"],                      // Kennen — burst_mage, mainly top
        107 => &["top"],                     // Rengar — assassin, also top
        133 => &["top"],                     // Quinn — marksman, mainly top
        147 => &["middle"],                  // Seraphine — enchanter, also mid
        150 => &["top"],                     // Gnar — specialist
        157 => &["bottom"],                  // Yasuo — skirmisher, also bot
        163 => &["jungle"],                  // Taliyah — burst_mage, also jungle
        166 => &["middle"],                  // Akshan — marksman, mainly mid
        203 => &["jungle"],                  // Kindred — marksman, jungler
        235 => &["utility"],                 // Senna — marksman, also support
        427 => &["jungle"],                  // Ivern — enchanter, jungler
        777 => &["bottom"],                  // Yone — skirmisher, also bot
        875 => &["utility"],                 // Sett — juggernaut, also support
        895 => &["bottom"],                  // Nilah — skirmisher, bot-lane
        _ => &[],
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

    // ── archetype_position_fit ────────────────────────────────────────────────

    #[test]
    fn mage_fits_support_lane() {
        // Brand-like burst mage support should NOT be flagged off-role at utility.
        assert!(archetype_position_fit("burst_mage", "utility"));
        assert!(archetype_position_fit("control_mage", "utility"));
        assert!(archetype_position_fit("enchanter", "utility"));
    }

    #[test]
    fn fighter_does_not_fit_mid() {
        assert!(!archetype_position_fit("juggernaut", "middle"));
        assert!(!archetype_position_fit("marksman", "middle"));
        assert!(!archetype_position_fit("warden", "middle"));
    }

    #[test]
    fn marksman_fits_bottom_only() {
        assert!(archetype_position_fit("marksman", "bottom"));
        assert!(!archetype_position_fit("marksman", "top"));
    }

    #[test]
    fn unknown_archetype_has_no_fit() {
        assert!(!archetype_position_fit("totally_unknown", "middle"));
    }

    // ── flex_positions ────────────────────────────────────────────────────────

    #[test]
    fn flex_positions_restore_off_archetype_roles() {
        // Senna is a `marksman` (archetype → bottom only) but mostly a support.
        assert!(flex_positions(235).contains(&"utility"));
        // Specialists map to nothing in archetype_position_fit → must be restored.
        assert!(flex_positions(17).contains(&"top")); // Teemo
        assert!(flex_positions(74).contains(&"middle")); // Heimerdinger
        assert!(flex_positions(74).contains(&"utility"));
        // Galio is a `vanguard` (top/jungle/utility) but mainly mid.
        assert!(flex_positions(3).contains(&"middle"));
        // Kindred is a `marksman` but a jungler.
        assert!(flex_positions(203).contains(&"jungle"));
        // Yasuo/Yone skirmishers also flex bottom.
        assert!(flex_positions(157).contains(&"bottom"));
    }

    #[test]
    fn flex_positions_empty_for_mono_role_champions() {
        // Mono-role champions stay strictly archetype-gated (A+B fix preserved):
        assert!(flex_positions(2).is_empty()); // Olaf (juggernaut → top/jungle)
        assert!(flex_positions(11).is_empty()); // Master Yi (skirmisher)
        assert!(flex_positions(14).is_empty()); // Sion (vanguard)
        assert!(flex_positions(99_999).is_empty()); // unknown id
    }
}
