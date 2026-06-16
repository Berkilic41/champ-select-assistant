import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Trophy } from 'lucide-react';
import { invoke } from '../../lib/host';
import { useActiveSummonerPuuid } from '../../hooks/useActiveSummonerPuuid';
import './GameReviewCard.css';

interface RankedStat {
  queue: string;
  tier: string;
  division: string;
  league_points: number;
  wins: number;
  losses: number;
  is_provisional: boolean;
  updated_at: number;
}

/** Tier'a ölçülü renk (LoL renk kodları). */
const TIER_COLOR: Record<string, string> = {
  IRON: '#7d6657',
  BRONZE: '#a0603a',
  SILVER: '#9aa4ad',
  GOLD: '#e0b94e',
  PLATINUM: '#4fb0a5',
  EMERALD: '#3fae6a',
  DIAMOND: '#5b8edd',
  MASTER: '#b85cd8',
  GRANDMASTER: '#e0574b',
  CHALLENGER: '#e8c95a',
};

/**
 * D2: rank bağlamı — soloQ/flex tier/division/LP + split rekoru.
 * Yalnız GÖRÜNTÜ; öneri/karne motorunu etkilemez (LCU keyless çekilir).
 */
export const RankCard: React.FC = () => {
  const { t } = useTranslation();
  const puuid = useActiveSummonerPuuid();
  const [ranks, setRanks] = useState<RankedStat[]>([]);
  const [error, setError] = useState(false);

  useEffect(() => {
    if (!puuid) return;
    let alive = true;
    (async () => {
      try {
        const res = await invoke<RankedStat[]>('get_ranked_stats', { puuid });
        if (alive) setRanks(res);
      } catch {
        // Honest error state (≠ "unranked"): a failed fetch shows a small badge.
        if (alive) setError(true);
      }
    })();
    return () => {
      alive = false;
    };
  }, [puuid]);

  if (error) {
    return (
      <div className="grc-card">
        <div className="grc-head">
          <Trophy size={14} />
          <span className="grc-title">{t('rank.title')}</span>
        </div>
        <p className="grc-baseline">{t('app.dataError')}</p>
      </div>
    );
  }
  if (ranks.length === 0) return null;
  // soloQ önce.
  const ordered = [...ranks].sort((a) => (a.queue === 'soloq' ? -1 : 1));

  return (
    <div className="grc-card">
      <div className="grc-head">
        <Trophy size={14} />
        <span className="grc-title">{t('rank.title')}</span>
      </div>
      <ul className="grc-lines">
        {ordered.map((r) => {
          const games = r.wins + r.losses;
          const wr = games > 0 ? Math.round((r.wins / games) * 100) : 0;
          const tierLabel = t(`rank.tier.${r.tier}`, { defaultValue: r.tier });
          return (
            <li key={r.queue} className="grc-line">
              <span className="grc-metric">{t(`review.queue.${r.queue}`, { defaultValue: r.queue })}</span>
              <span
                className="grc-value"
                style={{ color: TIER_COLOR[r.tier] ?? 'var(--text-primary)' }}
              >
                {tierLabel}
                {r.division ? ` ${r.division}` : ''}
                {r.is_provisional ? '' : ` · ${r.league_points} LP`}
              </span>
              <span className="grc-baseline">
                {r.is_provisional
                  ? t('rank.provisional')
                  : t('rank.record', { w: r.wins, l: r.losses, wr })}
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
};
