import { beforeEach, describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '../../lib/host';
import { SettingsPanel } from './SettingsPanel';
import { DEFAULT_SETTINGS } from '../../hooks/useSettings';

const invokeMock = vi.mocked(invoke);

describe('SettingsPanel feedback sync', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('calls the data pipeline refresh command and renders the summary', async () => {
    invokeMock.mockImplementation((cmd) => {
      if (cmd === 'sync_data_pipeline') {
        return Promise.resolve({
          before_status: 'degraded',
          after_status: 'healthy',
          actions: ['refresh_rates', 'refresh_builds', 'refresh_matchups'],
          ddragon_champions: 172,
          meraki_rates: 860,
          builds_imported: 31,
          matchups_imported: 80,
          match_v5_matches: 20,
          match_v5_rates: 100,
          match_v5_matchups: 100,
          match_v5_builds: 100,
          match_v5_errors: 0,
          data_pack_cached: true,
          cache_action: 'promote',
          cache_promoted: true,
          errors: [],
        });
      }
      return Promise.resolve(null);
    });

    render(
      <SettingsPanel
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: "Veri pipeline'ını yenile" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('sync_data_pipeline');
    });
    expect(
      await screen.findByText(
        'Riot 20 maç · DDragon 172 · meta 860 · build 31 · matchup 80 · cache promote edildi · durum sağlıklı',
      ),
    ).toBeInTheDocument();
  });

  it('calls the feedback flush command and renders the summary', async () => {
    invokeMock.mockImplementation((cmd) => {
      if (cmd === 'sync_recommendation_feedback') {
        return Promise.resolve({
          offline: false,
          attempted: 3,
          synced: 2,
          failed: 1,
          skipped_no_hash: 1,
        });
      }
      return Promise.resolve(null);
    });

    render(
      <SettingsPanel
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: "Feedback'i cloud'a gönder" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('sync_recommendation_feedback');
    });
    expect(screen.getByText('2 gönderildi · 1 hata · 1 hash yok')).toBeInTheDocument();
  });

  it('shows the offline no-op message when cloud is not configured', async () => {
    invokeMock.mockImplementation((cmd) => {
      if (cmd === 'sync_recommendation_feedback') {
        return Promise.resolve({
          offline: true,
          attempted: 0,
          synced: 0,
          failed: 0,
          skipped_no_hash: 0,
        });
      }
      return Promise.resolve(null);
    });

    render(
      <SettingsPanel
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: "Feedback'i cloud'a gönder" }));

    expect(await screen.findByText('Cloud endpoint yok; feedback yerelde bekliyor')).toBeInTheDocument();
  });
});
