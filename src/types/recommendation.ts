export type Tier = 's' | 'a' | 'b' | 'c';

export type ComboType =
  | 'engage_followup'
  | 'pick_potential'
  | 'zone_control'
  | 'peel_chain'
  | 'wombo';

export interface ComboHint {
  ally_champion_id: number;
  ally_champion_key: string;
  combo_text: string;
  combo_type: ComboType;
}

export interface DraftPlan {
  combo_with: ComboHint[];
  win_condition: string;
  team_role: string;
  damage_profile: string;
  blind_pick_safety: number;
  execution_difficulty: number;
  threats: string[];
  fills_team_need: string[];
  risk_note?: string;
  /** Late-pick counter-pick window note — present when pick_order ≥ 4 and candidate counters a visible enemy. */
  pick_window_note?: string;
  /** Structural clash between candidate's win condition and detected enemy comp. e.g. "Pick-comp'e karşı poke: vision basın" */
  comp_clash_note?: string;
  /** Power-spike / item-breakpoint advisory derived from power_curve + archetype. */
  spike_note?: string;
  /** Lane-phase micro-coaching (0-15dk): freeze / push / lvl 2 all-in / lvl 6 roam. */
  lane_phase_advice?: string;
}

export interface Recommendation {
  champion_id: number;
  champion_key: string;   // "Aatrox" — icon için
  champion_name: string;  // "Aatrox" — display name (from backend models.rs)
  total_score: number;
  comfort_score: number;
  /** Lane matchup vs opposite-lane opponent (renamed from lane_counter_score in Sprint E). */
  matchup_score: number;
  team_counter_score: number;
  synergy_score: number;
  meta_score: number;
  role_fit_score: number;
  risk_score: number;
  reason: string;
  core_items: number[];
  situational_items: number[];
  primary_rune_tree: number;
  keystone: number;
  /** Skill-order display text (e.g. "Q→W→E"). Absent when no build data. */
  skill_order?: string;
  /** Summoner spell IDs [spell1, spell2] (e.g. [4, 12] = Flash + Teleport). Empty when no data. */
  summoner_spells: number[];
  /** Secondary rune path [tree_id, rune1_id, rune2_id]. Empty when no data. */
  secondary_runes: number[];
  /** Stat shard IDs [offense, flex, defense]. Empty when no data. */
  stat_shards: number[];
  tier: Tier;
  confidence: 'high' | 'medium' | 'low';
  games_on_champ: number;
  wins_on_champ: number;
  /** Turkish summary of enemy team composition, e.g. "AP ağırlıklı · frontline yok" */
  enemy_team_summary: string;
  /** Draft IQ analysis; absent until DI-4b wires the analyzer. */
  draft_plan?: DraftPlan;
  /** Phase-based matchup advantage [early, mid, late] in [0.0, 1.0]. Present when opponent is visible and KB power-curve data exists. 0.5 = neutral. */
  phase_matchup?: [number, number, number];
}

export interface ChampSelectSession {
  my_cell_id: number;
  local_player: TeamSlot;
  my_team: TeamSlot[];
  their_team: TeamSlot[];
  my_bans: number[];
  their_bans: number[];
  phase: 'PLANNING' | 'BAN_PICK' | 'FINALIZATION';
  time_left_ms: number;
  /** "ban" | "pick" | "" — local player's currently active action type */
  action_type: string;
  queue_id: number;
  /** 1-indexed global pick position. 0 = unknown (ban phase or not yet parsed). */
  pick_order: number;
}

export interface TeamSlot {
  cell_id: number;
  champion_id: number;
  intent_champion_id: number;
  assigned_position: string;
  is_locked: boolean;
}

export interface ChampionPersonalStats {
  games: number;
  wins: number;
  losses: number;
  win_rate: number;
  mastery_level: number;
  mastery_points: number;
  last_played_days_ago?: number;
}

export interface BanSuggestion {
  champion_id: number;
  champion_key: string;
  champion_name: string;
  threat_score: number;
  reason: string;
}

export type PhaseView =
  | 'planning'
  | 'ban_acting'
  | 'ban_watching'
  | 'pick_acting'
  | 'pick_watching'
  | 'finalization';

/** Champion pool summary for one enemy slot. play_rate is 0..1 fraction of recent games. */
export interface EnemyPoolSummary {
  cell_id: number;
  top_champion_id: number;
  top_champion_key: string;
  play_rate: number;
  game_count: number;
}
