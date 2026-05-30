import React from 'react';
import { useTranslation } from 'react-i18next';
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
      <p className="disconnected__msg">{t('connection.disconnected')}</p>
      {error && <p className="disconnected__error">{error}</p>}
      <button
        className="disconnected__retry"
        onClick={onRetry}
        disabled={isRetrying}
      >
        {isRetrying ? t('connection.connecting') : t('connection.retry')}
      </button>
    </div>
  );
};
