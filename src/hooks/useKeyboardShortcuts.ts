import { useEffect } from 'react';
import { Recommendation } from '../types/recommendation';

export function useKeyboardShortcuts(
  recommendations: Recommendation[],
  isActive: boolean,
  onHover: (championId: number) => void,
  activeIndex: number = 0,
) {
  useEffect(() => {
    if (!isActive) return;

    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return;

      const num = parseInt(e.key);
      if (num >= 1 && num <= 5) {
        const rec = recommendations[num - 1];
        if (rec) onHover(rec.champion_id);
      } else if (e.key === 'Enter') {
        const rec = recommendations[activeIndex];
        if (rec) onHover(rec.champion_id);
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [recommendations, isActive, onHover, activeIndex]);
}
