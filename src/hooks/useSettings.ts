import { useState, useEffect, useCallback } from 'react';
import { invoke } from '../lib/host';

export interface AppSettings {
  weight_comfort: number;
  weight_matchup: number;
  weight_team_counter: number;
  weight_synergy: number;
  weight_meta: number;
  weight_role_fit: number;
  always_on_top: boolean;
  window_size: 'compact' | 'standard' | 'wide';
  auto_hide_in_game: boolean;
  sounds_enabled: boolean;
  language?: 'tr' | 'en';
  platform_region: string;
  // Privacy opt-in: upload anonymized recommendation feedback. OFF by default.
  share_anonymous_feedback: boolean;
}

// Normalize the six recommendation weights to sum to 1.0.
// Module-level so it can be used both on load and on save.
export function normalizeWeights(s: AppSettings): AppSettings {
  const total =
    s.weight_comfort +
    s.weight_matchup +
    s.weight_team_counter +
    s.weight_synergy +
    s.weight_meta +
    s.weight_role_fit;
  if (total === 0) return s;
  return {
    ...s,
    weight_comfort: s.weight_comfort / total,
    weight_matchup: s.weight_matchup / total,
    weight_team_counter: s.weight_team_counter / total,
    weight_synergy: s.weight_synergy / total,
    weight_meta: s.weight_meta / total,
    weight_role_fit: s.weight_role_fit / total,
  };
}

// Weights sum to exactly 1.0 so the Settings panel Save button is never
// disabled on first load. Backend defaults were 0.95 (missing 0.05).
const DEFAULT_SETTINGS: AppSettings = {
  weight_comfort: 0.25,        // +0.05 vs backend to reach 1.00 total
  weight_matchup: 0.25,
  weight_team_counter: 0.15,
  weight_synergy: 0.10,
  weight_meta: 0.15,
  weight_role_fit: 0.10,
  always_on_top: true,
  window_size: 'standard',
  auto_hide_in_game: false,
  sounds_enabled: false,
  language: 'tr',
  platform_region: 'tr1',
  share_anonymous_feedback: false,
};

export { DEFAULT_SETTINGS };

/** Recommendation weight fields (the 6 sliders). */
export type WeightFields = Pick<
  AppSettings,
  | 'weight_comfort'
  | 'weight_matchup'
  | 'weight_team_counter'
  | 'weight_synergy'
  | 'weight_meta'
  | 'weight_role_fit'
>;

export type WeightPresetName = 'balanced' | 'soloq' | 'flex' | 'otp';

/**
 * Quick weight profiles. Each sums to 1.0. `balanced` matches the backend
 * default (also the "reset"); `soloq` leans on lane matchup + role fit; `flex`
 * on team synergy + counter; `otp` on personal comfort.
 */
export const WEIGHT_PRESETS: Record<WeightPresetName, WeightFields> = {
  balanced: {
    weight_comfort: 0.2,
    weight_matchup: 0.25,
    weight_team_counter: 0.15,
    weight_synergy: 0.1,
    weight_meta: 0.15,
    weight_role_fit: 0.15,
  },
  soloq: {
    weight_comfort: 0.15,
    weight_matchup: 0.28,
    weight_team_counter: 0.12,
    weight_synergy: 0.08,
    weight_meta: 0.15,
    weight_role_fit: 0.22,
  },
  flex: {
    weight_comfort: 0.15,
    weight_matchup: 0.18,
    weight_team_counter: 0.22,
    weight_synergy: 0.2,
    weight_meta: 0.12,
    weight_role_fit: 0.13,
  },
  otp: {
    weight_comfort: 0.4,
    weight_matchup: 0.18,
    weight_team_counter: 0.1,
    weight_synergy: 0.07,
    weight_meta: 0.12,
    weight_role_fit: 0.13,
  },
};

/** True when `s`'s weights match `preset` within a small tolerance. */
export function matchesPreset(s: WeightFields, preset: WeightFields): boolean {
  const keys: (keyof WeightFields)[] = [
    'weight_comfort',
    'weight_matchup',
    'weight_team_counter',
    'weight_synergy',
    'weight_meta',
    'weight_role_fit',
  ];
  return keys.every((k) => Math.abs(s[k] - preset[k]) < 0.005);
}

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    invoke<AppSettings>('get_settings')
      .then(s => {
        // Normalize on load so weights always sum to 1.0 regardless of
        // what was stored (backend defaults were 0.95 pre-fix).
        setSettings(normalizeWeights(s));
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  const save = useCallback(
    async (next: AppSettings) => {
      setSettings(next);
      await invoke('save_settings', { settings: next }).catch(console.error);
      if (next.always_on_top !== settings.always_on_top) {
        await invoke('set_always_on_top', { enabled: next.always_on_top }).catch(() => {});
      }
      if (next.window_size !== settings.window_size) {
        await invoke('set_window_size', { preset: next.window_size }).catch(() => {});
      }
    },
    [settings],
  );

  return {
    settings,
    save: (s: AppSettings) => save(normalizeWeights(s)),
    loaded,
  };
}
