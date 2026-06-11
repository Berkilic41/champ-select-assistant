import React from 'react';
import { useTranslation } from 'react-i18next';
import { Unplug, RotateCw } from 'lucide-react';
import './DisconnectedView.css';

interface Props {
  error?: string;
  onRetry: () => void;
  isRetrying: boolean;
}

export const DisconnectedView: React.FC<Props> = ({ error, onRetry, isRetrying }) => {
  const { t } = useTranslation();
  return (
    <div className="disconnected">
      <div className="disconnected__icon"><Unplug size={32} /></div>
      <p className="disconnected__msg">{t('connection.disconnected')}</p>
      {error && <p className="disconnected__error">{error}</p>}
      <p className="disconnected__hint">{t('connection.autoSearching')}</p>
      <button
        className="disconnected__retry"
        onClick={onRetry}
        disabled={isRetrying}
      >
        <RotateCw size={15} className={isRetrying ? 'spin' : undefined} />
        {isRetrying ? t('connection.connecting') : t('connection.retry')}
      </button>
    </div>
  );
};
