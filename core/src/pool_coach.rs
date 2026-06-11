//! Champion Pool Coach Core v1 (Sprint I, Claude) — engine-pure pool analysis.
//!
//! Analyses a player's champion pool for a role across coverage dimensions (role
//! fit, blind-pick safety, counter/flex, comfort, execution risk, team identity),
//! surfaces the gaps, and produces a "strengthen your role with 3 champions" plan
//! (1 blind-safe · 1 counter-pick · 1 meta/easy entry) of champions the player does
//! NOT already play, drawn from caller-supplied candidates. Pure: no DB/network. The
//! per-champion traits are resolved
//! by the command layer from the KB `ChampionArchetype` + mastery — so the reads
//! are grounded, never fabricated. With no mastery/match data it returns a `thin`
//! data state instead of inventing comfort. Coaching is hedged (no guaranteed dil).
//!
//! Dimension + plan-role names are stable machine keys (UI i18n-maps); prose is TR.
#![allow(dead_code)] // public DTOs + engine consumed by a command/UI in a later turn

use crate::champion_types::archetype_position_fit;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

/// A champion in (or a candidate for) the player's pool. Traits are caller-resolved
/// from the KB `ChampionArchetype`; `comfort`/`games` come from mastery/history (0
/// when unknown — never guessed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoolChampion {
    pub champion_id: u32,
    pub champion_key: String,
    pub archetype: String,
    /// KB blind-pick safety in [0, 1].
    pub blind_safety: f32,
    /// KB execution difficulty 1 (easy) .. 5 (hard).
    pub execution_difficulty: u8,
    /// KB late-game power in [0, 1] (scaling).
    pub power_late: f32,
    /// KB engage capability.
    pub engage: bool,
    /// KB peel/protect capability.
    pub peel: bool,
    /// Personal comfort in [0, 1] (mastery), 0 when unknown.
    pub comfort: f32,
    /// Games played on the champ (comfort sample), 0 when unknown.
    pub games: u32,
    /// Meta strength in [0, 1] from blended win-rate (0.5 neutral when unknown).
    /// Used to prefer meta-strong picks for the blind-safe / counter slots; the
    /// comfort slot stays mastery-led.
    pub meta_strength: f32,
}

/// Analysis request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCoachInput {
    pub role: String,
    pub pool: Vec<PoolChampion>,
    /// Role-fit champions the player could learn (not owned). May be empty.
    pub candidates: Vec<PoolChampion>,
}

/// Per-dimension coverage flags. `execution_risk` ∈ {low, medium, high}.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct PoolCoverage {
    pub role_covered: bool,
    pub has_blind_safe: bool,
    pub has_counter_flex: bool,
    pub has_comfort: bool,
    pub identity_variety: bool,
    pub execution_risk: String,
}

/// A missing coverage dimension. `dimension`/`severity` are machine keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct PoolGap {
    pub dimension: String,
    pub severity: String,
    pub note: String,
}

/// One champion to learn, slotted into the 3-champ plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct TrainingRecommendation {
    pub champion_id: u32,
    pub champion_key: String,
    /// "blind_safe" | "counter_pick" | "meta_pick".
    pub role_in_plan: String,
    pub reason: String,
    pub confidence: String,
    /// 2-3 concrete KB-derived practice drills (training mode). Always serialized
    /// (even if empty) so the UI can `.map` safely.
    #[serde(default)]
    pub drills: Vec<String>,
}

/// Full pool-coach read. `data_state` ∈ {rich, thin}.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct ChampionPoolPlan {
    pub role: String,
    pub data_state: String,
    pub pool_size: u32,
    pub coverage: PoolCoverage,
    pub gaps: Vec<PoolGap>,
    pub training: Vec<TrainingRecommendation>,
    pub summary: String,
}

const BLIND_SAFE_THRESHOLD: f32 = 0.6;
const COMFORT_SCORE_THRESHOLD: f32 = 0.5;
const COMFORT_GAMES_THRESHOLD: u32 = 20;
const HIGH_EXEC_AVG: f32 = 4.0;
const MEDIUM_EXEC_AVG: f32 = 3.0;
/// At/above this blended meta strength, a learn-target is labelled "meta strong".
const META_STRONG_THRESHOLD: f32 = 0.6;

fn gap(dimension: &str, severity: &str, note: &str) -> PoolGap {
    PoolGap {
        dimension: dimension.to_string(),
        severity: severity.to_string(),
        note: note.to_string(),
    }
}

/// Analyse the pool and build the plan. Pure + deterministic.
pub fn analyze_pool(input: &PoolCoachInput) -> ChampionPoolPlan {
    let pool = &input.pool;
    let role = input.role.as_str();
    let has_personal = pool.iter().any(|c| c.games > 0 || c.comfort > 0.0);
    let data_state = if pool.is_empty() || !has_personal {
        "thin"
    } else {
        "rich"
    };

    let role_covered = pool
        .iter()
        .any(|c| archetype_position_fit(&c.archetype, role));
    let has_blind_safe = pool.iter().any(|c| c.blind_safety >= BLIND_SAFE_THRESHOLD);
    let distinct: HashSet<&str> = pool.iter().map(|c| c.archetype.as_str()).collect();
    let has_counter_flex = distinct.len() >= 2;
    let has_comfort = pool
        .iter()
        .any(|c| c.comfort >= COMFORT_SCORE_THRESHOLD || c.games >= COMFORT_GAMES_THRESHOLD);
    let identity_variety = {
        let engage = pool.iter().any(|c| c.engage);
        let peel = pool.iter().any(|c| c.peel);
        let scaling = pool.iter().any(|c| c.power_late >= 0.7);
        [engage, peel, scaling].iter().filter(|x| **x).count() >= 2
    };
    let avg_exec = if pool.is_empty() {
        0.0
    } else {
        pool.iter()
            .map(|c| f32::from(c.execution_difficulty))
            .sum::<f32>()
            / pool.len() as f32
    };
    let execution_risk = if avg_exec >= HIGH_EXEC_AVG {
        "high"
    } else if avg_exec >= MEDIUM_EXEC_AVG {
        "medium"
    } else {
        "low"
    };

    let coverage = PoolCoverage {
        role_covered,
        has_blind_safe,
        has_counter_flex,
        has_comfort,
        identity_variety,
        execution_risk: execution_risk.to_string(),
    };

    let mut gaps = Vec::new();
    if !pool.is_empty() && !role_covered {
        gaps.push(gap(
            "role_coverage",
            "high",
            "Havuzun bu rolü güvenle kapsamıyor.",
        ));
    }
    if !pool.is_empty() && !has_blind_safe {
        gaps.push(gap(
            "blind_safe",
            "high",
            "Blind first-pick için güvenli seçeneğin yok.",
        ));
    }
    if !pool.is_empty() && !has_counter_flex {
        gaps.push(gap(
            "counter_flex",
            "medium",
            "Havuz tek tip; matchup'a göre cevap dar.",
        ));
    }
    // Comfort is only a real gap when there IS personal data to judge it on.
    if data_state == "rich" && !has_comfort {
        gaps.push(gap(
            "comfort",
            "medium",
            "Güvenilir, çok oynanmış bir konfor pick'i yok.",
        ));
    }
    if execution_risk == "high" {
        gaps.push(gap(
            "execution_risk",
            "medium",
            "Havuz mekanik olarak zorlayıcı; tutarlılık riski.",
        ));
    }
    if !pool.is_empty() && !identity_variety {
        gaps.push(gap(
            "identity_variety",
            "low",
            "Takım ihtiyacına göre kimlik esnekliği sınırlı.",
        ));
    }

    let training = build_training(&input.candidates);
    let summary = build_summary(role, data_state, &coverage, &gaps, pool.len());

    ChampionPoolPlan {
        role: input.role.clone(),
        data_state: data_state.to_string(),
        pool_size: pool.len() as u32,
        coverage,
        gaps,
        training,
        summary,
    }
}

/// A short Turkish phrase naming the champion's core archetype edge — used to make
/// the pool training reasons champion-specific instead of one generic line per slot.
fn archetype_edge_tr(archetype: &str) -> &'static str {
    match archetype {
        "vanguard" => "frontline engage tankı",
        "juggernaut" => "dayanıklı split baskısı",
        "diver" => "backline'a dalan diver",
        "skirmisher" => "uzun duello uzmanı",
        "assassin" => "izole pick gücü",
        "control_mage" | "battle_mage" => "zone + CC kontrolü",
        "burst_mage" => "tek hedefe burst",
        "artillery" => "uzun menzilli poke",
        "marksman" => "geç oyun sürekli hasar",
        "catcher" => "CC ile yakalama",
        "enchanter" => "koruma + buff",
        "warden" => "pasif peel duvarı",
        _ => "esnek kimlik",
    }
}

/// What a reactive last pick of this archetype exploits in the matchup.
fn counter_exploit_tr(archetype: &str) -> &'static str {
    match archetype {
        "assassin" | "catcher" => "kırılgan carry'leri izole edip cezalandırır",
        "skirmisher" | "diver" => "duelloda rakibi aşındırır",
        "control_mage" | "battle_mage" => "alanı kapatıp matchup'ı daraltır",
        "artillery" => "menzil ve poke üstünlüğünü sömürür",
        "burst_mage" => "tek hedef burst penceresini zorlar",
        "juggernaut" => "uzun dövüşte aşındırıp baskı kurar",
        "vanguard" => "engage tehdidiyle rakibi pasifleştirir",
        "marksman" => "geç oyun hasar üstünlüğüne oynar",
        _ => "matchup avantajını kullanır",
    }
}

/// Turkish execution-difficulty descriptor for the learn-curve note.
fn exec_label(execution_difficulty: u8) -> &'static str {
    match execution_difficulty {
        1..=2 => "öğrenmesi kolay",
        3 => "orta zorlukta",
        _ => "mekanik isteyen",
    }
}

/// Champion-specific Turkish reason for why this champion fits the plan `slot`.
/// Grounded in the resolved KB traits (archetype edge, blind safety, execution,
/// personal games) so each suggestion reads distinctly. Measured language — no
/// guarantees (passes the pool-coach prose audit).
fn slot_reason(c: &PoolChampion, slot: &str) -> String {
    let edge = archetype_edge_tr(&c.archetype);
    match slot {
        "blind_safe" => {
            let safety = if c.blind_safety >= 0.78 {
                "rakip draftını görmeden rahat basılır"
            } else {
                "blind'de görece güvenli, cezalandırması zor"
            };
            format!(
                "Blind anchor — {edge}; {safety} ({}).",
                exec_label(c.execution_difficulty)
            )
        }
        "counter_pick" => format!(
            "Tepki pick'i — son pick'te {}; {edge}.",
            counter_exploit_tr(&c.archetype)
        ),
        "meta_pick" => format!(
            "Meta + kolay giriş — {edge}; {}; havuza güvenli, öğrenmesi erişilebilir ekleme.",
            exec_label(c.execution_difficulty)
        ),
        _ => edge.to_string(),
    }
}

/// 2-3 concrete practice drills for learning `c`, derived from its KB traits — the
/// training-mode "how to practice this". Honest, mechanic-grounded; no guarantees.
fn build_drills(c: &PoolChampion) -> Vec<String> {
    let mut drills = vec![match c.archetype.as_str() {
        "artillery" | "burst_mage" | "control_mage" | "battle_mage" => {
            "Skillshot isabeti: hareketli hedefe ana büyünü tutturma pratiği yap."
        }
        "assassin" | "diver" => {
            "Combo hızı: tam burst rotasyonunu refleks hâline getir (bot maçında tekrarla)."
        }
        "skirmisher" => "Spacing + animation cancel: uzun dövüş mikrosunu çalış.",
        "marksman" => "Kiting: orbwalk (attack-move) ile pozisyon koruyarak hasar bas.",
        "enchanter" | "warden" => {
            "Peel zamanlaması: carry'ye shield/CC kullanma refleksini geliştir."
        }
        "vanguard" | "catcher" => "Engage isabeti: hook/CC'yi sis ardından doğru hedefe land et.",
        "juggernaut" => "Eşik dövüş: ne zaman all-in, ne zaman geri çekileceğini çalış.",
        _ => "Temel mekanikler: combo ve hareketi bot maçında ısıt.",
    }
    .to_string()];

    if c.power_late >= 0.70 {
        drills.push("Ölçekleme disiplini: erken güvenli farmla, item spike'ını bekle.".to_string());
    } else {
        drills.push("Erken tempo: lane baskısını plate/obje avantajına çevir.".to_string());
    }

    if c.execution_difficulty >= 4 {
        drills.push("Yüksek mekanik: normal/draft öncesi 1-2 bot maçıyla ısın.".to_string());
    }

    drills
}

/// Build a learn-target rec for `c` in `slot` with a champion-specific reason, an
/// honest "meta güçlü" suffix only when the blended sample actually lifted it, and
/// concrete practice drills (training mode).
fn rec(c: &PoolChampion, slot: &str) -> TrainingRecommendation {
    let mut reason = slot_reason(c, slot);
    if c.meta_strength >= META_STRONG_THRESHOLD {
        reason.push_str(" · meta güçlü");
    }
    TrainingRecommendation {
        champion_id: c.champion_id,
        champion_key: c.champion_key.clone(),
        role_in_plan: slot.to_string(),
        reason,
        confidence: confidence_for(c.execution_difficulty),
        drills: build_drills(c),
    }
}

fn build_training(candidates: &[PoolChampion]) -> Vec<TrainingRecommendation> {
    let mut used: HashSet<u32> = HashSet::new();
    let (mut blind_safe, mut counter, mut meta_pick) = (None, None, None);

    // Candidates are all NEW champions (the command excludes anything the player
    // already plays), so the plan is three genuine learn-targets — never "learn a
    // champion you already own".

    // Blind-safe anchor: among the blind-safe candidates, prefer the meta-strongest
    // (then highest blind_safety, then id). Meta leads; safety floor still required.
    if let Some(c) = candidates
        .iter()
        .filter(|c| c.blind_safety >= 0.55)
        .max_by(|a, b| {
            a.meta_strength
                .partial_cmp(&b.meta_strength)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    a.blind_safety
                        .partial_cmp(&b.blind_safety)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
                .then(b.champion_id.cmp(&a.champion_id))
        })
    {
        used.insert(c.champion_id);
        blind_safe = Some(rec(c, "blind_safe"));
    }

    // Reactive counter: matchup-dependent (blind_safety < floor); among those prefer
    // the meta-strongest, then the most counter-y (lowest blind_safety), then id.
    if let Some(c) = candidates
        .iter()
        .filter(|c| !used.contains(&c.champion_id) && c.blind_safety < 0.55)
        .max_by(|a, b| {
            a.meta_strength
                .partial_cmp(&b.meta_strength)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    b.blind_safety
                        .partial_cmp(&a.blind_safety)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
                .then(b.champion_id.cmp(&a.champion_id))
        })
    {
        used.insert(c.champion_id);
        counter = Some(rec(c, "counter_pick"));
    }

    // Meta + easy entry: the meta-strongest of the remaining, tie-broken to the
    // easiest to learn — an accessible new champion to round out the role.
    if let Some(c) = candidates
        .iter()
        .filter(|c| !used.contains(&c.champion_id))
        .max_by(|a, b| {
            a.meta_strength
                .partial_cmp(&b.meta_strength)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.execution_difficulty.cmp(&a.execution_difficulty))
                .then(b.champion_id.cmp(&a.champion_id))
        })
    {
        meta_pick = Some(rec(c, "meta_pick"));
    }

    [blind_safe, counter, meta_pick]
        .into_iter()
        .flatten()
        .collect()
}

fn confidence_for(execution_difficulty: u8) -> String {
    match execution_difficulty {
        1..=2 => "high",
        3 => "medium",
        _ => "low",
    }
    .to_string()
}

fn build_summary(
    role: &str,
    data_state: &str,
    coverage: &PoolCoverage,
    gaps: &[PoolGap],
    pool_size: usize,
) -> String {
    if pool_size == 0 {
        return format!(
            "{role} için havuz verisi yok; mastery/maç geçmişi gelince analiz derinleşir."
        );
    }
    let head = if data_state == "thin" {
        "İnce veri (yeterli maç/mastery yok); yapısal okuma sınırlı".to_string()
    } else {
        format!("{role} havuzu {pool_size} şampiyon")
    };
    let tail = if gaps.is_empty() {
        "belirgin kapsama açığı yok; dengeli görünüyor.".to_string()
    } else {
        let dims: Vec<&str> = gaps.iter().take(3).map(|g| g.dimension.as_str()).collect();
        format!("öncelikli açıklar: {}.", dims.join(", "))
    };
    let _ = coverage;
    format!("{head}; {tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(id: u32, key: &str, archetype: &str, blind: f32, exec: u8) -> PoolChampion {
        PoolChampion {
            champion_id: id,
            champion_key: key.into(),
            archetype: archetype.into(),
            blind_safety: blind,
            execution_difficulty: exec,
            power_late: 0.5,
            engage: false,
            peel: false,
            comfort: 0.0,
            games: 0,
            meta_strength: 0.5, // neutral unless a test sets it
        }
    }

    fn played(mut c: PoolChampion, comfort: f32, games: u32) -> PoolChampion {
        c.comfort = comfort;
        c.games = games;
        c
    }

    fn with_meta(mut c: PoolChampion, meta: f32) -> PoolChampion {
        c.meta_strength = meta;
        c
    }

    #[test]
    fn empty_pool_is_thin_with_no_fabrication() {
        let plan = analyze_pool(&PoolCoachInput {
            role: "middle".into(),
            pool: vec![],
            candidates: vec![],
        });
        assert_eq!(plan.data_state, "thin");
        assert_eq!(plan.pool_size, 0);
        assert!(plan.training.is_empty());
        assert!(plan.summary.contains("veri"));
    }

    #[test]
    fn pool_without_games_is_thin_and_skips_comfort_gap() {
        // Structural champs but zero personal data → thin; no false "no comfort" gap.
        let plan = analyze_pool(&PoolCoachInput {
            role: "middle".into(),
            pool: vec![ch(1, "Lux", "control_mage", 0.7, 3)],
            candidates: vec![],
        });
        assert_eq!(plan.data_state, "thin");
        assert!(!plan.gaps.iter().any(|g| g.dimension == "comfort"));
    }

    #[test]
    fn no_blind_safe_pool_surfaces_high_gap() {
        let plan = analyze_pool(&PoolCoachInput {
            role: "middle".into(),
            pool: vec![played(ch(1, "Zed", "assassin", 0.3, 4), 0.8, 50)],
            candidates: vec![],
        });
        assert!(plan
            .gaps
            .iter()
            .any(|g| g.dimension == "blind_safe" && g.severity == "high"));
    }

    #[test]
    fn training_plan_fills_three_distinct_roles() {
        let candidates = vec![
            ch(1, "Malphite", "vanguard", 0.8, 2), // blind-safe
            ch(2, "Zed", "assassin", 0.3, 4),      // counter
            ch(3, "Annie", "burst_mage", 0.6, 1),  // meta/easy entry (easiest)
        ];
        let plan = analyze_pool(&PoolCoachInput {
            role: "middle".into(),
            pool: vec![played(ch(9, "Lux", "control_mage", 0.7, 3), 0.6, 30)],
            candidates,
        });
        let roles: Vec<&str> = plan
            .training
            .iter()
            .map(|t| t.role_in_plan.as_str())
            .collect();
        assert!(roles.contains(&"blind_safe"));
        assert!(roles.contains(&"counter_pick"));
        assert!(roles.contains(&"meta_pick"));
        // distinct champions
        let ids: HashSet<u32> = plan.training.iter().map(|t| t.champion_id).collect();
        assert_eq!(ids.len(), plan.training.len());
    }

    #[test]
    fn training_never_recommends_a_champion_already_played() {
        // The command excludes played champs from candidates; analyze_pool must also
        // produce no learn-target that the player already plays (all candidates new).
        let candidates = vec![
            ch(1, "Malphite", "vanguard", 0.8, 2),
            ch(2, "Zed", "assassin", 0.3, 4),
            ch(3, "Annie", "burst_mage", 0.6, 1),
        ];
        let plan = analyze_pool(&PoolCoachInput {
            role: "middle".into(),
            pool: vec![played(ch(9, "Lux", "control_mage", 0.7, 3), 0.9, 200)],
            candidates,
        });
        assert!(
            !plan.training.iter().any(|t| t.champion_id == 9),
            "a played champion is never a learn-target"
        );
    }

    #[test]
    fn meta_strength_breaks_blind_safe_and_counter_ties() {
        // Two equally blind-safe candidates; meta_strength must pick the stronger.
        // Two equally counter-y candidates; same. Comfort slot untouched (no played).
        let candidates = vec![
            with_meta(ch(1, "SafeWeak", "vanguard", 0.8, 2), 0.4),
            with_meta(ch(2, "SafeStrong", "vanguard", 0.8, 2), 0.9),
            with_meta(ch(3, "CtrWeak", "assassin", 0.3, 4), 0.4),
            with_meta(ch(4, "CtrStrong", "assassin", 0.3, 4), 0.9),
        ];
        let plan = analyze_pool(&PoolCoachInput {
            role: "middle".into(),
            pool: vec![played(ch(9, "Lux", "control_mage", 0.7, 3), 0.6, 30)],
            candidates,
        });
        let blind = plan
            .training
            .iter()
            .find(|t| t.role_in_plan == "blind_safe")
            .unwrap();
        assert_eq!(blind.champion_id, 2, "meta-strong blind-safe pick wins");
        assert!(blind.reason.contains("meta güçlü"));
        let counter = plan
            .training
            .iter()
            .find(|t| t.role_in_plan == "counter_pick")
            .unwrap();
        assert_eq!(counter.champion_id, 4, "meta-strong counter pick wins");
    }

    #[test]
    fn drills_are_archetype_specific_with_warmup_for_hard_champs() {
        let mage = build_drills(&ch(1, "Lux", "control_mage", 0.7, 3));
        assert!(
            mage.iter().any(|d| d.contains("Skillshot")),
            "mage → skillshot drill"
        );
        assert!(mage.len() >= 2, "mechanic + curve drill at minimum");

        // A high-execution champion (difficulty 5) gets the bot-game warm-up drill.
        let hard = build_drills(&ch(2, "Zed", "assassin", 0.3, 5));
        assert!(
            hard.iter().any(|d| d.contains("Yüksek mekanik")),
            "hard champ → warm-up drill"
        );
    }

    #[test]
    fn is_deterministic() {
        let input = PoolCoachInput {
            role: "top".into(),
            pool: vec![played(ch(1, "Garen", "juggernaut", 0.7, 2), 0.7, 60)],
            candidates: vec![
                ch(2, "Fiora", "skirmisher", 0.4, 4),
                ch(3, "Sion", "vanguard", 0.8, 2),
            ],
        };
        assert_eq!(analyze_pool(&input), analyze_pool(&input));
    }
}
