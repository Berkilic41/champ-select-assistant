import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { invoke } from '../../lib/host';
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

  it('surfaces the KB mobility tier and utility tags (champion profile)', async () => {
    mockInvoke.mockResolvedValueOnce({
      champion_id: 64,
      champion_key: 'LeeSin',
      archetype: 'diver',
      power_early: 0.8,
      power_mid: 0.6,
      power_late: 0.4,
      win_condition: 'skirmish',
      damage_ad: 0.8,
      damage_ap: 0.1,
      has_hard_cc: true,
      mobility: 'high',
      blind_safety: 0.5,
      execution_difficulty: 5,
      utility_tags: ['engage', 'frontline'],
      combos: [],
    });
    render(<ChampionDetailCard championId={64} championKey="LeeSin" onClose={() => {}} />);
    // Mobility rozeti (yeni) — yükleme tamamlanma sinyali olarak da kullanılır.
    await waitFor(() => expect(screen.getByText('Hareketlilik: High')).toBeInTheDocument());
    // Utility bölümü + etiketler (engage→"Engage", frontline→"Ön saf").
    expect(screen.getByText('Takım katkısı')).toBeInTheDocument();
    expect(screen.getByText('Engage')).toBeInTheDocument();
    expect(screen.getByText('Ön saf')).toBeInTheDocument();
  });
});
