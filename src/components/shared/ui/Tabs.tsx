import React from 'react';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import './ui.css';

export interface TabItem {
  value: string;
  label: React.ReactNode;
}

interface TabsProps {
  tabs: TabItem[];
  value: string;
  onChange: (value: string) => void;
  ariaLabel?: string;
  /** Shared layout id for the animated underline (unique per Tabs instance group). */
  layoutId?: string;
}

/** Controlled tab strip — keyboard (←/→/Home/End) + ARIA + animated underline. */
export const Tabs: React.FC<TabsProps> = ({
  tabs,
  value,
  onChange,
  ariaLabel,
  layoutId = 'ui-tab-underline',
}) => {
  const idx = Math.max(
    0,
    tabs.findIndex((tab) => tab.value === value),
  );

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (tabs.length === 0) return;
    if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      e.preventDefault();
      const dir = e.key === 'ArrowRight' ? 1 : -1;
      onChange(tabs[(idx + dir + tabs.length) % tabs.length].value);
    } else if (e.key === 'Home') {
      e.preventDefault();
      onChange(tabs[0].value);
    } else if (e.key === 'End') {
      e.preventDefault();
      onChange(tabs[tabs.length - 1].value);
    }
  };

  return (
    <div className="ui-tabs" role="tablist" aria-label={ariaLabel} onKeyDown={onKeyDown}>
      {tabs.map((tab) => {
        const active = tab.value === value;
        return (
          <button
            key={tab.value}
            type="button"
            role="tab"
            aria-selected={active}
            tabIndex={active ? 0 : -1}
            className={clsx('ui-tab', active && 'ui-tab--active')}
            onClick={() => onChange(tab.value)}
          >
            <span className="ui-tab-label">{tab.label}</span>
            {active && <motion.span layoutId={layoutId} className="ui-tab-underline" />}
          </button>
        );
      })}
    </div>
  );
};
