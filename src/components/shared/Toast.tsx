import React from 'react';
import { useTranslation } from 'react-i18next';
import { CheckCircle2, AlertTriangle, XCircle, Info, X } from 'lucide-react';
import { Toast as ToastType } from '../../hooks/useToast';
import './Toast.css';

interface Props {
  toasts: ToastType[];
  onRemove: (id: string) => void;
}

const ICONS: Record<ToastType['type'], React.ComponentType<{ size?: number }>> = {
  info: Info,
  success: CheckCircle2,
  warning: AlertTriangle,
  error: XCircle,
};

export const ToastContainer: React.FC<Props> = ({ toasts, onRemove }) => {
  const { t: translate } = useTranslation();
  return (
  <div className="toast-container">
    {toasts.map(t => {
      const Icon = ICONS[t.type];
      return (
        <div key={t.id} className={`toast toast--${t.type} animate-slide-up`}>
          <span className="toast__icon"><Icon size={16} /></span>
          <span className="toast__msg">{t.message}</span>
          <button className="toast__close" onClick={() => onRemove(t.id)} aria-label={translate('app.close')}>
            <X size={14} />
          </button>
        </div>
      );
    })}
  </div>
  );
};
