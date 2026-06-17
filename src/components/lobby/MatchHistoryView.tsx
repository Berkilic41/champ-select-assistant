import React, { useEffect, useState } from 'react';
import { invoke } from '../../lib/host';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { useActiveSummonerPuuid } from '../../hooks/useActiveSummonerPuuid';
import type { MatchHistoryEntry } from '../../types/match-history';
import { ChampionIcon } from '../shared/ChampionIcon';
import { GameReviewCard } from './GameReviewCard';
import './MatchHistoryView.css';

const HISTORY_LIMIT = 20;
const DASH = '—';

/** queue_id → koçluk grubu (host queueGroup ile aynı; i18n review.queue.* için). */
function queueGroup(queueId: number): string {
  if (queueId === 420) return 'soloq';
  if (queueId === 440) return 'flex';
  if (queueId === 450) return 'aram';
  return 'normal';
}

/** played_at (Unix SANİYE) → relatif tarih (LobbyView ile aynı time.* anahtarları). */
function relativeTime(playedAtSecs: number, t: TFunction): string {
  const diffMins = Math.floor((Date.now() - playedAtSecs * 1000) / 60_000);
  if (diffMins < 1) return t('time.justNow');
  if (diffMins < 60) return t('time.minsAgo', { n: diffMins });
  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) return t('time.hoursAgo', { n: diffHours });
  return t('time.daysAgo', { n: Math.floor(diffHours / 24) });
}

/** CS/dk = cs / (süre/60); cs null veya süre yoksa null (A3 dürüst "—"). */
function csPerMin(cs: number | null, durationSecs: number): number | null {
  if (cs === null || durationSecs <= 0) return null;
  return cs / (durationSecs / 60);
}

/**
 * Maç Geçmişi (Epic Slice 1): yerel DB'deki son maçlar — şampiyon, rol, sonuç,
 * tarih, KDA, CS/dk, vision + "İncelendi" işareti (karne varsa). Yeni Riot
 * çağrısı/cloud yok. Slice 2'de satıra tıklayınca game-review detay paneli açılır.
 */
export const MatchHistoryView: React.FC = () => {
  const { t } = useTranslation();
  const puuid = useActiveSummonerPuuid();
  const [matches, setMatches] = useState<MatchHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  // Slice 2: seçili maç → detay paneli (GameReviewCard). null = liste.
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    if (!puuid) return;
    let cancelled = false;
    setLoading(true);
    setError(false);
    invoke<MatchHistoryEntry[]>('get_match_history', { puuid, limit: HISTORY_LIMIT })
      .then((rows) => {
        if (cancelled) return;
        setMatches(rows ?? []);
        setLoading(false);
      })
      .catch(() => {
        // Sessiz hata yutma yok (P-07 deseni): fetch reddi → "veri alınamadı".
        if (cancelled) return;
        setMatches([]);
        setError(true);
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [puuid]);

  if (loading || !puuid) {
    return <p className="match-history__empty">{t('matchHistory.loading')}</p>;
  }
  if (error) {
    return <p className="match-history__empty">{t('app.dataError')}</p>;
  }
  if (matches.length === 0) {
    return <p className="match-history__empty">{t('matchHistory.empty')}</p>;
  }

  if (selected) {
    return (
      <div className="match-history__detail">
        <button
          type="button"
          className="match-history__back"
          onClick={() => setSelected(null)}
        >
          ← {t('matchHistory.back')}
        </button>
        <GameReviewCard matchId={selected} />
      </div>
    );
  }

  return (
    <ul className="match-history" aria-label={t('matchHistory.title')}>
      {matches.map((m) => {
        const cspm = csPerMin(m.cs, m.duration_secs);
        const role = (m.position ?? '').toLowerCase();
        const won = m.win === 1;
        const reviewed = m.has_review === 1;
        const open = () => setSelected(m.match_id);
        return (
          <li
            key={m.match_id}
            className={`match-history__row match-history__row--${won ? 'win' : 'loss'}${reviewed ? ' match-history__row--clickable' : ''}`}
            {...(reviewed
              ? {
                  role: 'button',
                  tabIndex: 0,
                  onClick: open,
                  onKeyDown: (e: React.KeyboardEvent) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      open();
                    }
                  },
                  'aria-label': t('matchHistory.openReview', { champion: m.champion_key }),
                }
              : {})}
          >
            <ChampionIcon championKey={m.champion_key} size="md" />
            <div className="match-history__head">
              <span className="match-history__champ">{m.champion_key || `#${m.champion_id}`}</span>
              <span className="match-history__meta">
                {role ? t(`poolBuilder.role_${role}`, { defaultValue: role }) : DASH}
                {' · '}
                {t(`review.queue.${queueGroup(m.queue_id)}`, { defaultValue: queueGroup(m.queue_id) })}
                {' · '}
                {relativeTime(m.played_at, t)}
              </span>
            </div>
            <span className={`match-history__result match-history__result--${won ? 'win' : 'loss'}`}>
              {won ? t('matchHistory.win') : t('matchHistory.loss')}
            </span>
            <div className="match-history__stats">
              <span className="match-history__stat">
                <span className="match-history__stat-label">{t('matchHistory.kda')}</span>
                {m.kills}/{m.deaths}/{m.assists}
              </span>
              <span className="match-history__stat">
                <span className="match-history__stat-label">{t('matchHistory.csPerMin')}</span>
                {cspm === null ? DASH : cspm.toFixed(1)}
              </span>
              <span className="match-history__stat">
                <span className="match-history__stat-label">{t('matchHistory.vision')}</span>
                {m.vision_score === null ? DASH : m.vision_score}
              </span>
            </div>
            {m.has_review === 1 && (
              <span className="match-history__reviewed">{t('matchHistory.reviewed')}</span>
            )}
          </li>
        );
      })}
    </ul>
  );
};
