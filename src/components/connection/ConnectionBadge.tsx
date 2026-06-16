import React from 'react';
import { useTranslation } from 'react-i18next';
import { AppStatus } from '../../types/app';
import './ConnectionBadge.css';

interface Props {
  status: AppStatus;
}

export const ConnectionBadge: React.FC<Props> = ({ status }) => {
  const { t } = useTranslation();

  // Tek persistent live region (role=status) — bağlantı durumu değişimleri ekran
  // okuyucuya duyurulur (eskiden her durum ayrı span'di, değişim duyurulmuyordu).
  const { cls, text } =
    status.kind === 'connecting'
      ? { cls: 'connecting', text: t('connection.badgeConnecting') }
      : status.kind === 'disconnected'
        ? { cls: 'disconnected', text: t('connection.badgeDisconnected') }
        : { cls: 'connected', text: status.summonerName || t('connection.badgeConnected') };

  return (
    <span className={`badge badge--${cls}`} role="status" aria-live="polite">
      {text}
    </span>
  );
};
