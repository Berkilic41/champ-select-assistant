import React from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import type { CompSummary, TeamCompBoard as TeamCompBoardData } from '../../types/recommendation';
import './TeamCompBoard.css';

interface Props {
  comp?: TeamCompBoardData | null;
}

const TeamColumn: React.FC<{ title: string; side: CompSummary; t: TFunction }> = ({ title, side, t }) => {
  const total = side.ap_share + side.ad_share;
  const apPct = total > 0 ? Math.round((side.ap_share / total) * 100) : 50;
  const roleData: Array<[number, string]> = [
    [side.tanks, t('teamComp.tank')],
    [side.fighters, t('teamComp.fighter')],
    [side.mages, t('teamComp.mage')],
    [side.marksmen, t('teamComp.marksman')],
    [side.assassins, t('teamComp.assassin')],
    [side.supports, t('teamComp.support')],
  ];
  const chips = roleData.filter(([n]) => n > 0).map(([n, label]) => `${label} ×${n}`);

  return (
    <div className="team-comp__col">
      <span className="team-comp__col-title">{title}</span>
      <div className="team-comp__roles">
        {chips.length ? (
          chips.map((c, i) => <span key={i} className="team-comp__chip">{c}</span>)
        ) : (
          <span className="team-comp__muted">—</span>
        )}
      </div>
      {total > 0 && (
        <div className="team-comp__dmg" title={`AP ${apPct}% / AD ${100 - apPct}%`}>
          <div className="team-comp__dmg-ap" style={{ width: `${apPct}%` }} />
          <div className="team-comp__dmg-ad" style={{ width: `${100 - apPct}%` }} />
        </div>
      )}
      <span className="team-comp__summary">{side.summary}</span>
      {side.gaps.length > 0 && (
        <div className="team-comp__gaps">
          {side.gaps.map((g, i) => (
            <span key={i} className="team-comp__gap">{g}</span>
          ))}
        </div>
      )}
    </div>
  );
};

/**
 * Draft board: both teams' composition (role counts, AP/AD split, utility gaps,
 * summary) side by side. Hidden until comp data is available.
 */
export const TeamCompBoard: React.FC<Props> = ({ comp }) => {
  const { t } = useTranslation();
  if (!comp) return null;

  return (
    <div className="team-comp">
      <span className="team-comp__label">{t('teamComp.title')}</span>
      <div className="team-comp__cols">
        <TeamColumn title={t('teamComp.ally')} side={comp.ally} t={t} />
        <TeamColumn title={t('teamComp.enemy')} side={comp.enemy} t={t} />
      </div>
    </div>
  );
};
