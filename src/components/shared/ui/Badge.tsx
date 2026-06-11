import React from 'react';
import clsx from 'clsx';
import './ui.css';

type BadgeTone = 'gold' | 'teal' | 'danger' | 'warning' | 'info' | 'neutral';

interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  tone?: BadgeTone;
  /** Fully rounded chip. */
  pill?: boolean;
  children: React.ReactNode;
}

/** Unified badge/chip — replaces the scattered `.badge`/`.*-chip`/`.*-tag` CSS. */
export const Badge: React.FC<BadgeProps> = ({
  tone = 'neutral',
  pill = false,
  className,
  children,
  ...rest
}) => (
  <span
    className={clsx('ui-badge', `ui-badge--${tone}`, pill && 'ui-badge--pill', className)}
    {...rest}
  >
    {children}
  </span>
);
