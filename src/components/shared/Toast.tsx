import React from 'react';
import { Toast as ToastType } from '../../hooks/useToast';
import './Toast.css';

interface Props {
  toasts: ToastType[];
  onRemove: (id: string) => void;
}

const ICONS: Record<ToastType['type'], string> = {
  info: 'i',
  success: '✓',
  warning: '⚠',
  error: '✗',
};

export const ToastContainer: React.FC<Props> = ({ toasts, onRemove }) => (
  <div className="toast-container">
    {toasts.map(t => (
      <div key={t.id} className={`toast toast--${t.type} animate-slide-up`}>
        <span className="toast__icon">{ICONS[t.type]}</span>
        <span className="toast__msg">{t.message}</span>
        <button className="toast__close" onClick={() => onRemove(t.id)}>
          &times;
        </button>
      </div>
    ))}
  </div>
);
