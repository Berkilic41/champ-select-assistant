import React from 'react';
import './LoadingSkeleton.css';

interface Props {
  rows?: number;
  height?: number;
}

export const LoadingSkeleton: React.FC<Props> = ({ rows = 5, height = 48 }) => (
  <div className="skeleton-list">
    {Array.from({ length: rows }).map((_, i) => (
      <div key={i} className="skeleton-row" style={{ height }} />
    ))}
  </div>
);
