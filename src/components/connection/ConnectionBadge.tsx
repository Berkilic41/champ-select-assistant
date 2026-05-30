import React from 'react';
import { AppStatus } from '../../types/app';
import './ConnectionBadge.css';

interface Props {
  status: AppStatus;
}

export const ConnectionBadge: React.FC<Props> = ({ status }) => {
  if (status.kind === 'connecting') {
    return <span className="badge badge--connecting">Bağlanıyor...</span>;
  }

  if (status.kind === 'disconnected') {
    return <span className="badge badge--disconnected">Bağlantı yok</span>;
  }

  return (
    <span className="badge badge--connected">
      {status.summonerName || 'Bağlandı'}
    </span>
  );
};
