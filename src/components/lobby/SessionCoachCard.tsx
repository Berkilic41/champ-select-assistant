import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Coffee, ListChecks, X, Target } from 'lucide-react';
import { invoke } from '../../lib/host';
import type { SessionRead } from '../../types/generated/SessionRead';
import './GameReviewCard.css';

interface SessionCoach {
  read: SessionRead;
  fresh_session: boolean;
}

interface StoredReview {
  review: { next_focus?: { label: string } | null };
}

/**
 * Seans koçu (Faz F): oturum başında ısınma kontrol listesi (devreden odak
 * hedefi otomatik madde), 2 ardışık kayıpta nazik not, 3+ kayıpta "ara öner"
 * kartı. Yalnız ÖNERİR — hiçbir şeyi engellemez; X ile kapatılabilir.
 */
export const SessionCoachCard: React.FC = () => {
  const { t } = useTranslation();
  const [coach, setCoach] = useState<SessionCoach | null>(null);
  const [focusLabel, setFocusLabel] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [checked, setChecked] = useState<boolean[]>([false, false, false]);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const puuid = await invoke<string | null>('get_active_summoner_puuid');
        if (!puuid) return;
        const res = await invoke<SessionCoach>('get_session_coach', { puuid });
        if (!alive) return;
        setCoach(res);
        if (res.fresh_session) {
          // Devreden odak hedefi: en son karnenin next_focus'u.
          const stored = await invoke<StoredReview[]>('get_game_reviews', {
            puuid,
            limit: 1,
          });
          if (alive && stored[0]?.review.next_focus) {
            setFocusLabel(stored[0].review.next_focus.label);
          }
        }
      } catch {
        /* seans okunamadı — kart gizli kalır */
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  if (!coach || dismissed) return null;
  const read = coach.read;

  // F1: tilt kartı (caution/break) — seans W/L + ölçülü core notu.
  if (read.verdict !== 'ok') {
    return (
      <div className={`grc-card src-tilt src-tilt--${read.verdict}`}>
        <div className="grc-head">
          <Coffee size={14} />
          <span className="grc-title">{t('session.title')}</span>
          <span className="grc-chip">
            {t('session.record', { w: read.wins, l: read.losses })}
          </span>
          <button
            type="button"
            className="grc-note-save"
            onClick={() => setDismissed(true)}
            aria-label={t('app.close')}
          >
            <X size={12} />
          </button>
        </div>
        {read.note && <p className="grc-note grc-note--fix">{read.note}</p>}
      </div>
    );
  }

  // F2: ısınma checklist'i — yalnız bu oturumda henüz maç yokken.
  if (!coach.fresh_session) return null;
  const items: string[] = [
    focusLabel ? t('session.warmupFocus', { label: focusLabel }) : t('session.warmupPlan'),
    t('session.warmupRole'),
    t('session.warmupFirstGame'),
  ];

  return (
    <div className="grc-card">
      <div className="grc-head">
        <ListChecks size={14} />
        <span className="grc-title">{t('session.warmupTitle')}</span>
        <button
          type="button"
          className="grc-note-save"
          onClick={() => setDismissed(true)}
          aria-label={t('app.close')}
        >
          <X size={12} />
        </button>
      </div>
      <ul className="grc-lines">
        {items.map((item, i) => (
          <li key={i} className="grc-line">
            <label className="src-check">
              <input
                type="checkbox"
                checked={checked[i]}
                onChange={() =>
                  setChecked((cur) => cur.map((c, j) => (j === i ? !c : c)))
                }
              />
              {i === 0 && focusLabel ? <Target size={12} /> : null}
              <span className={checked[i] ? 'src-check--done' : ''}>{item}</span>
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
};
