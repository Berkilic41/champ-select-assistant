//! Team-level macro game plan.
//!
//! Where the per-candidate `analyzer` answers "should I pick this champion",
//! this module answers "now that the team is forming, how do we win the game".
//! It aggregates ally + enemy champion archetypes (power curves + comp shape)
//! into an actionable plan: team identity, a phase-by-phase timeline, win
//! conditions, objective priorities, the enemy's threat, and a fallback plan.
//!
//! Pure — no I/O. Language is intentionally measured ("eğilimli", "avantajlı"),
//! never absolute guarantees.

use super::archetype::ChampionArchetype;
use super::narrative::detect_enemy_win_condition;
use serde::{Deserialize, Serialize};

/// One game-phase band of the macro timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PhaseBand {
    /// e.g. "Erken (0-14dk)".
    pub label: String,
    /// "advantage" | "even" | "disadvantage" — your team vs enemy in this phase.
    pub stance: String,
    /// Short Turkish advice for the band.
    pub advice: String,
    /// Your team's aggregate strength in this phase, 0..1 (UI bar).
    pub strength: f32,
}

/// Full macro game plan for the current (possibly partial) team composition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GamePlan {
    /// Short identity label, e.g. "Teamfight / deathball".
    pub team_identity: String,
    /// One-line elaboration of the identity.
    pub identity_note: String,
    /// Three bands: early / mid / late.
    pub timeline: Vec<PhaseBand>,
    /// 1–2 primary, actionable win conditions.
    pub win_conditions: Vec<String>,
    /// Objective priorities (dragon / herald / baron focus).
    pub objectives: Vec<String>,
    /// Enemy's likely win condition + how to deny it.
    pub enemy_threat: String,
    /// Fallback plan when ahead/behind.
    pub alt_plan: Option<String>,
    /// True when the draft is incomplete (fewer than a full team known) — the
    /// plan will sharpen as more champions lock.
    pub partial: bool,
}

/// Average (early, mid, late) power curve across a set of archetypes.
/// Returns the neutral `(0.5, 0.5, 0.5)` for an empty set.
fn avg_curve(arch: &[&ChampionArchetype]) -> (f32, f32, f32) {
    if arch.is_empty() {
        return (0.5, 0.5, 0.5);
    }
    let n = arch.len() as f32;
    let e = arch.iter().map(|a| a.power_curve.early).sum::<f32>() / n;
    let m = arch.iter().map(|a| a.power_curve.mid).sum::<f32>() / n;
    let l = arch.iter().map(|a| a.power_curve.late).sum::<f32>() / n;
    (e, m, l)
}

/// Relative stance of `ally_p` vs `enemy_p` in one phase.
fn stance(ally_p: f32, enemy_p: f32) -> &'static str {
    let d = ally_p - enemy_p;
    if d > 0.06 {
        "advantage"
    } else if d < -0.06 {
        "disadvantage"
    } else {
        "even"
    }
}

/// Composition shape: how many allies fall into each tactical bucket.
struct Shape {
    engage: u32,
    frontline: u32,
    poke: u32,
    pick: u32,
    protect: u32,
    /// Team gets meaningfully stronger as the game goes long.
    scaling: bool,
    /// Team is meaningfully strong in the early game.
    early_strong: bool,
}

fn shape_of(ally: &[&ChampionArchetype]) -> Shape {
    let mut engage = 0;
    let mut frontline = 0;
    let mut poke = 0;
    let mut pick = 0;
    let mut protect = 0;
    for a in ally {
        let at = a.archetype.as_str();
        if a.engage_role == "initiator" || matches!(at, "vanguard" | "diver") {
            engage += 1;
        }
        if matches!(at, "vanguard" | "juggernaut" | "warden") {
            frontline += 1;
        }
        if at == "artillery" || a.utility_tags.iter().any(|t| t == "poke") {
            poke += 1;
        }
        if matches!(at, "assassin" | "catcher") {
            pick += 1;
        }
        if matches!(at, "enchanter" | "warden") {
            protect += 1;
        }
    }
    let (e, _, l) = avg_curve(ally);
    Shape {
        engage,
        frontline,
        poke,
        pick,
        protect,
        scaling: l - e >= 0.12,
        early_strong: e >= 0.65,
    }
}

/// Returns `(identity_label, identity_token, identity_note)`.
/// `identity_token` is a stable key used to drive downstream copy.
fn classify_identity(shape: &Shape) -> (&'static str, &'static str, &'static str) {
    if shape.poke >= 2 && (shape.protect >= 1 || shape.poke >= 3) {
        return (
            "Poke & kuşatma",
            "poke",
            "Uzaktan eziterek can avantajı aç, sonra obje/kule kuşat. Yakın engage'e girme.",
        );
    }
    if shape.pick >= 2 {
        return (
            "Pick / koparma",
            "pick",
            "İzole hedefleri koparıp 5v4 obje al. Vision + flank ile fırsat üret.",
        );
    }
    if shape.engage >= 2 && shape.frontline >= 1 {
        return (
            "Teamfight / deathball",
            "teamfight",
            "Gruplaş, engage'i kontrollü aç ve AOE ile 5v5 kazan. Obje etrafında dövüş.",
        );
    }
    if shape.scaling {
        return (
            "Geç oyun scaling",
            "scaling",
            "Erken güvenli oyna; carry'ler ölçeklendikçe oyun senin lehine döner.",
        );
    }
    if shape.early_strong {
        return (
            "Erken tempo",
            "early",
            "Erken baskı kur, avantajı obje/kuleye çevir; oyunu geç oyuna uzatma.",
        );
    }
    (
        "Esnek / tempo",
        "flex",
        "Net bir kimlik yok — fazlara göre tempo ayarla, avantajlı pencerelerde bas.",
    )
}

fn band_advice(phase: &str, stance: &str, token: &str) -> String {
    match (phase, stance) {
        ("early", "advantage") => {
            "Erken güçlüsün — lane'de bas, ilk plate ve drake için tempo al.".into()
        }
        ("early", "disadvantage") => {
            "Erken zayıfsın — güvenli farmla, kötü takaslardan kaç, jungle'ı gözle.".into()
        }
        ("early", _) => "Erken eşit — riskli takas yok, vision + farm önceliği.".into(),
        ("mid", "advantage") => "Orta oyun senin — gruplaş, obje al ve avantajı büyüt.".into(),
        ("mid", "disadvantage") => {
            "Orta oyunda dezavantajlısın — dövüşü zorlama, pick/obje fırsatı bekle.".into()
        }
        ("mid", _) => "Orta oyun eşit — pick ve obje pencerelerini kolla.".into(),
        ("late", "advantage") => "Geç oyun senin — baron/elder etrafında 5v5'e zorla.".into(),
        ("late", "disadvantage") => {
            if token == "early" || token == "poke" {
                "Geç oyunda zayıflıyorsun — oyunu uzatma, erken kapatmaya çalış.".into()
            } else {
                "Geç oyunda dezavantaj — net engage olmadan 5v5'e girme.".into()
            }
        }
        ("late", _) => "Geç oyun eşit — tek iyi pick/engage oyunu çevirir, sabırlı ol.".into(),
        _ => "Tempo ayarla.".into(),
    }
}

fn win_conditions(token: &str, strongest: &str) -> Vec<String> {
    let mut out = Vec::new();
    match token {
        "teamfight" => out
            .push("Obje etrafında gruplaş; engage'i sen başlat, AOE hasarıyla 5v5'i kazan.".into()),
        "poke" => out.push("Koridorlarda poke ile can avantajı aç, sonra objeyi kuşat.".into()),
        "pick" => {
            out.push("Vision kurup izole düşmanı koparın; sayı üstünlüğüyle objeye geçin.".into())
        }
        "scaling" => out.push("Erken oyunu idare et; 3. item sonrası dövüşleri zorla.".into()),
        "early" => out.push("Erken avantajı drake/kule/herald'a çevir, kartopu yap.".into()),
        _ => out.push("Avantajlı fazında tempo bas, zayıf fazında güvenli oyna.".into()),
    }
    // Second, phase-anchored win condition.
    match strongest {
        "early" => out.push("En güçlü pencere erken — ilk 15 dakikada fark yarat.".into()),
        "mid" => out.push("En güçlü pencere orta oyun — 1-2 item sonrası bas.".into()),
        "late" => out.push("En güçlü pencere geç oyun — baron'a kadar idare et.".into()),
        _ => {}
    }
    out
}

fn objectives(token: &str, strongest: &str) -> Vec<String> {
    let mut out = Vec::new();
    match strongest {
        "early" => out.push("Erken drake + herald'ı al, kuleye çevir; tempoyu kaybetme.".into()),
        "late" => out.push("İlk drake'leri zorlamadan ver/al, drake ruhu ve baron'a oyna.".into()),
        _ => out.push("Drake kontrolü için vision kur; uygun pencerede herald al.".into()),
    }
    if token == "scaling" {
        out.push("Baron tehdidini geç oyuna sakla; erken baron riskine girme.".into());
    } else if token == "early" || token == "teamfight" {
        out.push("Herald'ı kule platelerine çevirip altın/tempo avantajı yarat.".into());
    }
    out
}

fn enemy_threat_text(enemy: &[&ChampionArchetype]) -> String {
    if enemy.is_empty() {
        return "Rakip kadrosu henüz belirsiz — lock'landıkça plan netleşir.".into();
    }
    let (ee, _, el) = avg_curve(enemy);
    if el - ee >= 0.12 {
        return "Rakip geç oyuna ölçekleniyor — 25. dk öncesi obje ve kule baskısıyla oyunu erken kapatmaya çalış.".into();
    }
    match detect_enemy_win_condition(enemy) {
        "pick" => "Rakip pick komposu — izole gezme, vision basıp gruplaş, sayı dezavantajına düşme.".into(),
        "teamfight" => "Rakip teamfight/engage komposu — kötü açıdan 5v5'e girme, engage'lerini wardla ve karşı-engage'e hazır ol.".into(),
        "poke" => "Rakip poke komposu — uzun koridorlarda dağınık durma, sis kullanıp üzerlerine engage aç.".into(),
        "split" => "Rakipte split baskısı — side lane'leri waveclear/TP ile karşıla, kalabalıkla 5v4 obje al.".into(),
        "protect" => "Rakip carry'yi koruyan kompo — hypercarry'yi geç oyuna bırakma, dive/pick ile carry'yi düşür.".into(),
        _ => "Rakip kompozisyonu karma — fazlara göre tempo ayarla.".into(),
    }
}

fn alt_plan_text(token: &str) -> Option<String> {
    Some(match token {
        "scaling" => "Geride kalırsan: teamfight'tan kaç, güvenli farmla 3. item'a ulaş. Öndeysen objeyle tempolu oyna, gereksiz dövüş arama.".into(),
        "teamfight" => "Engage yerlerse: objeyi bırak, dağıl ve pick avına çık — tek iyi engage oyunu çevirir.".into(),
        "poke" => "Engage yerlerse: geri çekil, vision yenile; poke avantajını koridor dışına taşıma.".into(),
        "pick" => "Pick çıkmazsa: 5v5'i zorlama, side push ile baskı kurup fırsat bekle.".into(),
        "early" => "Erken avantaj gelmezse: panikleme, güvenli farmla orta oyuna eşit ulaş.".into(),
        _ => "Öndeysen objeyle tempolu oyna; geride kalırsan dövüşten kaçıp scale et.".into(),
    })
}

/// Build the macro game plan from ally (including the local player's pick) and
/// enemy archetypes.
pub fn compute_game_plan(ally: &[&ChampionArchetype], enemy: &[&ChampionArchetype]) -> GamePlan {
    let (ae, am, al) = avg_curve(ally);
    let (ee, em, el) = avg_curve(enemy);

    let shape = shape_of(ally);
    let (identity, token, note) = classify_identity(&shape);

    // Strongest own phase (drives objective/win-condition anchoring).
    let strongest = if al >= am && al >= ae {
        "late"
    } else if ae >= am && ae >= al {
        "early"
    } else {
        "mid"
    };

    let timeline = vec![
        PhaseBand {
            label: "Erken (0-14dk)".into(),
            stance: stance(ae, ee).into(),
            advice: band_advice("early", stance(ae, ee), token),
            strength: ae.clamp(0.0, 1.0),
        },
        PhaseBand {
            label: "Orta (14-25dk)".into(),
            stance: stance(am, em).into(),
            advice: band_advice("mid", stance(am, em), token),
            strength: am.clamp(0.0, 1.0),
        },
        PhaseBand {
            label: "Geç (25dk+)".into(),
            stance: stance(al, el).into(),
            advice: band_advice("late", stance(al, el), token),
            strength: al.clamp(0.0, 1.0),
        },
    ];

    GamePlan {
        team_identity: identity.to_string(),
        identity_note: note.to_string(),
        timeline,
        win_conditions: win_conditions(token, strongest),
        objectives: objectives(token, strongest),
        enemy_threat: enemy_threat_text(enemy),
        alt_plan: alt_plan_text(token),
        partial: ally.len() < 5 || enemy.len() < 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft_iq::archetype::{CcProfile, DamageProfile, PowerCurve};

    fn arch(archetype: &str, early: f32, mid: f32, late: f32) -> ChampionArchetype {
        ChampionArchetype {
            champion_id: 1,
            archetype: archetype.to_string(),
            damage_profile: DamageProfile {
                ad: 0.5,
                ap: 0.5,
                true_damage: 0.0,
            },
            cc: CcProfile {
                has_hard_cc: false,
                hard_cc_count: 0,
                primary_cc: vec![],
            },
            mobility: "medium".into(),
            engage_role: "none".into(),
            peel_capability: "low".into(),
            blind_safety: 0.6,
            execution_difficulty: 3,
            win_condition: "teamfight".into(),
            ult_type: "aoe".into(),
            confidence: "high".into(),
            power_curve: PowerCurve { early, mid, late },
            counters_archetypes: vec![],
            utility_tags: vec![],
        }
    }

    #[test]
    fn scaling_team_vs_early_enemy_reads_correctly() {
        // Ally: late-scaling (low early, high late). Enemy: early-dominant.
        let a1 = arch("marksman", 0.35, 0.6, 0.9);
        let a2 = arch("enchanter", 0.4, 0.6, 0.85);
        let ally = vec![&a1, &a2];
        let e1 = arch("juggernaut", 0.8, 0.7, 0.45);
        let e2 = arch("diver", 0.8, 0.65, 0.4);
        let enemy = vec![&e1, &e2];

        let plan = compute_game_plan(&ally, &enemy);
        assert_eq!(plan.team_identity, "Geç oyun scaling");
        // Early disadvantage, late advantage.
        assert_eq!(plan.timeline[0].stance, "disadvantage");
        assert_eq!(plan.timeline[2].stance, "advantage");
        assert!(plan.partial, "2v2 known → partial draft");
        assert!(plan.alt_plan.is_some());
        assert!(!plan.win_conditions.is_empty());
        // Enemy is early-heavy (not scaling) and split-ish → some threat text present.
        assert!(!plan.enemy_threat.is_empty());
    }

    #[test]
    fn empty_enemy_is_partial_and_safe() {
        let a1 = arch("vanguard", 0.6, 0.65, 0.6);
        let ally = vec![&a1];
        let enemy: Vec<&ChampionArchetype> = vec![];
        let plan = compute_game_plan(&ally, &enemy);
        assert!(plan.partial);
        assert_eq!(plan.timeline.len(), 3);
        assert!(plan.enemy_threat.contains("belirsiz"));
    }

    #[test]
    fn poke_comp_detected() {
        let a1 = arch("artillery", 0.5, 0.7, 0.7);
        let a2 = arch("artillery", 0.5, 0.7, 0.7);
        let a3 = arch("enchanter", 0.45, 0.6, 0.7);
        let ally = vec![&a1, &a2, &a3];
        let enemy: Vec<&ChampionArchetype> = vec![];
        let plan = compute_game_plan(&ally, &enemy);
        assert_eq!(plan.team_identity, "Poke & kuşatma");
    }

    #[test]
    fn teamfight_comp_detected() {
        let a1 = arch("vanguard", 0.6, 0.7, 0.65); // engage + frontline
        let a2 = arch("diver", 0.6, 0.7, 0.6); // engage
        let a3 = arch("control_mage", 0.5, 0.7, 0.7);
        let ally = vec![&a1, &a2, &a3];
        let enemy: Vec<&ChampionArchetype> = vec![];
        let plan = compute_game_plan(&ally, &enemy);
        assert_eq!(plan.team_identity, "Teamfight / deathball");
    }
}
