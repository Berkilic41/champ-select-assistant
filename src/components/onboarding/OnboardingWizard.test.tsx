import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '../../lib/host';
import { OnboardingWizard } from './OnboardingWizard';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('OnboardingWizard', () => {
  beforeEach(() => mockInvoke.mockReset());

  it('triggers LCU player-data sync on completion so a new user lands with data', async () => {
    mockInvoke.mockResolvedValue(undefined);
    const onComplete = vi.fn();
    render(<OnboardingWizard onComplete={onComplete} />);

    // Step through the 4 screens (3× next, then start).
    fireEvent.click(screen.getByText('İleri'));
    fireEvent.click(screen.getByText('İleri'));
    fireEvent.click(screen.getByText('İleri'));
    fireEvent.click(screen.getByText('Başla!'));

    await waitFor(() => expect(onComplete).toHaveBeenCalled());
    expect(mockInvoke).toHaveBeenCalledWith('complete_onboarding');
    expect(mockInvoke).toHaveBeenCalledWith('sync_ddragon_champions');
    expect(mockInvoke).toHaveBeenCalledWith('sync_lcu_player_data', {});
    // DDragon sync, LCU mastery sync'inden ÖNCE gelmeli (placeholder numeric key önlensin).
    const order = mockInvoke.mock.calls.map((c) => c[0]);
    expect(order.indexOf('sync_ddragon_champions')).toBeLessThan(
      order.indexOf('sync_lcu_player_data'),
    );
  });
});
