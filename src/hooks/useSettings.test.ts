import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import {
  useSettings,
  normalizeWeights,
  DEFAULT_SETTINGS,
  WEIGHT_PRESETS,
  matchesPreset,
  type AppSettings,
  type WeightPresetName,
} from './useSettings';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('normalizeWeights', () => {
  it('scales the six weights to sum to 1.0', () => {
    const s: AppSettings = {
      ...DEFAULT_SETTINGS,
      weight_comfort: 2, weight_matchup: 2, weight_team_counter: 2,
      weight_synergy: 2, weight_meta: 1, weight_role_fit: 1,
    };
    const n = normalizeWeights(s);
    const total =
      n.weight_comfort + n.weight_matchup + n.weight_team_counter +
      n.weight_synergy + n.weight_meta + n.weight_role_fit;
    expect(total).toBeCloseTo(1.0, 6);
    // relative proportions preserved (comfort was 2/10)
    expect(n.weight_comfort).toBeCloseTo(0.2, 6);
  });

  it('returns settings unchanged when all weights are zero', () => {
    const s: AppSettings = {
      ...DEFAULT_SETTINGS,
      weight_comfort: 0, weight_matchup: 0, weight_team_counter: 0,
      weight_synergy: 0, weight_meta: 0, weight_role_fit: 0,
    };
    expect(normalizeWeights(s)).toEqual(s);
  });

  it('DEFAULT_SETTINGS already sums to 1.0', () => {
    const d = DEFAULT_SETTINGS;
    const total =
      d.weight_comfort + d.weight_matchup + d.weight_team_counter +
      d.weight_synergy + d.weight_meta + d.weight_role_fit;
    expect(total).toBeCloseTo(1.0, 6);
  });
});

describe('useSettings', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('loads + normalizes settings from get_settings on mount', async () => {
    mockInvoke.mockResolvedValueOnce({
      ...DEFAULT_SETTINGS,
      weight_comfort: 0.5, weight_matchup: 0.5, weight_team_counter: 0,
      weight_synergy: 0, weight_meta: 0, weight_role_fit: 0,
    });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.loaded).toBe(true));
    expect(result.current.settings.weight_comfort).toBeCloseTo(0.5, 6);
    expect(result.current.settings.weight_matchup).toBeCloseTo(0.5, 6);
  });

  it('save persists normalized settings via save_settings', async () => {
    mockInvoke.mockResolvedValue(DEFAULT_SETTINGS); // get_settings
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.loaded).toBe(true));
    mockInvoke.mockClear();
    mockInvoke.mockResolvedValue(undefined);

    await act(async () => {
      await result.current.save({ ...DEFAULT_SETTINGS, language: 'en' });
    });
    expect(mockInvoke).toHaveBeenCalledWith(
      'save_settings',
      expect.objectContaining({ settings: expect.objectContaining({ language: 'en' }) }),
    );
  });
});

describe('weight presets', () => {
  it('every preset sums to ~1.0', () => {
    (Object.keys(WEIGHT_PRESETS) as WeightPresetName[]).forEach((name) => {
      const p = WEIGHT_PRESETS[name];
      const sum =
        p.weight_comfort + p.weight_matchup + p.weight_team_counter +
        p.weight_synergy + p.weight_meta + p.weight_role_fit;
      expect(Math.abs(sum - 1)).toBeLessThan(0.001);
    });
  });

  it('matchesPreset detects an exact match and rejects a different one', () => {
    expect(matchesPreset(WEIGHT_PRESETS.soloq, WEIGHT_PRESETS.soloq)).toBe(true);
    expect(matchesPreset(WEIGHT_PRESETS.otp, WEIGHT_PRESETS.balanced)).toBe(false);
  });
});
