use crate::db::{champion_rates_repo, champion_repo, mastery_repo, match_repo};
use crate::errors::AppError;
use crate::recommendation::champion_types::{archetype_position_fit, flex_positions};
use crate::recommendation::draft_iq::archetype::ChampionArchetype;
use crate::recommendation::pool_coach::{
    analyze_pool, ChampionPoolPlan, PoolChampion, PoolCoachInput,
};
use crate::recommendation::rate_blend;
use crate::recommendation::scoring::MetaRate;
use crate::AppState;
use std::collections::{HashMap, HashSet};
use tauri::State;

/// Below this sample, meta is unknown → neutral strength (no meta tilt).
const MIN_META_SAMPLE: u32 = 50;

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// Normalized meta strength in [0, 1] from the blended win-rate (0.5 neutral when
/// missing or under-sampled). Same shape as the pool-builder meta fit.
fn meta_strength_for(meta: &HashMap<(u32, String), MetaRate>, id: u32, role: &str) -> f32 {
    match meta.get(&(id, role.to_string())) {
        Some(r) if r.sample_size >= MIN_META_SAMPLE => ((r.win_rate - 0.48) / 0.07).clamp(0.0, 1.0),
        _ => 0.5,
    }
}

fn personal_comfort(mastery_points: i64, mastery_level: i64, games: u32) -> f32 {
    if mastery_points <= 0 && mastery_level <= 0 && games == 0 {
        return 0.0;
    }

    let points = (mastery_points.max(0) as f32 / 100_000.0).min(0.65);
    let level = (mastery_level.max(0) as f32 / 7.0).min(1.0) * 0.2;
    let sample = (games as f32 / 50.0).min(1.0) * 0.15;
    clamp01(points + level + sample)
}

fn has_any_tag(arch: &ChampionArchetype, tags: &[&str]) -> bool {
    arch.utility_tags.iter().any(|tag| {
        let tag = tag.to_lowercase();
        tags.iter().any(|needle| tag.contains(needle))
    })
}

fn has_engage(arch: &ChampionArchetype) -> bool {
    let engage_role = arch.engage_role.to_lowercase();
    engage_role != "none" && !engage_role.is_empty()
        || has_any_tag(arch, &["engage", "initiate", "pick"])
        || matches!(
            arch.archetype.as_str(),
            "vanguard" | "diver" | "catcher" | "assassin"
        )
}

fn has_peel(arch: &ChampionArchetype) -> bool {
    let peel = arch.peel_capability.to_lowercase();
    peel != "none" && peel != "low" && !peel.is_empty()
        || has_any_tag(arch, &["peel", "protect", "shield", "disengage"])
        || matches!(arch.archetype.as_str(), "warden" | "enchanter" | "catcher")
}

fn to_pool_champion(
    champion_id: u32,
    champion_key: &str,
    arch: &ChampionArchetype,
    comfort: f32,
    games: u32,
    meta_strength: f32,
) -> PoolChampion {
    PoolChampion {
        champion_id,
        champion_key: champion_key.to_string(),
        archetype: arch.archetype.clone(),
        blind_safety: clamp01(arch.blind_safety),
        execution_difficulty: arch.execution_difficulty.clamp(1, 5),
        power_late: clamp01(arch.power_curve.late),
        engage: has_engage(arch),
        peel: has_peel(arch),
        comfort: clamp01(comfort),
        games,
        meta_strength,
    }
}

fn champion_key_map(
    champions: Vec<champion_repo::ChampionRecord>,
    archetypes: &HashMap<String, ChampionArchetype>,
) -> HashMap<u32, String> {
    let mut by_id: HashMap<u32, String> = champions
        .into_iter()
        .filter_map(|champ| {
            u32::try_from(champ.champion_id)
                .ok()
                .map(|id| (id, champ.key))
        })
        .collect();

    for (key, arch) in archetypes {
        by_id.entry(arch.champion_id).or_insert_with(|| key.clone());
    }

    by_id
}

#[tauri::command]
pub async fn get_champion_pool_plan(
    role: String,
    puuid: String,
    state: State<'_, AppState>,
) -> Result<ChampionPoolPlan, AppError> {
    let (champions, mastery, stats, rate_rows, position_totals) = {
        let db = state.db.lock().await;
        (
            champion_repo::list_all(&db)?,
            mastery_repo::top_for_puuid(&db, &puuid, 100)?,
            match_repo::player_stats(&db, &puuid)?,
            champion_rates_repo::get_all_for_position(&db, &role).unwrap_or_default(),
            champion_rates_repo::position_sample_totals(&db).unwrap_or_default(),
        )
    };

    // Blend per-source rates → one MetaRate per champion, then a meta strength.
    let mut meta_rates: HashMap<(u32, String), MetaRate> = rate_blend::blend_rates(
        rate_rows
            .into_iter()
            .map(|r| rate_blend::SourceRate {
                champion_id: r.champion_id,
                position: r.position,
                win_rate: r.win_rate,
                ban_rate: r.ban_rate,
                sample_size: r.sample_size,
            })
            .collect(),
    );
    // Role-share filter: drop off-role noise so a champion only counts as "played this
    // role" when it's a real share of its cross-lane games (Olaf ADC ≈ 1% → dropped).
    let played = rate_blend::role_played_set(&position_totals, rate_blend::ROLE_SHARE_MIN);
    meta_rates.retain(|key, _| played.contains(key));

    let kb = state.draft_iq.clone();
    let key_by_id = champion_key_map(champions, &kb.archetypes);
    let mastery_by_id: HashMap<u32, (i64, i64)> = mastery
        .into_iter()
        .filter_map(|m| {
            u32::try_from(m.champion_id)
                .ok()
                .map(|id| (id, (m.level, m.points)))
        })
        .collect();
    let games_by_id: HashMap<u32, u32> = stats
        .into_iter()
        .filter_map(|s| u32::try_from(s.champion_id).ok().map(|id| (id, s.games)))
        .collect();

    let personal_ids: HashSet<u32> = mastery_by_id
        .keys()
        .copied()
        .chain(games_by_id.keys().copied())
        .collect();

    // Role fit: a champion belongs to THIS role only when real meta share, a curated
    // flex slot, or its archetype places it there (stops ADC Olaf / Yi / Sion noise).
    // Applied to BOTH the player's own pool — so the pool size + coverage are about
    // the role, not "every champion you've ever played" — and the learn candidates.
    let role_fits = |id: u32, archetype: &str| -> bool {
        let meta_present = meta_rates
            .get(&(id, role.clone()))
            .is_some_and(|r| r.sample_size >= MIN_META_SAMPLE);
        meta_present
            || flex_positions(id).contains(&role.as_str())
            || archetype_position_fit(archetype, &role)
    };

    // The pool is the champions the player MEANINGFULLY plays in this role — not every
    // champion they've ever touched once (a 134-champion mastery list is "every champ I
    // hovered", not a pool). A champion counts with real investment: mastery level ≥ 4,
    // or ≥ 12k points, or ≥ 15 recorded games. Then keep the top by comfort so even a
    // deep account shows a readable pool instead of 80 champions.
    const POOL_MAX: usize = 12;
    let mut pool: Vec<PoolChampion> = personal_ids
        .iter()
        .filter_map(|id| {
            let key = key_by_id.get(id)?;
            let arch = kb.get_archetype(key)?;
            if !role_fits(*id, &arch.archetype) {
                return None; // a champ you play in OTHER roles isn't your pool here
            }
            let (level, points) = mastery_by_id.get(id).copied().unwrap_or((0, 0));
            let games = games_by_id.get(id).copied().unwrap_or(0);
            if level < 4 && points < 12_000 && games < 15 {
                return None; // touched once ≠ part of your pool
            }
            Some(to_pool_champion(
                *id,
                key,
                arch,
                personal_comfort(points, level, games),
                games,
                meta_strength_for(&meta_rates, *id, &role),
            ))
        })
        .collect();
    // Highest comfort first, then cap — pool size + coverage reflect your real picks.
    pool.sort_by(|a, b| {
        b.comfort
            .partial_cmp(&a.comfort)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.champion_id.cmp(&b.champion_id))
    });
    pool.truncate(POOL_MAX);

    // Learn candidates: role-fitting champions the player does NOT already play — a
    // learn-target should be genuinely new (recommending a champ you already own is
    // pointless in a pool-development view).
    let mut candidates: Vec<PoolChampion> = kb
        .archetypes
        .iter()
        .filter(|(_, arch)| role_fits(arch.champion_id, &arch.archetype))
        .filter(|(_, arch)| !personal_ids.contains(&arch.champion_id))
        .map(|(key, arch)| {
            let (level, points) = mastery_by_id
                .get(&arch.champion_id)
                .copied()
                .unwrap_or((0, 0));
            let games = games_by_id.get(&arch.champion_id).copied().unwrap_or(0);
            to_pool_champion(
                arch.champion_id,
                key,
                arch,
                personal_comfort(points, level, games),
                games,
                meta_strength_for(&meta_rates, arch.champion_id, &role),
            )
        })
        .collect();
    candidates.sort_by_key(|champ| champ.champion_id);

    Ok(analyze_pool(&PoolCoachInput {
        role,
        pool,
        candidates,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommendation::draft_iq::archetype::{CcProfile, DamageProfile, PowerCurve};

    fn arch(
        archetype: &str,
        blind: f32,
        exec: u8,
        engage_role: &str,
        peel: &str,
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
                has_hard_cc: false,
                hard_cc_count: 0,
                primary_cc: vec![],
            },
            mobility: "medium".to_string(),
            engage_role: engage_role.to_string(),
            peel_capability: peel.to_string(),
            blind_safety: blind,
            execution_difficulty: exec,
            win_condition: "test".to_string(),
            ult_type: "test".to_string(),
            confidence: "medium".to_string(),
            power_curve: PowerCurve {
                early: 0.5,
                mid: 0.6,
                late: 0.8,
            },
            counters_archetypes: vec![],
            utility_tags: vec![],
        }
    }

    #[test]
    fn comfort_uses_mastery_and_match_sample_but_stays_bounded() {
        assert_eq!(personal_comfort(0, 0, 0), 0.0);
        assert!(personal_comfort(20_000, 3, 0) > 0.0);
        assert!(personal_comfort(500_000, 7, 500) <= 1.0);
    }

    #[test]
    fn traits_are_resolved_from_kb_signals() {
        let engage = arch("vanguard", 0.7, 2, "primary", "low");
        let peel = arch("enchanter", 0.7, 2, "none", "high");
        assert!(has_engage(&engage));
        assert!(has_peel(&peel));
    }
}
