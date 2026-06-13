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

/// Mid-game (≈ first item → 25 min) macro advice from the champion's archetype — what
/// this champion should be DOING after the lane phase. KB-grounded, measured.
pub fn build_mid_game_advice(candidate: &ChampionArchetype) -> String {
    match candidate.archetype.as_str() {
        "assassin" | "diver" => {
            "Lvl 6+ roam: yan lane'lere bas, izole hedefleri yakala, jungle tempoya katıl"
        }
        "control_mage" | "battle_mage" => {
            "Obje etrafında zone kur: vision + büyü baskısıyla alanı kapat, koridorları tut"
        }
        "artillery" => "Obje öncesi poke biriktir, menzilini kullan, yakın dövüşten kaç",
        "marksman" => {
            "Güvenli farmla 2-3 item spike'ına ulaş, obje dövüşlerinde arkadan sürekli hasar"
        }
        "enchanter" | "warden" => {
            "Carry'ni koru, vision kur, obje setup'ını hazırla, peel'e hazır ol"
        }
        "vanguard" | "juggernaut" => "Frontline ol, engage penceresini bekle, drake/herald'ı zorla",
        "skirmisher" => {
            "Side lane baskısı kur, skirmish'lerde tempo al, 1v1 fırsatlarını değerlendir"
        }
        "catcher" => "Vision + pick: fog of war'da bekle, hook/CC ile sayı üstünlüğü yarat",
        _ => "Obje etrafında grupla, vision al, güç eğrine göre tempo ayarla",
    }
    .to_string()
}

/// Late-game (≈ 25 min+) advice keyed by the champion's win condition — how to close.
pub fn build_late_game_advice(win_condition: &str) -> String {
    match win_condition {
        "teamfight" => {
            "5v5: doğru hedef için pozisyon al, ana cooldown'ı takım CC'siyle aynı pencereye sakla"
        }
        "split_push" => "Side lane it (1-3-1), haritayı böl, TP'yi obje dövüşü için hazır tut",
        "pick" => "Vision'la flank, izole hedefi yakala → hemen Baron/objeye geç",
        "skirmish" => "Sürekli küçük dövüşlerle avantajı büyüt, dağınık 5v5'ten kaçın",
        "poke" | "siege" => {
            "Obje öncesi poke biriktir, sağlık avantajıyla dövüşü başlat, kuşatmayı sürdür"
        }
        "protect" => "Hypercarry'nin etrafında dövüş, onu peel'le, tüm kaynakları ona ak",
        _ => "Sayı üstünlüğü olmadan obje başlatma, vision kontrolüyle kapat",
    }
    .to_string()
}

/// A short, honest rationale for the recommended build — grounds it in the champion's
/// archetype intent + the lane opponent's damage type (early itemization). NOT a
/// winrate claim: `general` (heuristic) builds are hedged as a sensible template.
pub fn build_rationale(
    candidate: &ChampionArchetype,
    lane_opponent_archetype: Option<&str>,
    build_source: &str,
) -> String {
    let core = match candidate.archetype.as_str() {
        "burst_mage" | "control_mage" | "battle_mage" => "büyü gücü + sihirsel penetrasyon",
        "artillery" => "menzil + sihirsel penetrasyon (poke)",
        "marksman" => "saldırı hızı + crit/lethality",
        "assassin" | "diver" => "lethality + burst",
        "skirmisher" => "on-hit / bruiser karışımı",
        "juggernaut" | "vanguard" | "warden" => "dayanıklılık + hasar",
        "catcher" | "enchanter" => "mana + ekip utility",
        _ => "dengeli çekirdek",
    };
    let mut s = if build_source == "general" {
        format!("Çekirdek: {core} (genel şablon)")
    } else {
        format!("Çekirdek: {core}")
    };

    // Early-itemization hint from the lane opponent's damage type.
    if let Some(opp_arch) = lane_opponent_archetype {
        let counter = match opp_arch {
            "burst_mage" | "control_mage" | "battle_mage" | "artillery" => {
                Some("rakip AP → erken magic resist değerli")
            }
            "assassin" | "marksman" | "skirmisher" | "diver" => {
                Some("rakip AD → erken zırh değerli")
            }
            "juggernaut" | "vanguard" | "warden" => Some("rakip dayanıklı → penetrasyon düşün"),
            _ => None,
        };
        if let Some(c) = counter {
            s.push_str("; ");
            s.push_str(c);
        }
    }
    s.push('.');
    s
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

/// Minimum power-curve farkı: bunun altındaki farklardan pencere okuması üretmeyiz
/// (gürültüden ders çıkarmak dürüst değil).
const SPIKE_WINDOW_DIFF: f32 = 0.15;

/// Matchup'a özel güç penceresi: iki şampiyonun `power_curve`'ü karşılaştırılır,
/// belirgin (≥0.15) faz farkı varsa aksiyona dönük Türkçe okuma döner. Yalnız
/// KB'deki sayısal eğriden türetilir (mekanik uydurma yok); fark yoksa `None`.
pub fn build_matchup_spike_window(
    mine: &ChampionArchetype,
    opp: &ChampionArchetype,
) -> Option<String> {
    let m = &mine.power_curve;
    let o = &opp.power_curve;
    let early = m.early - o.early;
    let late = m.late - o.late;

    // Çapraz pencereler önce: en aksiyona dönük iki okuma.
    if early >= SPIKE_WINDOW_DIFF && late <= -SPIKE_WINDOW_DIFF {
        return Some(
            "Güç penceren erken: laning'de baskıyı kur ve avantajı objeye çevir — oyun uzadıkça rakip öne geçer"
                .to_string(),
        );
    }
    if early <= -SPIKE_WINDOW_DIFF && late >= SPIKE_WINDOW_DIFF {
        return Some(
            "Rakibin penceresi erken: laning'i kayıpsız atlat — item'ler tamamlandıkça güç sana geçer"
                .to_string(),
        );
    }
    if early >= SPIKE_WINDOW_DIFF {
        return Some(
            "Erken oyun farkı sende — ilk dakikalardan tempo kur, kazandığın avantajı büyüt"
                .to_string(),
        );
    }
    if early <= -SPIKE_WINDOW_DIFF {
        return Some(
            "Erken oyun rakipte — ilk dakikalarda gereksiz takas alma, kayıpsız farm'a odaklan"
                .to_string(),
        );
    }
    if late >= SPIKE_WINDOW_DIFF {
        return Some("Geç oyun senin lehine — eşit giden oyunu uzatmak kazandırır".to_string());
    }
    if late <= -SPIKE_WINDOW_DIFF {
        return Some(
            "Geç oyun rakipte — avantajı orta oyunda objeye çevir, oyunu uzatma".to_string(),
        );
    }
    None // belirgin pencere farkı yok — uydurma okuma üretmeyiz
}

/// Opponent-archetype-specific lane caution. Uses only the KB archetype class we hold
/// (no per-ability mechanics → honest, no fabrication). Lets two early bullies of
/// different classes get distinct advice when combined with the power-curve tempo line.
fn opponent_threat_note(opponent_archetype: &str) -> Option<&'static str> {
    let note = match opponent_archetype {
        "assassin" => "lvl 6 sonrası all-in ve roam penceresine karşı pozisyon al",
        "burst_mage" => "kombo menziline girmeden CS al, kısa takasla XP eşitle",
        "control_mage" | "battle_mage" => {
            "uzun lane kontrolüne karşı dalga yönetimini önceliklendir"
        }
        "artillery" => "poke birikimine izin verme, all-in penceresini zorla",
        "juggernaut" => "uzun all-in verme; kite edip kısa takasla aşındır",
        "diver" | "skirmisher" => "kısa takasta cooldown eşitliğini koru, izole yakalanma",
        "vanguard" | "catcher" => "engage menzilinde durma, fog of war kontrolü tut",
        "marksman" => "2v2 tempo penceresinde support follow-up'ı bekle",
        "enchanter" | "warden" => "sustain'i aşmak için all-in zamanlamasını planla",
        _ => return None,
    };
    Some(note)
}

/// Returns a Turkish lane-phase micro-coaching advisory (0-15 dakika oyunu).
/// Combines power-curve matchup (early bully vs scaler) with position+archetype
/// patterns. Returns `None` for ARAM (`position == ""`) or when no specific
/// pattern matches (caller can show nothing or a generic line).
///
/// Priority order:
///
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

        // Power-curve tempo line (early bully push / late scaler freeze / lvl 2 race).
        let tempo: Option<&str> = if pc.early >= 0.70 && opp_pc.early <= 0.50 {
            Some("Lvl 1-3 push penceresi — düşman ölçekleniyor, ilk plate (14dk) için baskı kur")
        } else if pc.early <= 0.50 && opp_pc.early >= 0.70 {
            Some("Tower altında freeze — lvl 6 öncesi short trade'lerden kaçın, jungler gank pencerelerini bekle")
        } else if pc.early >= 0.70 && opp_pc.early >= 0.70 {
            Some("Lvl 2 race kritik — ilk dalga 6 melee minion'a hızlı bas, lvl 2 all-in penceresi")
        } else {
            None
        };

        // Deepen with an opponent-archetype-specific caution so two different early
        // bullies (e.g. juggernaut vs assassin) get tailored advice — archetype class
        // only, no fabricated ability mechanics.
        if let Some(tempo) = tempo {
            return Some(match opponent_threat_note(&opp.archetype) {
                Some(note) => format!("{tempo}; {note}"),
                None => tempo.to_string(),
            });
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

/// Rich lane-matchup tips for `candidate` vs `opponent` at `position`. Combines
/// the lane-phase advice with power-curve, mobility/CC and archetype-counter
/// relationships into up to 4 concrete tips. Measured language.
pub fn build_matchup_tips(
    candidate: &ChampionArchetype,
    opponent: &ChampionArchetype,
    position: &str,
) -> Vec<String> {
    let mut tips: Vec<String> = Vec::new();

    if let Some(adv) = build_lane_phase_advice(candidate, Some(opponent), position) {
        tips.push(adv);
    }

    let cpc = &candidate.power_curve;
    let opc = &opponent.power_curve;
    if cpc.early >= 0.70 && opc.early <= 0.55 {
        tips.push(
            "Erken oyunda öndesin — lvl 1-3 agresif bas, ölçeklenmesine izin verme.".to_string(),
        );
    } else if cpc.early <= 0.55 && opc.early >= 0.70 {
        tips.push(
            "Rakip erken güçlü — freeze/güvenli farmla, lvl 6 ve item spike'ını bekle.".to_string(),
        );
    }

    if matches!(candidate.mobility.as_str(), "none" | "low")
        && (opponent.engage_role == "initiator" || opponent.cc.hard_cc_count >= 2)
    {
        tips.push(
            "Düşük mobilite + rakipte sert CC — izole durma, ward bas, flash'ı engage'e sakla."
                .to_string(),
        );
    }

    if candidate.cc.hard_cc_count >= 2 && matches!(opponent.mobility.as_str(), "none" | "low") {
        tips.push("CC üstünlüğün var — all-in'de crowd control'ü zincirle.".to_string());
    }

    if candidate
        .counters_archetypes
        .iter()
        .any(|a| a == &opponent.archetype)
    {
        tips.push(
            "Arketip olarak rakibe avantajlısın — uygun pencerelerde baskıyı sürdür.".to_string(),
        );
    } else if opponent
        .counters_archetypes
        .iter()
        .any(|a| a == &candidate.archetype)
    {
        tips.push(
            "Rakip arketibi seni sayar — yanlış pozisyon verme, takasları seçici yap.".to_string(),
        );
    }

    if candidate.execution_difficulty >= 4 {
        tips.push(
            "Yüksek execution — mekaniklerini ısıt, stresli anlarda riskli oyun arama.".to_string(),
        );
    }

    tips.dedup();
    tips.truncate(4);
    tips
}

// ── E2: dalga yönetimi dersleri ───────────────────────────────────────────────

/// Arketip × baskı-durumu wave planları (derleme zamanında gömülü KB).
const WAVE_PLANS_JSON: &str = include_str!("../../resources/draft_iq/wave_plans.json");

#[derive(serde::Deserialize)]
struct WavePlanTable {
    plans: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

fn wave_plans(
) -> &'static std::collections::HashMap<String, std::collections::HashMap<String, String>> {
    use std::sync::OnceLock;
    static TABLE: OnceLock<
        std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    > = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str::<WavePlanTable>(WAVE_PLANS_JSON)
            .map(|t| t.plans)
            .unwrap_or_default()
    })
}

/// Dalga yönetimi dersi: adayın arketipi × erken-oyun baskı durumu.
/// Baskı = power_curve.early karşılaştırması (rakip yoksa "even"); arketip
/// tabloda yoksa `None` (uydurma yok). Genel wave kuralları — şampiyona özgü
/// mekanik iddiası içermez (coach_quality dili).
pub fn wave_plan_line(
    candidate: &ChampionArchetype,
    lane_opponent: Option<&ChampionArchetype>,
) -> Option<String> {
    let state = match lane_opponent {
        None => "even",
        Some(opp) => {
            let mine = candidate.power_curve.early;
            let theirs = opp.power_curve.early;
            let total = mine + theirs;
            if total < 0.01 {
                "even"
            } else {
                let share = mine / total;
                if share > 0.55 {
                    "pressing"
                } else if share < 0.45 {
                    "weak"
                } else {
                    "even"
                }
            }
        }
    };
    wave_plans()
        .get(candidate.archetype.as_str())
        .and_then(|m| m.get(state))
        .cloned()
}

#[cfg(test)]
mod matchup_tips_tests {
    use super::*;
    use crate::draft_iq::archetype::{CcProfile, DamageProfile, PowerCurve};

    fn arch(
        archetype: &str,
        early: f32,
        late: f32,
        mobility: &str,
        hard_cc: u8,
    ) -> ChampionArchetype {
        ChampionArchetype {
            champion_id: 1,
            archetype: archetype.to_string(),
            damage_profile: DamageProfile {
                ad: 0.5,
                ap: 0.5,
                true_damage: 0.0,
            },
            cc: CcProfile {
                has_hard_cc: hard_cc > 0,
                hard_cc_count: hard_cc,
                primary_cc: vec![],
            },
            mobility: mobility.to_string(),
            engage_role: "none".to_string(),
            peel_capability: "low".to_string(),
            blind_safety: 0.6,
            execution_difficulty: 3,
            win_condition: "teamfight".to_string(),
            ult_type: "x".to_string(),
            confidence: "high".to_string(),
            power_curve: PowerCurve {
                early,
                mid: 0.6,
                late,
            },
            counters_archetypes: vec![],
            utility_tags: vec![],
        }
    }

    #[test]
    fn early_bully_vs_scaler_suggests_pressure() {
        let me = arch("juggernaut", 0.80, 0.50, "low", 1);
        let opp = arch("marksman", 0.40, 0.85, "low", 0);
        let tips = build_matchup_tips(&me, &opp, "top");
        assert!(
            tips.iter()
                .any(|t| t.contains("agresif bas") || t.contains("push") || t.contains("plate")),
            "erken bully → baskı tüyosu beklenir, got: {tips:?}"
        );
    }

    #[test]
    fn scaler_vs_early_bully_suggests_safe() {
        let me = arch("marksman", 0.40, 0.85, "low", 0);
        let opp = arch("juggernaut", 0.80, 0.50, "low", 1);
        let tips = build_matchup_tips(&me, &opp, "top");
        assert!(
            tips.iter()
                .any(|t| t.contains("freeze") || t.contains("güvenli")),
            "scaler → güvenli/freeze tüyosu beklenir, got: {tips:?}"
        );
    }

    #[test]
    fn phase_advice_is_archetype_and_wincondition_grounded() {
        let assassin = arch("assassin", 0.6, 0.5, "high", 0);
        assert!(
            build_mid_game_advice(&assassin).contains("roam"),
            "assassin mid → roam"
        );
        let adc = arch("marksman", 0.4, 0.85, "low", 0);
        assert!(
            build_mid_game_advice(&adc).contains("spike"),
            "marksman mid → spike"
        );
        assert!(build_late_game_advice("split_push").contains("Side lane"));
        assert!(build_late_game_advice("teamfight").contains("5v5"));
    }

    #[test]
    fn build_rationale_grounds_core_and_lane_counter() {
        let mage = arch("burst_mage", 0.5, 0.8, "low", 0);
        let r = build_rationale(&mage, Some("assassin"), "seed");
        assert!(r.contains("büyü gücü"), "mage core intent: {r}");
        assert!(r.contains("zırh"), "vs AD lane opp → armor hint: {r}");
        // General (heuristic) build is honestly hedged.
        let g = build_rationale(&mage, None, "general");
        assert!(g.contains("genel şablon"), "general hedge: {g}");
        assert!(!g.contains("zırh"), "no opponent → no counter hint: {g}");
    }
}
