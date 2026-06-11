import React from 'react';
import clsx from 'clsx';
import './ui.css';

type CardVariant = 'flat' | 'raised' | 'glass';
type CardAccent = 'gold' | 'teal' | 'danger' | 'info' | 'none';

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: CardVariant;
  /** Left accent bar + subtle tint. */
  accent?: CardAccent;
  children: React.ReactNode;
}

/** Unified card surface — replaces the per-panel `.hero-card`/`.rec-card`/… CSS. */
export const Card: React.FC<CardProps> = ({
  variant = 'flat',
  accent = 'none',
  className,
  children,
  ...rest
}) => (
  <div
    className={clsx(
      'ui-card',
      `ui-card--${variant}`,
      accent !== 'none' && `ui-card--accent-${accent}`,
      className,
    )}
    {...rest}
  >
    {children}
  </div>
);
