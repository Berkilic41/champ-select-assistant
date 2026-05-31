import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DraftPlanPanel } from './DraftPlanPanel';
import type { DraftPlan } from '../../types/recommendation';

const fullPlan: DraftPlan = {
  combo_with: [
    {
      ally_champion_id: 56,
      ally_champion_key: 'Nocturne',
      combo_text: 'Nocturne R engage → Orianna E+R burst',
      combo_type: 'engage_followup',
    },
  ],
  win_condition: 'Takım dövüşü komposu — engage ile galip gel',
  team_role: 'Zone + CC kontrol büyücüsü',
  damage_profile: 'AP burst / kontrol',
  blind_pick_safety: 0.85,
  execution_difficulty: 4,
  threats: ['Yüksek execution riski (zorluk 4/5)'],
  fills_team_need: ['Engage eksiği giderilir', 'CC eksiği giderilir'],
};

describe('DraftPlanPanel', () => {
  it('renders nothing when plan is undefined', () => {
    const { container } = render(<DraftPlanPanel />);
    expect(container.firstChild).toBeNull();
  });

  it('renders partial plan (only win_condition, no combo or fills)', () => {
    const partial: DraftPlan = {
      combo_with: [],
      win_condition: 'Split push planı',
      team_role: 'Juggernaut',
      damage_profile: 'AD fiziksel hasar',
      blind_pick_safety: 0.7,
      execution_difficulty: 2,
      threats: [],
      fills_team_need: [],
    };
    render(<DraftPlanPanel plan={partial} />);
    expect(screen.getByText('Split push planı')).toBeInTheDocument();
    expect(screen.queryByText('Combo Planları')).not.toBeInTheDocument();
    expect(screen.queryByText('Takım İhtiyacı')).not.toBeInTheDocument();
    expect(screen.queryByText('Riskler')).not.toBeInTheDocument();
  });

  it('renders all four sections when plan is full', () => {
    render(<DraftPlanPanel plan={fullPlan} />);
    expect(screen.getByText('Galibiyet Koşulu')).toBeInTheDocument();
    expect(screen.getByText('Combo Planları')).toBeInTheDocument();
    expect(screen.getByText('Takım İhtiyacı')).toBeInTheDocument();
    expect(screen.getByText('Riskler')).toBeInTheDocument();
    // combo text rendered
    expect(screen.getByText(/Nocturne R engage/)).toBeInTheDocument();
    // chip content
    expect(screen.getByText('Engage eksiği giderilir')).toBeInTheDocument();
    expect(screen.getByText('CC eksiği giderilir')).toBeInTheDocument();
  });

  it('renders damage_profile when provided', () => {
    render(<DraftPlanPanel plan={fullPlan} />);
    expect(screen.getByText('Hasar Profili')).toBeInTheDocument();
    expect(screen.getByText('AP burst / kontrol')).toBeInTheDocument();
  });

  it('renders blind_pick_safety as safe badge when >= 0.75', () => {
    render(<DraftPlanPanel plan={fullPlan} />);
    expect(screen.getByText('Pick Güvenliği')).toBeInTheDocument();
    expect(screen.getByText('Blind pick güvenli')).toBeInTheDocument();
  });

  it('renders execution_difficulty as hard badge when >= 4', () => {
    render(<DraftPlanPanel plan={fullPlan} />);
    expect(screen.getByText('Execution Zorluğu')).toBeInTheDocument();
    expect(screen.getByText(/Zor \(4\/5\)/)).toBeInTheDocument();
  });

  it('renders lane_phase_advice under "Lane Mikrosu" label', () => {
    const lanePlan: DraftPlan = {
      ...fullPlan,
      lane_phase_advice: 'Lvl 1-3 push penceresi — düşman ölçekleniyor, ilk plate için baskı kur',
    };
    render(<DraftPlanPanel plan={lanePlan} />);
    expect(screen.getByText('Lane Mikrosu')).toBeInTheDocument();
    expect(screen.getByText(/Lvl 1-3 push penceresi/)).toBeInTheDocument();
  });

  it('renders spike_note under "Güç Tepesi" label', () => {
    const spikePlan: DraftPlan = {
      ...fullPlan,
      spike_note: '2–3 item sonrası dominant — erken baskıyı savun',
    };
    render(<DraftPlanPanel plan={spikePlan} />);
    expect(screen.getByText('Güç Tepesi')).toBeInTheDocument();
    expect(screen.getByText(/2–3 item sonrası dominant/)).toBeInTheDocument();
  });

  it('renders comp_clash_note under "Kompo Çakışması" label', () => {
    const clashPlan: DraftPlan = {
      ...fullPlan,
      comp_clash_note: 'Pick-comp\'e karşı poke: vision basın, izole durmayın',
    };
    render(<DraftPlanPanel plan={clashPlan} />);
    expect(screen.getByText('Kompo Çakışması')).toBeInTheDocument();
    expect(screen.getByText(/vision basın/)).toBeInTheDocument();
  });

  it('renders stretch pick risk_note with alert role', () => {
    const stretchPlan: DraftPlan = {
      ...fullPlan,
      risk_note: 'Bu pick\'i hiç oynamadın — combo planına bağımlı, execution riski yüksek',
    };
    render(<DraftPlanPanel plan={stretchPlan} />);
    const alert = screen.getByRole('alert');
    expect(alert).toBeInTheDocument();
    expect(alert.textContent).toContain('hiç oynamadın');
  });
});
