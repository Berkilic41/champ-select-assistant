import React, { useEffect, useState } from 'react';
import { invoke } from '../../lib/host';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import type { ChampionDetail } from '../../types/recommendation';
import { ChampionIcon } from '../shared/ChampionIcon';
import { archetypeLabel } from '../../lib/archetype';
import './ChampionDetailCard.css';

interface Props {
  championId: number | null;
  championKey?: string;
  onClose: () => void;
}

/** Win-condition key → short label (LoL terms, used as-is in TR/EN). */
const WIN_LABELS: Record<string, string> = {
  teamfight: 'Teamfight',
  split_push: 'Split push',
  pick: 'Pick',
  poke: 'Poke',
  siege: 'Siege',
  protect: 'Protect',
  skirmish: 'Skirmish',
  duel: 'Duel',
};

function pct(v: number): number {
  return Math.round(Math.max(0, Math.min(1, v)) * 100);
}

const PhaseBar: React.FC<{ label: string; value: number }> = ({ label, value }) => (
  <div className="cdc-phase">
    <span className="cdc-phase__label">{label}</span>
    <div className="cdc-phase__track">
      <div className="cdc-phase__fill" style={{ width: `${pct(value)}%` }} />
    </div>
    <span className="cdc-phase__pct">{pct(value)}%</span>
  </div>
);

/** D3: "asla önerme" / "öğreniyorum" toggle'ları — yalnız yerel tercih. */
const PreferenceToggles: React.FC<{ championId: number }> = ({ championId }) => {
  const { t } = useTranslation();
  const [puuid, setPuuid] = useState('');
  const [pref, setPref] = useState<'never' | 'learning' | null>(null);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const p = await invoke<string | null>('get_active_summoner_puuid');
        if (!p || !alive) return;
        setPuuid(p);
        const prefs = await invoke<{ never: number[]; learning: number[] }>(
          'get_champion_preferences',
          { puuid: p },
        );
        if (!alive) return;
        if (prefs.never.includes(championId)) setPref('never');
        else if (prefs.learning.includes(championId)) setPref('learning');
        else setPref(null);
      } catch {
        /* tercih okunamadı — toggle'lar nötr kalır */
      }
    })();
    return () => {
      alive = false;
    };
  }, [championId]);

  const toggle = async (next: 'never' | 'learning') => {
    if (!puuid) return;
    const value = pref === next ? null : next;
    try {
      await invoke('set_champion_preference', { puuid, championId, preference: value });
      setPref(value);
    } catch {
      /* kaydedilemedi — durum değişmez */
    }
  };

  return (
    <div className="cdc-prefs">
      <button
        type="button"
        className={`cdc-pref ${pref === 'never' ? 'cdc-pref--never' : ''}`}
        onClick={() => toggle('never')}
      >
        {t('prefs.never')}
      </button>
      <button
        type="button"
        className={`cdc-pref ${pref === 'learning' ? 'cdc-pref--learning' : ''}`}
        onClick={() => toggle('learning')}
      >
        {t('prefs.learning')}
      </button>
    </div>
  );
};

/**
 * Lobby champion detail: full KB profile (archetype, power curve, win condition,
 * damage split, CC/mobility/difficulty, combos). Fetches on open; hidden when
 * `championId` is null.
 */
export const ChampionDetailCard: React.FC<Props> = ({ championId, championKey, onClose }) => {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<ChampionDetail | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (championId === null) {
      setDetail(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    invoke<ChampionDetail | null>('get_champion_detail', { championId })
      .then((d) => {
        if (!cancelled) setDetail(d ?? null);
      })
      .catch(() => {
        if (!cancelled) setDetail(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [championId]);

  if (championId === null) return null;

  const total = detail ? detail.damage_ad + detail.damage_ap : 0;
  const apPct = total > 0 ? Math.round((detail!.damage_ap / total) * 100) : 50;

  return (
    <div className="cdc-overlay" onClick={onClose}>
      <div
        className="cdc-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <div className="cdc-header">
          <ChampionIcon championKey={championKey ?? detail?.champion_key ?? ''} size="md" />
          <h2 className="cdc-name">{championKey ?? detail?.champion_key ?? ''}</h2>
          <button className="cdc-close" onClick={onClose} type="button" aria-label={t('app.close')}>
            <X size={18} />
          </button>
        </div>

        {loading && !detail && <p className="cdc-muted">…</p>}
        {!loading && !detail && <p className="cdc-muted">{t('champDetail.noData')}</p>}

        {detail && (
          <>
            <div className="cdc-badges">
              <span className="cdc-badge cdc-badge--archetype">{archetypeLabel(detail.archetype)}</span>
              <span className="cdc-badge">{WIN_LABELS[detail.win_condition] ?? detail.win_condition}</span>
              <span className="cdc-badge">{t('champDetail.difficulty')}: {detail.execution_difficulty}/5</span>
              <span className="cdc-badge">{detail.has_hard_cc ? t('champDetail.hasCc') : t('champDetail.noCc')}</span>
            </div>

            <PreferenceToggles championId={championId} />

            <div className="cdc-section">
              <span className="cdc-label">{t('champDetail.powerCurve')}</span>
              <PhaseBar label={t('champDetail.early')} value={detail.power_early} />
              <PhaseBar label={t('champDetail.mid')} value={detail.power_mid} />
              <PhaseBar label={t('champDetail.late')} value={detail.power_late} />
            </div>

            {total > 0 && (
              <div className="cdc-section">
                <span className="cdc-label">{t('draftPlan.damageProfile')}</span>
                <div className="cdc-dmg" title={`AP ${apPct}% / AD ${100 - apPct}%`}>
                  <div className="cdc-dmg-ap" style={{ width: `${apPct}%` }} />
                  <div className="cdc-dmg-ad" style={{ width: `${100 - apPct}%` }} />
                </div>
              </div>
            )}

            {detail.combos.length > 0 && (
              <div className="cdc-section">
                <span className="cdc-label">{t('comboBoard.title')}</span>
                <ul className="cdc-combos">
                  {detail.combos.slice(0, 8).map((c, i) => (
                    <li key={i} className="cdc-combo">
                      <ChampionIcon championKey={c.partner_key} size="sm" />
                      <div className="cdc-combo__body">
                        <span className="cdc-combo__name">
                          {c.partner_key} · {c.name}
                          <span className="cdc-combo__type"> · {t(`comboBoard.type_${c.combo_type}`)}</span>
                        </span>
                        <span className="cdc-combo__text">{c.combo_text}</span>
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
};
