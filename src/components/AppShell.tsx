import React, { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
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

export const AppShell: React.FC<Props> = ({ status, onRetry, onSettingsOpen, addToast, settings }) => {
  const { t } = useTranslation();
  // Stores the user's window_size preference before switching to the in-game
  // overlay so it can be restored when the game ends.
  const savedWindowSize = useRef<'compact' | 'standard' | 'wide'>(settings.window_size);

  useEffect(() => {
    savedWindowSize.current = settings.window_size;
  }, [settings.window_size]);

  useEffect(() => {
    const unlisten = listen<string>('gameflow-phase', (e) => {
      if (e.payload === 'InProgress') {
        // Snapshot the user's current preference before switching to overlay
        savedWindowSize.current = settings.window_size;
        invoke('set_window_size', { preset: 'overlay' }).catch(() => {});
      } else if (e.payload === 'Lobby' || e.payload === 'EndOfGame') {
        // Restore the user's preference instead of forcing 'standard'
        invoke('set_window_size', { preset: savedWindowSize.current }).catch(() => {});
      }
    });
    return () => { unlisten.then(fn => fn()); };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
          >
            ⚙
          </button>
        </div>
      </header>
      <main className="app-main">
        {renderMain(status, onRetry, addToast, t('connection.inGame'), settings.platform_region)}
      </main>
    </div>
  );
};
