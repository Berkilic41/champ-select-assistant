import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '../../lib/host';
import { useActiveSummonerPuuid } from '../../hooks/useActiveSummonerPuuid';
import './GameReviewCard.css';

/** Host get_off_role_performance satırı (elle; matches GROUP BY position). */
interface RolePerformance {
  position: string;
  games: number;
  wins: number;
}
interface OffRoleReport {
  main_role: string;
  main_games: number;
  main_wins: number;
  off_roles: RolePerformance[];
}

const winRate = (wins: number, games: number): number =>
  games > 0 ? Math.round((wins / games) * 100) : 0;

/**
 * Off-rol zayıflık kartı (Post-game Koçluk — Slice 2): oyuncunun ana rolünden
 * (en çok oynadığı) daha düşük WR'li off-rollerini gösterir. Tamamen ölçülen veri
 * (matches.position + win) — uydurma yok. Anlamlı zayıflık yoksa kart gizli.
 */
export const OffRoleCard: React.FC = () => {
  const { t } = useTranslation();
  const puuid = useActiveSummonerPuuid();
  const [data, setData] = useState<OffRoleReport | null>(null);

  useEffect(() => {
    if (!puuid) return;
    let alive = true;
    invoke<OffRoleReport | null>('get_off_role_performance', { puuid })
      .then((r) => {
        if (alive) setData(r ?? null);
      })
      .catch(() => {
        if (alive) setData(null);
      });
    return () => {
      alive = false;
    };
  }, [puuid]);

  if (!data || data.off_roles.length === 0) return null;
  const roleLabel = (r: string): string => t(`overlay.position.${r}`, { defaultValue: r });
  const mainWr = winRate(data.main_wins, data.main_games);

  return (
    <div className="grc-card">
      <div className="grc-head">
        <span className="grc-title">{t('offRole.title')}</span>
        <span className="grc-chip">
          {t('offRole.mainRole', { role: roleLabel(data.main_role), wr: mainWr })}
        </span>
      </div>
      <p className="grc-baseline">{t('offRole.hint')}</p>
      <ul className="off-role__list">
        {data.off_roles.map((r) => (
          <li key={r.position} className="off-role__row">
            <span className="off-role__name">{roleLabel(r.position)}</span>
            <span className="off-role__wr">
              {t('offRole.roleStat', { wr: winRate(r.wins, r.games), n: r.games })}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
};
