import React from 'react';
import { useTranslation } from 'react-i18next';
import './ConnectingView.css';

export const ConnectingView: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div className="connecting">
      <span className="connecting__indicator" />
      <p className="connecting__text">{t('connection.connecting')}</p>
    </div>
  );
};
