import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { Copy, Check } from 'lucide-react';
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
 * Draft chat suggestions derived from the ALLY comp's structured deficits.
 * Compliance: copy-to-clipboard ONLY — the user pastes into team chat
 * themselves; this app never writes to the LCU chat endpoints.
 * Gated behind ≥2 known picks so an empty draft doesn't spam "everything
 * is missing".
 */
export function chatSuggestions(ally: CompSummary, t: TFunction): string[] {
  const known =
    ally.tanks + ally.fighters + ally.mages + ally.marksmen + ally.assassins + ally.supports;
  if (known < 2) return [];
  const out: string[] = [];
  if (!ally.has_engage) out.push(t('teamComp.chat.needEngage'));
  if (!ally.has_frontline) out.push(t('teamComp.chat.needFrontline'));
  const total = ally.ap_share + ally.ad_share;
  if (total > 0 && ally.ad_share / total >= 0.75) out.push(t('teamComp.chat.fullAd'));
  if (total > 0 && ally.ap_share / total >= 0.75) out.push(t('teamComp.chat.fullAp'));
  if (ally.marksmen > 0 && !ally.has_peel) out.push(t('teamComp.chat.needPeel'));
  return out.slice(0, 2);
}

const ChatHelper: React.FC<{ ally: CompSummary; t: TFunction }> = ({ ally, t }) => {
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null);
  const lines = chatSuggestions(ally, t);
  if (lines.length === 0) return null;

  const copy = async (line: string, i: number) => {
    try {
      await navigator.clipboard.writeText(line);
      setCopiedIdx(i);
      setTimeout(() => setCopiedIdx((cur) => (cur === i ? null : cur)), 1500);
    } catch {
      /* pano erişimi yoksa sessiz — satır görünür kalır, elle yazılabilir */
    }
  };

  return (
    <div className="team-comp__chat">
      <span className="team-comp__col-title">{t('teamComp.chat.title')}</span>
      {lines.map((line, i) => (
        <div key={i} className="team-comp__chat-row">
          <span className="team-comp__chat-line">{line}</span>
          <button
            type="button"
            className="team-comp__chat-copy"
            onClick={() => copy(line, i)}
            aria-label={t('teamComp.chat.copy')}
            title={t('teamComp.chat.copy')}
          >
            {copiedIdx === i ? <Check size={13} /> : <Copy size={13} />}
          </button>
        </div>
      ))}
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
      <ChatHelper ally={comp.ally} t={t} />
    </div>
  );
};
