import React from 'react';
import './ui.css';

interface EmptyStateProps {
  icon?: React.ReactNode;
  message: string;
}

/** Honest, on-brand placeholder for a panel/tab with no data yet. */
export const EmptyState: React.FC<EmptyStateProps> = ({ icon, message }) => (
  <div className="ui-empty">
    {icon && <span className="ui-empty-icon">{icon}</span>}
    <p className="ui-empty-msg">{message}</p>
  </div>
);
