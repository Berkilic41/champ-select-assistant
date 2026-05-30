import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { useSummonerData } from '../../hooks/useSummonerData';
import { ChampionGrid } from '../ChampionGrid';
import { StatsView } from './StatsView';
import './LobbyView.css';

interface Props {
  summonerName: string;
  port: number;
  platformRegion: string;
}

type Tab = 'champions' | 'stats';

function parseSummoner(summonerName: string): { gameName: string; tagLine: string } {
  const sep = summonerName.indexOf('#');
  if (sep === -1) return { gameName: summonerName, tagLine: 'EUW' };
  return {
    gameName: summonerName.slice(0, sep),
    tagLine: summonerName.slice(sep + 1),
  };
}

function ErrorBanner({ error }: { error: string }) {
  return (
    <div className="lobby__error-banner">
      {error}
    </div>
  );
}

function relativeTime(ts: number): string {
  const diffMs = Date.now() - ts;
  const diffMins = Math.floor(diffMs / 60_000);
  if (diffMins < 1) return 'az önce';
  if (diffMins < 60) return `${diffMins} dk önce`;
  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) return `${diffHours} sa önce`;
  return `${Math.floor(diffHours / 24)} gün önce`;
}

export const LobbyView: React.FC<Props> = ({ summonerName, platformRegion }) => {
  const { t } = useTranslation();
  const { gameName, tagLine } = parseSummoner(summonerName);
  const { loading, error, stats, masteries, champMap, sync } = useSummonerData(gameName, tagLine, platformRegion);
  const [activeTab, setActiveTab] = useState<Tab>('champions');
  const [patchVersion, setPatchVersion] = useState<string | null>(null);
  const [lastMetaSync, setLastMetaSync] = useState<number | null>(null);

  // Auto-sync on first mount
  useEffect(() => {
    sync();
    // Fetch DDragon patch version
    invoke<string>('get_ddragon_version').then(setPatchVersion).catch(() => {});
    // Read last meta sync timestamp from localStorage
    const stored = localStorage.getItem('lastMetaSync');
    if (stored) setLastMetaSync(parseInt(stored, 10));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const hasData = stats.length > 0 || masteries.length > 0;

  return (
    <div className="lobby">
      <div className="lobby__header">
        <h2 className="lobby__greeting">{t('lobby.greeting', { name: summonerName })}</h2>
      </div>

      <p className="lobby__hint">
        {t('lobby.hint')}
      </p>

      <button
        className="lobby__sync-btn"
        onClick={() => { sync(); localStorage.setItem('lastMetaSync', Date.now().toString()); setLastMetaSync(Date.now()); }}
        disabled={loading}
        type="button"
      >
        {loading ? t('lobby.loading') : hasData ? t('lobby.refreshBtn') : t('lobby.syncBtn')}
      </button>

      {error && <ErrorBanner error={error} />}

      {!loading && !error && !hasData && (
        <p className="lobby__no-data">
          {t('lobby.noData')}
        </p>
      )}

      {(patchVersion || lastMetaSync) && (
        <div className="lobby__data-badges">
          {patchVersion && (
            <span className="lobby__data-badge">Patch {patchVersion}</span>
          )}
          {lastMetaSync && (
            <span className="lobby__data-badge">Meta: {relativeTime(lastMetaSync)}</span>
          )}
        </div>
      )}

      <div className="lobby-tabs" role="tablist">
        <button
          role="tab"
          aria-selected={activeTab === 'champions'}
          className={`lobby-tab${activeTab === 'champions' ? ' lobby-tab--active' : ''}`}
          onClick={() => setActiveTab('champions')}
          type="button"
        >
          {t('lobby.tabChampions')}
        </button>
        <button
          role="tab"
          aria-selected={activeTab === 'stats'}
          className={`lobby-tab${activeTab === 'stats' ? ' lobby-tab--active' : ''}`}
          onClick={() => setActiveTab('stats')}
          type="button"
        >
          {t('lobby.tabStats')}
        </button>
      </div>

      <div className="lobby__grid-wrap">
        {activeTab === 'champions'
          ? <ChampionGrid stats={stats} masteries={masteries} champMap={champMap} isLoading={loading} />
          : <StatsView stats={stats} masteries={masteries} champMap={champMap} />
        }
      </div>
    </div>
  );
};
