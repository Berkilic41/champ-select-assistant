use super::archetype::{ChampionArchetype, DamageProfile};

/// Turkish display label for a champion's primary damage output.
pub fn build_damage_profile_label(dmg: &DamageProfile) -> String {
    if dmg.ap > 0.70 {
        "AP burst / kontrol".to_string()
    } else if dmg.ad > 0.70 {
        "AD fiziksel hasar".to_string()
    } else if dmg.true_damage > 0.25 {
        "Gerçek hasar ağırlıklı".to_string()
    } else {
        "Karma hasar".to_string()
    }
}

/// Turkish display label for the champion's primary team role.
pub fn build_team_role_text(archetype: &ChampionArchetype) -> String {
    match archetype.archetype.as_str() {
        "vanguard" => "Frontline engage tank",
        "juggernaut" => "Dayanıklı duelist / frontline",
        "diver" => "Backline'a hızlı dalan diver",
        "skirmisher" => "1v1 duello uzmanı",
        "assassin" => "Pick-odaklı assassin",
        "control_mage" => "Zone + CC kontrol büyücüsü",
        "burst_mage" => "Tek hedefe burst büyücüsü",
        "artillery" => "Uzun menzilli poke büyücüsü",
        "marksman" => "Late-game carry ADC",
        "catcher" => "Pick uzmanı + CC kilidi",
        "enchanter" => "Koruma ve buff enchanter",
        "warden" => "Pasif koruma tankı",
        _ => "Flex rol",
    }
    .to_string()
}

/// Turkish win condition text based on the champion's primary win condition.
pub fn build_win_condition_text(win_condition: &str) -> String {
    match win_condition {
        "teamfight" => "Takım dövüşü komposu — engage + AOE ile galip gel",
        "split_push" => "Lane'leri böl, 1v1 duellolarla baskı kur",
        "pick" => "İzole pick'lerle sayı üstünlüğü sağla",
        "skirmish" => "Küçük karşılaşmalarda sürekli avantaj kazan",
        "poke" => "Uzaktan eziyetle lifeları erittikten sonra obje al",
        "siege" => "Vision + turret baskısıyla alan kontrolü kur",
        "protect" => "Hypercarry'yi koru, late-game'e güvenle götür",
        _ => "Güçlü oyun planıyla karşı takımı geride bırak",
    }
    .to_string()
}

/// Derive Turkish threat strings based on candidate vs enemy compositions.
pub fn build_threats_text(
    candidate: &ChampionArchetype,
    enemy_archetypes: &[&ChampionArchetype],
    ally_archetypes: &[&ChampionArchetype],
) -> Vec<String> {
    let mut threats = Vec::new();

    // Hard CC flood: squishy archetypes need peel/tenacity
    let is_squishy = matches!(
        candidate.archetype.as_str(),
        "assassin" | "burst_mage" | "artillery" | "marksman"
    );
    let enemy_cc_count: u32 = enemy_archetypes
        .iter()
        .map(|e| e.cc.hard_cc_count as u32)
        .sum();
    if is_squishy && enemy_cc_count >= 3 {
        threats.push("Çoklu hard CC'ye karşı kırılgan — peel veya tenacity gerekli".to_string());
    }

    // Low mobility vs strong engage initiators
    let enemy_has_engage = enemy_archetypes
        .iter()
        .any(|e| e.engage_role == "initiator" && e.cc.has_hard_cc);
    if enemy_has_engage && matches!(candidate.mobility.as_str(), "none" | "low") {
        threats.push("Düşman engage'ine karşı kaçış imkânı yok".to_string());
    }

    // High execution difficulty reminder
    if candidate.execution_difficulty >= 4 {
        threats.push(format!(
            "Yüksek execution riski (zorluk {}/5) — stresten etkilenen oyunlarda dikkat",
            candidate.execution_difficulty
        ));
    }

    // Power-curve temporal mismatch: late-scaling champ into an early-heavy enemy comp
    let pc = &candidate.power_curve;
    let enemy_avg_early: f32 = if enemy_archetypes.is_empty() {
        0.0
    } else {
        enemy_archetypes
            .iter()
            .map(|e| e.power_curve.early)
            .sum::<f32>()
            / enemy_archetypes.len() as f32
    };
    if pc.late >= 0.75 && pc.early <= 0.40 && enemy_avg_early >= 0.70 {
        threats.push(
            "Late-game scaling — düşman erken baskı yapar, 15. dk öncesi dikkatli oyna".to_string(),
        );
    }

    // ── B5: Richer threat detection ───────────────────────────────────────

    let is_immobile_carry = matches!(
        candidate.archetype.as_str(),
        "marksman" | "artillery" | "enchanter"
    ) && matches!(candidate.mobility.as_str(), "none" | "low");

    // Tower-dive risk: immobile carry vs enemy dive comp (≥2 divers/juggernauts)
    let enemy_dive_count = enemy_archetypes
        .iter()
        .filter(|e| matches!(e.archetype.as_str(), "diver" | "juggernaut"))
        .count();
    if is_immobile_carry && enemy_dive_count >= 2 {
        threats.push(
            "Tower-dive riski — düşman dive komposu, lvl 6 sonrası turret altında bile güvende değilsin"
                .to_string(),
        );
    }

    // Pick-comp vs immobile carry: warding & isolation become dangerous
    let enemy_pick_count = enemy_archetypes
        .iter()
        .filter(|e| matches!(e.archetype.as_str(), "assassin" | "catcher"))
        .count();
    if is_immobile_carry && enemy_pick_count >= 2 {
        threats.push(
            "Pick-comp izole pozisyonları cezalandırır — guardian/swiftness şart, takımdan ayrılma"
                .to_string(),
        );
    }

    // Splitpush vulnerability: enemy has split threat AND team lacks waveclear
    let enemy_split_threat = enemy_archetypes
        .iter()
        .any(|e| matches!(e.archetype.as_str(), "juggernaut" | "skirmisher"));
    let team_has_waveclear = candidate.utility_tags.iter().any(|t| t == "waveclear")
        || ally_archetypes
            .iter()
            .any(|a| a.utility_tags.iter().any(|t| t == "waveclear"));
    if enemy_split_threat && !team_has_waveclear && !ally_archetypes.is_empty() {
        threats.push(
            "Rakipte split baskısı var — takımda waveclear eksik, side lane'leri korumak zor"
                .to_string(),
        );
    }

    // Outscaled: early-dominant candidate vs late-scaling enemy comp
    let enemy_avg_late: f32 = if enemy_archetypes.is_empty() {
        0.0
    } else {
        enemy_archetypes
            .iter()
            .map(|e| e.power_curve.late)
            .sum::<f32>()
            / enemy_archetypes.len() as f32
    };
    if pc.early >= 0.70 && pc.late <= 0.55 && enemy_avg_late >= 0.75 {
        threats.push(
            "Düşman seni geç oyunda geçer — erken avantajı drake/turret ile somut hale getir"
                .to_string(),
        );
    }

    threats
}

/// Returns a Turkish power-spike / item-breakpoint advisory for the candidate.
/// Derived from `power_curve` + `archetype` + `win_condition` without external data.
/// Returns `None` when the champion has no standout spike profile.
pub fn build_spike_note(archetype: &ChampionArchetype) -> Option<String> {
    let pc = &archetype.power_curve;
    let wc = archetype.win_condition.as_str();
    let a = archetype.archetype.as_str();

    // Early-dominant: wins the lane phase before items
    if pc.early >= 0.70 && pc.late <= 0.65 {
        return Some(
            "Erken baskı penceresi geniş — ilk item öncesi avantaj al, geç oyuna uzatma"
                .to_string(),
        );
    }

    // Late scaler: needs 2–3 items to hit power spike
    if pc.late >= 0.80 && pc.early <= 0.50 {
        return Some(match wc {
            "protect" => {
                "Hypercarry — 2 item tamamlayınca dominant, carry'yi geç oyuna güvenle taşı"
                    .to_string()
            }
            "split_push" => {
                "2. item sonrası 1v2 duellolar kazanılır — geç split baskısı başlat".to_string()
            }
            _ => "2–3 item sonrası dominant — erken baskıyı savun, geç oyuna ulaş".to_string(),
        });
    }

    // Mid-game spike: peaks between first and second item
    if pc.mid >= 0.80 {
        return Some(match a {
            "assassin" | "diver" => {
                "1. item + lvl 6 = pick penceresi — roam ve gank fırsatlarını hemen değerlendir"
                    .to_string()
            }
            "control_mage" | "battle_mage" => {
                "Orta oyun item spike'ı — first item sonrası aktif zone kontrolü kur".to_string()
            }
            "skirmisher" => {
                "1–2 item arası doruk nokta — düşmanı duel'e çek, obje öncesi baskı kur".to_string()
            }
            _ => "Orta oyun item spike'ı — first item tamamlanınca aktivite başlat".to_string(),
        });
    }

    // Artillery: sustained poke before full-fights
    if a == "artillery" {
        return Some(
            "Poke item döngüsünü tamamla, hasar farkı oluşmadan büyük dövüşe girme".to_string(),
        );
    }

    // Enchanter scales with carry's items
    if a == "enchanter" {
        return Some(
            "Carry 2. item tamamlayınca hazır — mana + ward kapasiteni erken döngüye sok"
                .to_string(),
        );
    }

    None
}

/// Returns a Turkish lane-phase micro-coaching advisory (0-15 dakika oyunu).
/// Combines power-curve matchup (early bully vs scaler) with position+archetype
/// patterns. Returns `None` for ARAM (`position == ""`) or when no specific
/// pattern matches (caller can show nothing or a generic line).
///
/// Priority order:
/// 1. Matchup-aware: candidate vs lane opponent power-curve mismatch
/// 2. Position + archetype combo (top/jungle/middle/bottom/utility)
/// 3. None
pub fn build_lane_phase_advice(
    candidate: &ChampionArchetype,
    lane_opponent: Option<&ChampionArchetype>,
    position: &str,
) -> Option<String> {
    // No lane phase in ARAM / Arena
    if position.is_empty() {
        return None;
    }

    let pc = &candidate.power_curve;
    let pos_lc = position.to_lowercase();

    // ── Matchup-aware advice (priority over position-only patterns) ──────────
    if let Some(opp) = lane_opponent {
        let opp_pc = &opp.power_curve;

        // Early bully vs late scaler: aggressive push window
        if pc.early >= 0.70 && opp_pc.early <= 0.50 {
            return Some(
                "Lvl 1-3 push penceresi — düşman ölçekleniyor, ilk plate (14dk) için baskı kur"
                    .to_string(),
            );
        }

        // Late scaler vs early bully: freeze + survive
        if pc.early <= 0.50 && opp_pc.early >= 0.70 {
            return Some(
                "Tower altında freeze — lvl 6 öncesi short trade'lerden kaçın, jungler gank pencerelerini bekle"
                    .to_string(),
            );
        }

        // Both early bullies: lvl 2 race
        if pc.early >= 0.70 && opp_pc.early >= 0.70 {
            return Some(
                "Lvl 2 race kritik — ilk dalga 6 melee minion'a hızlı bas, lvl 2 all-in penceresi"
                    .to_string(),
            );
        }
    }

    // ── Position + archetype combos ──────────────────────────────────────────
    let a = candidate.archetype.as_str();
    let line = match (pos_lc.as_str(), a) {
        ("top", "juggernaut" | "vanguard") => {
            "Wave control: slow push lvl 3, jungler TP / gank için tower yakını freeze setup"
        }
        ("top", "skirmisher" | "diver") => {
            "Lvl 6 all-in — düşman jungle quadrant'ı ward, flash + ult combo'ya hazır ol"
        }
        ("middle", "assassin" | "burst_mage") => {
            "Lvl 6 roam timer — mid wave push sonrası bot/top gank, summoner takip et"
        }
        ("middle", "control_mage" | "battle_mage") => {
            "Zone control: enemy XP'sini kes, dalga dondurarak roam fırsatı yarat"
        }
        ("jungle", "skirmisher" | "diver" | "assassin") => {
            "Scuttle priority lvl 3 — agresif invade, lane gank lvl 4-5 hazır"
        }
        ("jungle", "vanguard" | "juggernaut") => {
            "Farm-focused jungle: full clear → lvl 6 obje (drake/herald) priority"
        }
        ("bottom", "marksman") => {
            "Plate timer 14dk öncesi push, 2v2 trade'lerde support follow-up bekle"
        }
        ("bottom", "artillery") => {
            "Uzun menzilden poke + farm, all-in trade'lerden kaçın, lvl 6 spike sonrası aktif"
        }
        ("utility", "enchanter") => {
            "Ward timing 3 / 5 / 7dk; ADC trade'lerine W/E follow-up, lvl 6 sonrası roam"
        }
        ("utility", "catcher" | "vanguard") => {
            "Lvl 2 hook penceresi — 2. dalga push, all-in için fog of war kullan"
        }
        _ => return None,
    };
    Some(line.to_string())
}

/// Classify enemy team's dominant win condition based on archetype distribution.
/// Returns one of: "pick" | "teamfight" | "protect" | "poke" | "split" | "mixed".
/// Requires ≥ 2 archetypes in a category to classify (avoids single-champion noise).
pub fn detect_enemy_win_condition(enemy_archetypes: &[&ChampionArchetype]) -> &'static str {
    if enemy_archetypes.is_empty() {
        return "mixed";
    }
    let mut pick = 0u32;
    let mut teamfight = 0u32;
    let mut protect = 0u32;
    let mut poke = 0u32;
    let mut split = 0u32;

    for e in enemy_archetypes {
        match e.archetype.as_str() {
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
    let (best_label, best_count) = scores
        .iter()
        .max_by_key(|(_, c)| c)
        .copied()
        .unwrap_or(("mixed", 0));

    if best_count < 2 {
        "mixed"
    } else {
        best_label
    }
}

/// Returns a Turkish clash-advisory note when the candidate's win condition
/// creates a structural tension against the detected enemy comp. Returns `None`
/// when there is no notable clash (neutral matchups need no special note).
pub fn build_comp_clash_note(my_wc: &str, enemy_wc: &str) -> Option<String> {
    let note = match (my_wc, enemy_wc) {
        ("poke" | "siege", "pick") => "Pick-comp'e karşı poke: vision basın, izole durmayın",
        ("poke" | "siege", "teamfight") => {
            "Engage'e karşı poke: dağınık durun, uzak engage'e hazır ol"
        }
        ("split_push", "protect") => "Protect-comp'e karşı split: 2-2-1 / 3-1-1 ile uğraştır",
        ("protect", "pick") => {
            "Pick-comp, carry'nizi izole hedef alır: yakın dur, ward takibi şart"
        }
        ("teamfight", "poke") => {
            "Poke'a karşı teamfight: vision açarak engage aç, uzun lane'den kaçın"
        }
        ("teamfight", "split" | "split_push") => {
            "Split-push'a karşı teamfight: objeyi takip et, 4 grupla"
        }
        ("pick", "teamfight") => {
            "Teamfight'a karşı pick: 5v5 kaçın, pick fırsatı sonrası objeye geç"
        }
        ("pick", "protect") => {
            "Protect-comp karşı pick: ward bazlı flank aç, carry'yi savunmasız yakala"
        }
        _ => return None,
    };
    Some(note.to_string())
}
