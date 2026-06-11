import { useEffect } from 'react';
import { Recommendation } from '../types/recommendation';

export function useKeyboardShortcuts(
  recommendations: Recommendation[],
  isActive: boolean,
  onHover: (championId: number) => void,
  activeIndex: number = 0,
  /** When provided, 1-5 SWITCH the active pick (the glance action); Enter then
   *  applies a hover on it. Without it, 1-5 fall back to applying a hover. */
  onSelect?: (index: number) => void,
) {
  useEffect(() => {
    if (!isActive) return;

    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return;

      const num = parseInt(e.key);
      if (num >= 1 && num <= 5) {
        const idx = num - 1;
        const rec = recommendations[idx];
        if (!rec) return;
        if (onSelect) onSelect(idx);
        else onHover(rec.champion_id);
      } else if (e.key === 'Enter') {
        const rec = recommendations[activeIndex];
        if (rec) onHover(rec.champion_id);
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [recommendations, isActive, onHover, activeIndex, onSelect]);
}
