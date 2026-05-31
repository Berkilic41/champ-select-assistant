import React from 'react';
import { useTranslation } from 'react-i18next';
import { AppStatus } from '../../types/app';
import './ConnectionBadge.css';

interface Props {
  status: AppStatus;
}

export const ConnectionBadge: React.FC<Props> = ({ status }) => {
  const { t } = useTranslation();

  if (status.kind === 'connecting') {
    return <span className="badge badge--connecting">{t('connection.badgeConnecting')}</span>;
  }

  if (status.kind === 'disconnected') {
    return <span className="badge badge--disconnected">{t('connection.badgeDisconnected')}</span>;
  }

  return (
    <span className="badge badge--connected">
      {status.summonerName || t('connection.badgeConnected')}
    </span>
  );
};
