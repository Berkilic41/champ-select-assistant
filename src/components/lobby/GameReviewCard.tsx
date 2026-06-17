import React, { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Target, TrendingUp, TrendingDown, Minus, Lock } from 'lucide-react';
import { invoke } from '../../lib/host';
import type { GameReview } from '../../types/generated/GameReview';
import type { ReviewLine } from '../../types/generated/ReviewLine';
import { ChampionIcon } from '../shared/ChampionIcon';
import './GameReviewCard.css';

interface StoredReview {
  match_id: string;
  queue_group: string;
  created_at: number;
  review: GameReview;
}

interface GenerateResult {
  created: number;
  latest: StoredReview | null;
  streak: number;
}

/**
 * Maç sonu karnesi (Faz C1+C2): en son maçın kişisel-baseline karnesi +
 * önceki hedef kontrolü + sonraki maç hedefi. Kendi verisini çözer
 * (PoolBuilder deseni) — açılışta eksik karneleri üretir, en yenisini gösterir.
 */
const NOTE_TAGS = ['tilt', 'wave', 'vision', 'macro'] as const;

interface MatchNote {
  match_id: string;
  note: string;
  tags: string[];
  updated_at: number;
}

interface FocusHistoryEntry {
  result: string; // 'met' | 'missed'
  label: string;
  metric: string;
}

interface Props {
  /** Belirli bir maçın karnesi (Maç Geçmişi detay paneli — Slice 2). Verilmezse
   *  mevcut "en yeni maç" davranışı (StatsView'da prop'suz kullanım). */
  matchId?: string;
}

export const GameReviewCard: React.FC<Props> = ({ matchId }) => {
  const { t } = useTranslation();
  const [latest, setLatest] = useState<StoredReview | null>(null);
  const [streak, setStreak] = useState(0);
  const [puuid, setPuuid] = useState('');
  const [note, setNote] = useState('');
  const [tags, setTags] = useState<string[]>([]);
  const [noteSaved, setNoteSaved] = useState(false);
  const [noteError, setNoteError] = useState(false);
  const [focusHistory, setFocusHistory] = useState<FocusHistoryEntry[]>([]);
  const noteTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const p = await invoke<string | null>('get_active_summoner_puuid');
        if (!p) return;
        if (alive) setPuuid(p);
        let current: StoredReview | null;
        if (matchId) {
          // Detay paneli: belirli maçın karnesi (yeniden üretme/streak YOK).
          current = await invoke<StoredReview | null>('get_game_review', { matchId });
        } else {
          const gen = await invoke<GenerateResult>('generate_game_reviews', { puuid: p });
          if (!alive) return;
          setStreak(gen.streak);
          current = gen.latest;
          if (!current) {
            const stored = await invoke<StoredReview[]>('get_game_reviews', { puuid: p, limit: 1 });
            current = stored[0] ?? null;
          }
        }
        if (!alive || !current) return;
        setLatest(current);
        // C5: bu maçın mevcut notu (varsa) yüklenir.
        const existing = await invoke<MatchNote | null>('get_match_note', {
          matchId: current.match_id,
        });
        if (alive && existing) {
          setNote(existing.note);
          setTags(existing.tags);
        }
        // Epic #3: hedef-tutturma serisi (gelişim koçluğu görseli).
        const history = await invoke<FocusHistoryEntry[]>('get_focus_history', {
          puuid: p,
          group: current.queue_group,
          limit: 8,
        });
        if (alive) setFocusHistory(history ?? []);
      } catch {
        /* karne üretilemedi (motor/DB yok) — kart sessizce gizli kalır */
      }
    })();
    return () => {
      alive = false;
    };
  }, [matchId]);

  const toggleTag = (tag: string) =>
    setTags((cur) => (cur.includes(tag) ? cur.filter((x) => x !== tag) : [...cur, tag]));

  const saveNote = async () => {
    if (!latest || !puuid) return;
    if (noteTimerRef.current) clearTimeout(noteTimerRef.current);
    try {
      await invoke('set_match_note', { puuid, matchId: latest.match_id, note, tags });
      setNoteError(false);
      setNoteSaved(true);
      noteTimerRef.current = setTimeout(() => setNoteSaved(false), 1500);
    } catch {
      // Surface the failure (the note stays in the box so it isn't lost).
      setNoteSaved(false);
      setNoteError(true);
      noteTimerRef.current = setTimeout(() => setNoteError(false), 2500);
    }
  };

  // Clear any pending status-reset timer on unmount.
  useEffect(() => () => {
    if (noteTimerRef.current) clearTimeout(noteTimerRef.current);
  }, []);

  if (!latest) return null;
  const review = latest.review;

  const verdictIcon = (line: ReviewLine) => {
    switch (line.verdict) {
      case 'better':
        return <TrendingUp size={12} className="grc-up" />;
      case 'worse':
        return <TrendingDown size={12} className="grc-down" />;
      case 'even':
        return <Minus size={12} className="grc-even" />;
      default:
        return <Lock size={12} className="grc-muted" />;
    }
  };

  // locked/no_baseline satırlarından yalnız DEĞERİ olanlar + kilitli timeline
  // satırları gösterilir (dürüst "Riot anahtarıyla açılır" mesajıyla).
  const visibleLines = review.lines.filter(
    (l) => l.value !== undefined && l.value !== null,
  );
  const lockedLines = review.lines.filter((l) => l.verdict === 'locked');

  const check = review.focus_check;

  return (
    <div className="grc-card">
      <div className="grc-head">
        <ChampionIcon championKey={review.champion_key} size="sm" />
        <span className="grc-title">{t('review.title')}</span>
        <span className={`grc-result ${review.win ? 'grc-result--win' : 'grc-result--loss'}`}>
          {review.win ? t('review.win') : t('review.loss')}
        </span>
        <span className="grc-chip">{t(`review.queue.${latest.queue_group}`, { defaultValue: latest.queue_group })}</span>
      </div>

      {check && (
        <div className={`grc-focus-check grc-focus-check--${check.result}`}>
          <Target size={13} />
          <span>
            {t(`review.focusChecked.${check.result}`, {
              label: check.label,
              achieved: check.achieved ?? '—',
            })}
          </span>
          {streak > 1 && <span className="grc-streak">{t('review.streak', { n: streak })}</span>}
        </div>
      )}

      {focusHistory.length > 0 && (
        <div
          className="grc-streak-history"
          role="img"
          aria-label={t('review.focusHistoryAria', {
            met: focusHistory.filter((g) => g.result === 'met').length,
            total: focusHistory.length,
          })}
          title={t('review.focusHistoryTitle')}
        >
          {[...focusHistory].reverse().map((g, i) => (
            <span
              key={i}
              className={`grc-goal-dot grc-goal-dot--${g.result === 'met' ? 'met' : 'missed'}`}
              title={g.label}
              aria-hidden="true"
            />
          ))}
        </div>
      )}

      <ul className="grc-lines">
        {visibleLines.map((l) => (
          <li key={l.metric} className="grc-line">
            {verdictIcon(l)}
            <span className="grc-metric">{t(`review.metric.${l.metric}`, { defaultValue: l.metric })}</span>
            <span className="grc-value">{l.value}</span>
            {l.baseline !== undefined && l.baseline !== null && (
              <span className="grc-baseline">{t('review.baselineShort', { v: l.baseline })}</span>
            )}
          </li>
        ))}
        {lockedLines.length > 0 && (
          <li className="grc-line grc-line--locked">
            <Lock size={12} className="grc-muted" />
            <span className="grc-metric">
              {lockedLines.map((l) => t(`review.metric.${l.metric}`, { defaultValue: l.metric })).join(' · ')}
            </span>
            <span className="grc-locked-note">{t('review.locked')}</span>
          </li>
        )}
      </ul>

      <p className="grc-note grc-note--good">{review.went_right}</p>
      <p className="grc-note grc-note--fix">{review.to_fix}</p>

      {review.next_focus && (
        <div className="grc-next-focus">
          <Target size={13} />
          <span>{t('review.nextFocus', { label: review.next_focus.label })}</span>
        </div>
      )}
      {review.partial && <p className="grc-partial">{t('review.partial')}</p>}

      {/* C5: maç notu — yalnız yerel DB'ye yazılır. */}
      <div className="grc-note-editor">
        <div className="grc-note-tags">
          {NOTE_TAGS.map((tag) => (
            <button
              key={tag}
              type="button"
              className={`grc-tag ${tags.includes(tag) ? 'grc-tag--on' : ''}`}
              onClick={() => toggleTag(tag)}
            >
              {t(`review.noteTag.${tag}`)}
            </button>
          ))}
        </div>
        <div className="grc-note-row">
          <input
            className="grc-note-input"
            value={note}
            onChange={(e) => setNote(e.target.value)}
            placeholder={t('review.notePlaceholder')}
            maxLength={200}
          />
          <button type="button" className="grc-note-save" onClick={saveNote}>
            {noteError
              ? t('review.noteSaveError')
              : noteSaved
                ? t('review.noteSaved')
                : t('review.noteSave')}
          </button>
        </div>
      </div>
    </div>
  );
};
