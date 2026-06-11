//! In-game macro/objective timer core (Faz 4 overlay — Sprint, Claude) — engine-pure.
//!
//! Given the current game time + the objectives already taken (from the official Riot
//! **Live Client Data API**, port 2999), it computes the next neutral-objective windows
//! and short macro reminders. The spawn/respawn values are **public game rules**
//! (constants, not fabricated data); the engine just does the arithmetic.
//!
//! Policy-safe by construction: it consumes only game time + your team's objective
//! takes (public, on-screen info). **No hidden information** — no enemy cooldowns,
//! wards, or summoner timers. Pure: no LCU / network / window here; the overlay runtime
//! (later) polls the Live Client Data API and feeds this. Objective / state / phase
//! names are stable machine keys (UI i18n-maps); prose reminders are TR.
#![allow(dead_code)] // public DTOs + engine consumed by the overlay runtime later

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const OBJECTIVES: [&str; 4] = ["grubs", "herald", "dragon", "baron"];
pub const OBJECTIVE_STATES: [&str; 4] = ["pending_first", "respawning", "soon", "up"];
pub const GAME_PHASES: [&str; 3] = ["early", "mid", "late"];

// ── Public game rules (current Summoner's Rift, seconds). Adjust per patch. ───────
const GRUBS_FIRST: u32 = 6 * 60; // Voidgrubs spawn 6:00 (despawn handled by caller events)
const HERALD_FIRST: u32 = 14 * 60; // Rift Herald 14:00
const DRAGON_FIRST: u32 = 5 * 60; // First drake 5:00
const DRAGON_RESPAWN: u32 = 5 * 60; // 5:00 after a take
const BARON_FIRST: u32 = 25 * 60; // Baron 25:00
const BARON_RESPAWN: u32 = 6 * 60; // 6:00 after a take
/// Window (s) before spawn at which we flag an objective as `soon`.
const SOON_WINDOW: u32 = 45;

// ── Inputs (caller-built; Rust-only) ─────────────────────────────────────────────

/// One objective already taken (by either team), from a Live Client Data event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveEvent {
    /// "grubs" | "herald" | "dragon" | "baron".
    pub objective: String,
    pub killed_at_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroTimerInput {
    pub game_time_secs: u32,
    pub events: Vec<ObjectiveEvent>,
}

// ── Outputs (ts-rs; all u32/i32 → number, no bigint) ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct ObjectiveTimer {
    pub objective: String,
    /// Game-time second of the next spawn.
    pub next_spawn_secs: u32,
    /// next_spawn − now (negative ⇒ available now).
    pub seconds_until: i32,
    /// `OBJECTIVE_STATES`.
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct MacroState {
    pub game_time_secs: u32,
    /// `GAME_PHASES`.
    pub phase: String,
    pub objectives: Vec<ObjectiveTimer>,
    /// TR macro reminders for objectives that are up / imminent.
    pub reminders: Vec<String>,
    /// TR one-line read of the current macro phase.
    pub phase_note: String,
}

fn last_kill(events: &[ObjectiveEvent], objective: &str) -> Option<u32> {
    events
        .iter()
        .filter(|e| e.objective == objective)
        .map(|e| e.killed_at_secs)
        .max()
}

/// Next spawn time for an objective with a repeating respawn (dragon/baron).
fn next_repeating(events: &[ObjectiveEvent], objective: &str, first: u32, respawn: u32) -> u32 {
    match last_kill(events, objective) {
        Some(killed) => killed + respawn,
        None => first,
    }
}

/// Next spawn for a one-off objective (grubs/herald): once taken, it does not return.
fn next_one_off(events: &[ObjectiveEvent], objective: &str, first: u32) -> Option<u32> {
    if last_kill(events, objective).is_some() {
        None // already taken — no respawn
    } else {
        Some(first)
    }
}

fn timer(objective: &str, next_spawn: u32, now: u32) -> ObjectiveTimer {
    let seconds_until = next_spawn as i32 - now as i32;
    let state = if seconds_until <= 0 {
        "up"
    } else if (seconds_until as u32) <= SOON_WINDOW {
        "soon"
    } else if now >= first_spawn_of(objective) {
        "respawning"
    } else {
        "pending_first"
    };
    ObjectiveTimer {
        objective: objective.to_string(),
        next_spawn_secs: next_spawn,
        seconds_until,
        state: state.to_string(),
    }
}

fn first_spawn_of(objective: &str) -> u32 {
    match objective {
        "grubs" => GRUBS_FIRST,
        "herald" => HERALD_FIRST,
        "dragon" => DRAGON_FIRST,
        "baron" => BARON_FIRST,
        _ => 0,
    }
}

fn reminder_for(objective: &str, state: &str) -> Option<&'static str> {
    match (objective, state) {
        ("dragon", "up") => Some("Dragon hazır — vision kur, tempo penceresini değerlendir."),
        ("dragon", "soon") => Some("Dragon yakında — nehir görüşü ve pozisyon al."),
        ("baron", "up") => Some("Baron hazır — vision kontrolü olmadan başlatma."),
        ("baron", "soon") => Some("Baron yakında — ward temizliği ve sayı üstünlüğü ara."),
        ("herald", "up") => Some("Herald hazır — plate/tempo için al, güvenli alırsan."),
        ("herald", "soon") => Some("Herald yakında — üst nehir kontrolünü hazırla."),
        ("grubs", "up") => Some("Voidgrubs hazır — erken jungle tempo + obje hasarı için al."),
        ("grubs", "soon") => Some("Voidgrubs yakında — jungle yol planını buna göre kur."),
        _ => None,
    }
}

fn phase_of(now: u32) -> &'static str {
    if now < HERALD_FIRST {
        "early"
    } else if now < BARON_FIRST {
        "mid"
    } else {
        "late"
    }
}

fn phase_note_for(phase: &str) -> &'static str {
    match phase {
        "early" => "Erken oyun: lane tempo + ilk dragon/grubs görüşü, gereksiz ölme yok.",
        "mid" => "Orta oyun: obje etrafında grupla, herald/dragon için vision önceliği.",
        _ => "Geç oyun: Baron/Elder kritik — vision kontrolü, sayı eksikken obje başlatma.",
    }
}

/// Compute the macro state (objective timers + reminders) for the current game time.
/// Pure + deterministic.
pub fn compute_macro_state(input: &MacroTimerInput) -> MacroState {
    let now = input.game_time_secs;
    let mut objectives = Vec::new();

    // Repeating objectives.
    for (obj, first, respawn) in [
        ("dragon", DRAGON_FIRST, DRAGON_RESPAWN),
        ("baron", BARON_FIRST, BARON_RESPAWN),
    ] {
        let next = next_repeating(&input.events, obj, first, respawn);
        objectives.push(timer(obj, next, now));
    }
    // One-off objectives (drop once taken).
    for (obj, first) in [("grubs", GRUBS_FIRST), ("herald", HERALD_FIRST)] {
        if let Some(next) = next_one_off(&input.events, obj, first) {
            objectives.push(timer(obj, next, now));
        }
    }

    // Deterministic order: soonest meaningful spawn first, then objective name.
    objectives.sort_by(|a, b| {
        a.seconds_until
            .max(0)
            .cmp(&b.seconds_until.max(0))
            .then(a.objective.cmp(&b.objective))
    });

    let reminders: Vec<String> = objectives
        .iter()
        .filter(|t| t.state == "up" || t.state == "soon")
        .filter_map(|t| reminder_for(&t.objective, &t.state))
        .map(String::from)
        .collect();

    let phase = phase_of(now);
    MacroState {
        game_time_secs: now,
        phase: phase.to_string(),
        objectives,
        reminders,
        phase_note: phase_note_for(phase).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(now: u32, events: &[(&str, u32)]) -> MacroTimerInput {
        MacroTimerInput {
            game_time_secs: now,
            events: events
                .iter()
                .map(|(o, t)| ObjectiveEvent {
                    objective: (*o).to_string(),
                    killed_at_secs: *t,
                })
                .collect(),
        }
    }

    fn find<'a>(s: &'a MacroState, obj: &str) -> Option<&'a ObjectiveTimer> {
        s.objectives.iter().find(|t| t.objective == obj)
    }

    #[test]
    fn dragon_pending_then_up_at_first_spawn() {
        let early = compute_macro_state(&input(120, &[])); // 2:00
        let d = find(&early, "dragon").unwrap();
        assert_eq!(d.next_spawn_secs, DRAGON_FIRST);
        assert_eq!(d.state, "pending_first");
        assert_eq!(d.seconds_until, 180);

        let up = compute_macro_state(&input(300, &[])); // 5:00
        assert_eq!(find(&up, "dragon").unwrap().state, "up");
    }

    #[test]
    fn dragon_respawns_five_minutes_after_a_take() {
        // Dragon taken at 6:00 → next at 11:00.
        let s = compute_macro_state(&input(7 * 60, &[("dragon", 6 * 60)]));
        let d = find(&s, "dragon").unwrap();
        assert_eq!(d.next_spawn_secs, 11 * 60);
        assert_eq!(d.state, "respawning");
        assert_eq!(d.seconds_until, 4 * 60);
    }

    #[test]
    fn soon_window_flags_imminent_objective_with_reminder() {
        // 20s before baron first spawn (25:00) → soon + reminder.
        let s = compute_macro_state(&input(25 * 60 - 20, &[]));
        let b = find(&s, "baron").unwrap();
        assert_eq!(b.state, "soon");
        assert!(s.reminders.iter().any(|r| r.contains("Baron yakında")));
    }

    #[test]
    fn one_off_grubs_disappear_once_taken() {
        let before = compute_macro_state(&input(5 * 60, &[]));
        assert!(find(&before, "grubs").is_some());
        let after = compute_macro_state(&input(8 * 60, &[("grubs", 6 * 60)]));
        assert!(find(&after, "grubs").is_none(), "grubs do not respawn");
    }

    #[test]
    fn baron_up_emits_vision_reminder() {
        let s = compute_macro_state(&input(25 * 60 + 5, &[]));
        assert_eq!(find(&s, "baron").unwrap().state, "up");
        assert!(s.reminders.iter().any(|r| r.contains("vision kontrolü")));
    }

    #[test]
    fn phase_transitions_early_mid_late() {
        assert_eq!(compute_macro_state(&input(60, &[])).phase, "early");
        assert_eq!(compute_macro_state(&input(15 * 60, &[])).phase, "mid");
        assert_eq!(compute_macro_state(&input(30 * 60, &[])).phase, "late");
    }

    #[test]
    fn objectives_sorted_by_soonest_and_tokens_locked() {
        let s = compute_macro_state(&input(13 * 60, &[("dragon", 6 * 60)]));
        // Every emitted token stays in its vocabulary.
        for t in &s.objectives {
            assert!(
                OBJECTIVES.contains(&t.objective.as_str()),
                "obj {}",
                t.objective
            );
            assert!(
                OBJECTIVE_STATES.contains(&t.state.as_str()),
                "state {}",
                t.state
            );
        }
        assert!(GAME_PHASES.contains(&s.phase.as_str()));
        // Sorted by clamped seconds_until ascending.
        let ups: Vec<i32> = s
            .objectives
            .iter()
            .map(|t| t.seconds_until.max(0))
            .collect();
        let mut sorted = ups.clone();
        sorted.sort();
        assert_eq!(ups, sorted);
    }

    #[test]
    fn no_fabrication_no_events_no_kill_based_timers() {
        // With no events, repeating objectives use first-spawn, one-offs are pending.
        let s = compute_macro_state(&input(0, &[]));
        assert_eq!(find(&s, "dragon").unwrap().next_spawn_secs, DRAGON_FIRST);
        assert_eq!(find(&s, "baron").unwrap().next_spawn_secs, BARON_FIRST);
        assert!(find(&s, "grubs").is_some());
        assert!(find(&s, "herald").is_some());
    }
}
