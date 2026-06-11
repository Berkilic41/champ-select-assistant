import React from 'react';
import './LoadingSkeleton.css';

interface Props {
  rows?: number;
  height?: number;
  /** `hero` shimmers a decision-card silhouette (avatar + lines + ring + bars). */
  variant?: 'rows' | 'hero';
}

export const LoadingSkeleton: React.FC<Props> = ({ rows = 5, height = 48, variant = 'rows' }) => {
  if (variant === 'hero') {
    return (
      <div className="skeleton-hero" aria-hidden="true">
        <div className="skeleton-hero__head">
          <div className="skeleton-row skeleton-hero__avatar" />
          <div className="skeleton-hero__lines">
            <div className="skeleton-row skeleton-hero__line skeleton-hero__line--lg" />
            <div className="skeleton-row skeleton-hero__line" />
          </div>
          <div className="skeleton-row skeleton-hero__ring" />
        </div>
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="skeleton-row skeleton-hero__bar" />
        ))}
      </div>
    );
  }
  return (
    <div className="skeleton-list">
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="skeleton-row" style={{ height }} />
      ))}
    </div>
  );
};
