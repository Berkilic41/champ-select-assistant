import React from 'react';
import { useTranslation } from 'react-i18next';
import { Settings } from 'lucide-react';
import { AppSettings } from '../hooks/useSettings';
import { AppStatus } from '../types/app';
import { ConnectionBadge } from './connection/ConnectionBadge';
import { ConnectingView } from './connection/ConnectingView';
import { DisconnectedView } from './connection/DisconnectedView';
import { LobbyView } from './lobby/LobbyView';
import { ChampSelectWrapper } from './champ-select/ChampSelectWrapper';
import { IngameView } from './connection/IngameView';
import './AppShell.css';

interface Props {
  status: AppStatus;
  onRetry: () => void;
  onSettingsOpen: () => void;
  addToast: (message: string, type?: 'info' | 'success' | 'warning' | 'error') => void;
  settings: AppSettings;
  /** Briefly true after an LCU hiccup — shows a reconnect banner over the kept view. */
  reconnecting?: boolean;
}

function renderMain(
  status: AppStatus,
  onRetry: () => void,
  addToast: Props['addToast'],
  _inGameLabel: string,
  platformRegion: string,
): React.ReactNode {
  switch (status.kind) {
    case 'connecting':
      return <ConnectingView />;
    case 'disconnected':
      return (
        <DisconnectedView
          error={status.error}
          onRetry={onRetry}
          isRetrying={false}
        />
      );
    case 'lobby':
      return (
        <LobbyView
          summonerName={status.summonerName}
          port={status.port}
          platformRegion={platformRegion}
        />
      );
    case 'champ-select':
      return (
        <ChampSelectWrapper
          summonerName={status.summonerName}
          port={status.port}
          addToast={addToast}
        />
      );
    case 'in-game':
      return <IngameView summonerName={status.summonerName} />;
  }
}

export const AppShell: React.FC<Props> = ({ status, onRetry, onSettingsOpen, addToast, settings, reconnecting }) => {
  const { t } = useTranslation();
  // The window stays at the user's chosen `window_size` throughout — including
  // in-game. We deliberately do NOT shrink to a tiny overlay on game start, so
  // the user can alt-tab back to a full-size window and review their game plan
  // during the match.

  return (
    <div className="app-shell">
      <header className="app-header">
        <span className="app-title">{t('app.title')}</span>
        <div className="app-header-right">
          <ConnectionBadge status={status} />
          <button
            className="appshell-settings-btn"
            onClick={onSettingsOpen}
            title={t('app.settings')}
            aria-label={t('app.settings')}
          >
            <Settings size={18} />
          </button>
        </div>
      </header>
      <main className="app-main">
        {reconnecting && (
          <div className="app-reconnect-banner" role="status">
            {t('connection.reconnecting')}
          </div>
        )}
        {renderMain(status, onRetry, addToast, t('connection.inGame'), settings.platform_region)}
      </main>
    </div>
  );
};
