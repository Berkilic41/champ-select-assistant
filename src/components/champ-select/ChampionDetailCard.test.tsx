import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { ChampionDetailCard } from './ChampionDetailCard';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('ChampionDetailCard', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('renders nothing when championId is null', () => {
    const { container } = render(<ChampionDetailCard championId={null} onClose={() => {}} />);
    expect(container.firstChild).toBeNull();
  });

  it('shows archetype + win condition after fetch', async () => {
    mockInvoke.mockResolvedValueOnce({
      champion_id: 61,
      champion_key: 'Orianna',
      archetype: 'control_mage',
      power_early: 0.4,
      power_mid: 0.7,
      power_late: 0.85,
      win_condition: 'teamfight',
      damage_ad: 0.1,
      damage_ap: 0.85,
      has_hard_cc: true,
      mobility: 'low',
      blind_safety: 0.6,
      execution_difficulty: 4,
      utility_tags: [],
      combos: [],
    });
    render(<ChampionDetailCard championId={61} championKey="Orianna" onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText('Control Mage')).toBeInTheDocument());
    expect(screen.getByText('Teamfight')).toBeInTheDocument();
  });
});
