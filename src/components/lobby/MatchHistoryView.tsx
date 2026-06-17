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
  const [fetching, setFetching] = useState(true);
  const [error, setError] = useState(false);
  // Slice 5: "daha fazla yükle" — limit arttıkça host daha çok maç döndürür.
  const [limit, setLimit] = useState(HISTORY_LIMIT);
  const [hasMore, setHasMore] = useState(false);
  // Slice 2: seçili maç → detay paneli (GameReviewCard). null = liste.
  const [selected, setSelected] = useState<string | null>(null);
  // Slice 3: client-side filtreler (çekilen liste üzerinde; yeni fetch yok).
  const [roleFilter, setRoleFilter] = useState('all');
  const [champFilter, setChampFilter] = useState('all');
  const [resultFilter, setResultFilter] = useState('all');

  useEffect(() => {
    if (!puuid) return;
    let cancelled = false;
    setFetching(true);
    setError(false);
    invoke<MatchHistoryEntry[]>('get_match_history', { puuid, limit })
      .then((rows) => {
        if (cancelled) return;
        const list = rows ?? [];
        setMatches(list);
        // Tam sayfa döndüyse muhtemelen daha var → "daha fazla" göster.
        setHasMore(list.length >= limit);
        setFetching(false);
      })
      .catch(() => {
        // Sessiz hata yutma yok (P-07 deseni): fetch reddi → "veri alınamadı".
        if (cancelled) return;
        setMatches([]);
        setError(true);
        setHasMore(false);
        setFetching(false);
      });
    return () => {
      cancelled = true;
    };
  }, [puuid, limit]);

  // Tam-ekran "yükleniyor" yalnız İLK yüklemede; "daha fazla" sırasında liste kalır.
  if ((fetching && matches.length === 0) || !puuid) {
    return <p className="match-history__empty">{t('matchHistory.loading')}</p>;
  }
  if (error && matches.length === 0) {
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

  // Çekilen liste üzerinde mevcut rol/şampiyon seçenekleri (yalnız var olanlar).
  const presentRoles = Array.from(
    new Set(matches.map((m) => (m.position ?? '').toLowerCase()).filter(Boolean)),
  );
  const presentChamps = Array.from(
    new Set(matches.map((m) => m.champion_key).filter(Boolean)),
  ).sort();

  const filtered = matches.filter((m) => {
    const role = (m.position ?? '').toLowerCase();
    if (roleFilter !== 'all' && role !== roleFilter) return false;
    if (champFilter !== 'all' && m.champion_key !== champFilter) return false;
    if (resultFilter === 'win' && m.win !== 1) return false;
    if (resultFilter === 'loss' && m.win === 1) return false;
    return true;
  });

  // Gösterilen (filtrelenmiş) maçların özeti: rekor + WR + toplam KDA. Filtrelerle
  // birleşince "bu şampiyonda/rolde rekorum" sinyali. Saf renderer (yeni fetch yok).
  const wins = filtered.filter((m) => m.win === 1).length;
  const sumK = filtered.reduce((s, m) => s + m.kills, 0);
  const sumD = filtered.reduce((s, m) => s + m.deaths, 0);
  const sumA = filtered.reduce((s, m) => s + m.assists, 0);
  const summary =
    filtered.length > 0
      ? t('matchHistory.summary', {
          wins,
          losses: filtered.length - wins,
          wr: Math.round((wins / filtered.length) * 100),
          kda: ((sumK + sumA) / Math.max(sumD, 1)).toFixed(2),
        })
      : '';

  return (
    <div className="match-history-wrap">
      <div className="match-history__filters" role="group" aria-label={t('matchHistory.filters')}>
        <select
          className="match-history__filter"
          aria-label={t('matchHistory.filterRole')}
          value={roleFilter}
          onChange={(e) => setRoleFilter(e.target.value)}
        >
          <option value="all">{t('matchHistory.filterAll')}</option>
          {presentRoles.map((r) => (
            <option key={r} value={r}>
              {t(`poolBuilder.role_${r}`, { defaultValue: r })}
            </option>
          ))}
        </select>
        <select
          className="match-history__filter"
          aria-label={t('matchHistory.filterChampion')}
          value={champFilter}
          onChange={(e) => setChampFilter(e.target.value)}
        >
          <option value="all">{t('matchHistory.filterAll')}</option>
          {presentChamps.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
        <select
          className="match-history__filter"
          aria-label={t('matchHistory.filterResult')}
          value={resultFilter}
          onChange={(e) => setResultFilter(e.target.value)}
        >
          <option value="all">{t('matchHistory.filterAll')}</option>
          <option value="win">{t('matchHistory.win')}</option>
          <option value="loss">{t('matchHistory.loss')}</option>
        </select>
      </div>

      {filtered.length === 0 ? (
        <p className="match-history__empty">{t('matchHistory.noFilterMatch')}</p>
      ) : (
        <>
        <p className="match-history__summary">{summary}</p>
        <ul className="match-history" aria-label={t('matchHistory.title')}>
          {filtered.map((m) => {
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
        </>
      )}

      {hasMore && (
        <button
          type="button"
          className="match-history__more"
          onClick={() => setLimit((l) => l + HISTORY_LIMIT)}
          disabled={fetching}
        >
          {fetching ? t('matchHistory.loading') : t('matchHistory.loadMore')}
        </button>
      )}
    </div>
  );
};
