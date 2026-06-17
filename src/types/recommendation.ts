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
  /** Ability-by-ability mechanical breakdown (English ability names) — deep-dive "how to execute" detail. */
  ability_ref?: string;
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

export interface DataSourceBadge {
  source: string;
  patch?: string | null;
  sample_size?: number | null;
  confidence: 'high' | 'medium' | 'low' | 'none' | 'heuristic' | string;
  risk_level: 'low' | 'medium' | 'high' | string;
  updated_at?: number | null;
}

export interface ScoreBreakdownItem {
  key: string;
  label: string;
  value: number;
  weight: number;
  contribution: number;
  confidence: string;
}

/** FAZ 3 / 3A — bu öneri skoru seviyesinde kalibre kazanma olasılığı (yerel
 *  outcome etiketlerinden; min-sample altında HİÇ iliştirilmez). Saf görsel
 *  augment: total_score'u DEĞİŞTİRMEZ. core win_calibration::WinProbEstimate. */
export interface WinProbEstimate {
  score: number;
  probability: number;
  confidence: string;
  sample_size: number;
}

/** FAZ 3 / 3C — bu pick'in birincil combo müttefikiyle oyuncunun co-pick geçmişi
 *  (ham maç/galibiyet). Saf görsel; ≥2 maç varsa iliştirilir. scoring'e dokunmaz. */
export interface ComboRecord {
  games: number;
  wins: number;
}

/** FAZ 4 / Sprint 1 — grounded koçluk notu (deterministik ya da audit'i geçmiş
 *  LLM adayı). DeepDive'da gösterilir. core coach_narrator::CoachNarrative. */
export interface CoachNarrative {
  text: string;
  source: string;
  external_rejected: boolean;
}

export interface Recommendation {
  champion_id: number;
  champion_key: string;   // "Aatrox" — icon için
  champion_name: string;  // "Aatrox" — display name (from backend models.rs)
  total_score: number;
  comfort_score: number;
  /** Mechanical comfort from Champion Mastery alone (no win-rate), 0..1. */
  mechanical_comfort?: number;
  /** Bayesian-shrunk personal win-rate 0..1, or null/absent when no games on champ. */
  draft_winrate?: number | null;
  /** Lane matchup vs opposite-lane opponent (renamed from lane_counter_score in Sprint E). */
  matchup_score: number;
  team_counter_score: number;
  synergy_score: number;
  meta_score: number;
  role_fit_score: number;
  risk_score: number;
  reason: string;
  /** Punchy one-line headline for the hero (champion's strongest grounded driver). */
  headline?: string | null;
  decision_sentence?: string;
  score_breakdown?: ScoreBreakdownItem[];
  model_score?: number;
  rules_score?: number;
  model_version?: string;
  /** Build provenance: "seed" (curated), "general" (archetype heuristic — shown
   *  with a "Genel öneri" badge, no winrate claim), or "none". Always set by the
   *  backend; optional here so lightweight test fixtures may omit it. */
  build_source?: string;
  build_confidence?: 'high' | 'medium' | 'low' | 'none' | string;
  /** Short honest rationale for the build (core intent + lane-opponent counter). */
  build_note?: string;
  matchup_confidence?: 'high' | 'medium' | 'low' | 'none' | 'heuristic' | string;
  /** Signals that fell back to a placeholder (no real data): 'meta' | 'matchup' | 'build'. */
  missing_signals?: string[];
  /** Pro-play presence (pick% + ban%) from Leaguepedia, when available. */
  pro_presence?: number | null;
  lane_plan?: string | null;
  mid_game_plan?: string | null;
  teamfight_plan?: string | null;
  teamfight_job?: string | null;
  fallback_plan?: string | null;
  risk_summary?: string | null;
  why_not?: string[];
  data_sources?: DataSourceBadge[];
  coach_depth?: string;
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
  /** What drove confidence: 'sample_depth' | 'signal_convergence' | 'games_only'. */
  confidence_basis?: string;
  games_on_champ: number;
  wins_on_champ: number;
  /** Turkish summary of enemy team composition, e.g. "AP ağırlıklı · frontline yok" */
  enemy_team_summary: string;
  /** Resolved lane opponent display name; grounds the prose in the matchup. */
  lane_opponent_name?: string | null;
  /** First core build item display name; grounds the macro prose in the build. */
  core_item_name?: string | null;
  /** Draft IQ analysis; absent until DI-4b wires the analyzer. */
  draft_plan?: DraftPlan;
  /** Phase-based matchup advantage [early, mid, late] in [0.0, 1.0]. Present when opponent is visible and KB power-curve data exists. 0.5 = neutral. */
  phase_matchup?: [number, number, number];
  /** FAZ 3 / 3A — host'un iliştirdiği kalibre kazanma olasılığı (yeterli yerel
   *  etiket varsa); yoksa absent → HeroCard badge göstermez. */
  win_prob?: WinProbEstimate | null;
  /** FAZ 3 / 3C — birincil combo müttefikiyle co-pick geçmişi (≥2 maç varsa);
   *  yoksa absent → HeroCard combo satırına geçmiş eklemez. */
  combo_history?: ComboRecord | null;
  /** FAZ 4 / Sprint 1 — host'un iliştirdiği grounded koçluk notu; DeepDive'da
   *  "Koç notu" olarak gösterilir. Yoksa absent → bölüm gizli. */
  coach_narrative?: CoachNarrative | null;
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

/** One phase band of the macro game-plan timeline. */
export interface PhaseBand {
  label: string;
  /** Your team vs enemy in this phase. */
  stance: 'advantage' | 'even' | 'disadvantage';
  advice: string;
  /** Aggregate team strength 0..1 (UI bar). */
  strength: number;
}

/** Team-level macro game plan (#6). Computed from ally + enemy compositions. */
export interface GamePlan {
  team_identity: string;
  identity_note: string;
  timeline: PhaseBand[];
  win_conditions: string[];
  objectives: string[];
  enemy_threat: string;
  alt_plan?: string | null;
  /** True when the draft is incomplete — the plan sharpens as champions lock. */
  partial: boolean;
}

/** A champion from the player's pool that counters the visible lane opponent. */
export interface CounterPickHint {
  champion_id: number;
  champion_key: string;
  champion_name: string;
  /** 0..1 matchup advantage vs the lane opponent. */
  advantage: number;
  reason: string;
  games_on_champ: number;
  /** Power-curve advantage vs the opponent per phase [early, mid, late] (0.5 = even). */
  phase_advantage?: [number, number, number];
}

/** One team's composition summary for the draft board. */
export interface CompSummary {
  tanks: number;
  fighters: number;
  mages: number;
  marksmen: number;
  assassins: number;
  supports: number;
  /** Average AP / AD damage share across known archetypes (0..1). */
  ap_share: number;
  ad_share: number;
  has_engage: boolean;
  has_frontline: boolean;
  has_hard_cc: boolean;
  has_peel: boolean;
  /** Missing utilities, e.g. "Engage yok". */
  gaps: string[];
  summary: string;
}

/** Both teams' composition summaries. */
export interface TeamCompBoard {
  ally: CompSummary;
  enemy: CompSummary;
}

/** One ally-combo entry for the synergy board (local pick × an ally). */
export interface ComboBoardEntry {
  ally_champion_id: number;
  ally_champion_key: string;
  name: string;
  /** Turkish combo description (the `tr` field). */
  combo_text: string;
  combo_type: ComboType;
  strength: number;
}

/** A combo a champion participates in (for the lobby detail card). */
export interface ChampionDetailCombo {
  partner_key: string;
  name: string;
  combo_text: string;
  combo_type: ComboType;
  strength: number;
}

/** One defensive counter-itemization advisory vs the enemy comp. */
export interface CounterItemHint {
  category: string;
  reason: string;
  /** Item IDs (empty for text-only categories like anti-heal / tenacity). */
  item_ids: number[];
}

/** A champion suggested to learn for a role (pool builder). */
export interface PoolSuggestion {
  champion_id: number;
  champion_key: string;
  archetype: string;
  reason: string;
  /** 1 (hard) .. 5 (easy). */
  ease: number;
  /** Meta tier (S/A/B/C) when the blended sample is sufficient, else absent. */
  meta_tier?: string | null;
}

/** Lane matchup read: local pick vs the visible lane opponent. */
export interface LaneMatchup {
  opponent_key: string;
  opponent_name: string;
  /** [early, mid, late] advantage 0..1 (0.5 = even). */
  phase_advantage: [number, number, number];
  /** Provenance of phase_advantage. 'kb_estimate' = archetype power-curve estimate
   *  (current — NOT measured win-rates) → UI shows a "KB tahmini" badge. */
  source?: 'kb_estimate' | 'measured';
  tips: string[];
  /** True when the opponent was inferred (Blind/Normal, no LCU positions). */
  inferred?: boolean;
}

/** Single decisive read on the draft (favorability + dodge + top action). */
export interface DraftVerdict {
  favorability: 'favorable' | 'even' | 'risky';
  score: number;
  headline: string;
  reasons: string[];
  dodge_consider: boolean;
  dodge_note?: string | null;
  top_action: string;
  team_needs: string[];
}

/** Full KB profile for one champion (lobby detail card). */
export interface ChampionDetail {
  champion_id: number;
  champion_key: string;
  archetype: string;
  power_early: number;
  power_mid: number;
  power_late: number;
  win_condition: string;
  damage_ad: number;
  damage_ap: number;
  has_hard_cc: boolean;
  mobility: string;
  blind_safety: number;
  execution_difficulty: number;
  utility_tags: string[];
  combos: ChampionDetailCombo[];
}
