import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '../../lib/host';
import { useTranslation } from 'react-i18next';
import { Minus } from 'lucide-react';
import type { OverlayMacroState } from '../../types/generated/OverlayMacroState';
import type { ObjectiveTimer } from '../../types/generated/ObjectiveTimer';
import type { IngamePlan } from '../../types/generated/IngamePlan';
import { ChampionIcon } from '../shared/ChampionIcon';
import { useSettings } from '../../hooks/useSettings';
import './IngameView.css';

// Sesli makro uyarısı (B5) — asset'siz WebAudio bip; yalnız ÇIKTI üretir
// (oyuna/LCU'ya hiçbir şey yazmaz). sounds_enabled ayarı kapalıysa hiç çalmaz.
let audioCtx: AudioContext | null = null;
function beep(freq: number, ms = 180): void {
  try {
    audioCtx ??= new AudioContext();
    const osc = audioCtx.createOscillator();
    const gain = audioCtx.createGain();
    osc.type = 'sine';
    osc.frequency.value = freq;
    gain.gain.value = 0.08;
    osc.connect(gain);
    gain.connect(audioCtx.destination);
    osc.start();
    osc.stop(audioCtx.currentTime + ms / 1000);
  } catch {
    /* ses aygıtı yoksa sessiz devam */
  }
}

interface Props {
  summonerName?: string;
}

const POLL_MS = 1500;

function stateKind(state: string): string {
  if (state === 'up') return 'up';
  if (state === 'soon') return 'soon';
  return 'pending';
}

// Policy-safe: only the player's own game time + public neutral-objective takes from the
// official Live Client Data API. No hidden info (enemy cooldowns / wards / summoners).
export const IngameView: React.FC<Props> = ({ summonerName }) => {
  const { t } = useTranslation();
  const [data, setData] = useState<OverlayMacroState | null>(null);
  const [plan, setPlan] = useState<IngamePlan | null>(null);
  const { settings } = useSettings();
  const prevSecsRef = useRef<Record<string, number>>({});

  // 60sn ve 30sn eşik GEÇİŞLERİNDE tek bip (60: alçak, 30: yüksek ton).
  // Eşik geçişi = önceki poll > eşik && şimdiki ≤ eşik — spam yok.
  useEffect(() => {
    const macroState = data?.live ? data.state : null;
    if (!macroState) {
      prevSecsRef.current = {};
      return;
    }
    for (const o of macroState.objectives) {
      const prev = prevSecsRef.current[o.objective];
      if (settings.sounds_enabled && prev !== undefined && o.seconds_until > 0) {
        if (prev > 60 && o.seconds_until <= 60) beep(660);
        else if (prev > 30 && o.seconds_until <= 30) beep(880);
      }
      prevSecsRef.current[o.objective] = o.seconds_until;
    }
  }, [data, settings.sounds_enabled]);

  useEffect(() => {
    let alive = true;
    const tick = async () => {
      try {
        const next = await invoke<OverlayMacroState>('get_macro_state');
        if (alive) setData(next);
      } catch {
        if (alive) setData({ live: false, state: null });
      }
    };
    tick();
    const id = setInterval(tick, POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  // The game plan doesn't change mid-match, so fetch it once (retrying slowly until a
  // game is live) and stop polling as soon as we have it.
  useEffect(() => {
    let alive = true;
    let id: ReturnType<typeof setInterval> | undefined;
    const fetchPlan = async () => {
      try {
        const p = await invoke<IngamePlan | null>('get_ingame_plan');
        if (alive && p) {
          setPlan(p);
          if (id) clearInterval(id);
        }
      } catch {
        /* no live game yet — keep retrying quietly */
      }
    };
    fetchPlan();
    id = setInterval(fetchPlan, 5000);
    return () => {
      alive = false;
      if (id) clearInterval(id);
    };
  }, []);

  const countdown = (secs: number): string => {
    if (secs <= 0) return t('overlay.now');
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  const macro = data?.live ? data.state : null;

  return (
    <div className="ingame-view">
      <div className="ingame-header">
        <span className="ingame-title">{summonerName ?? t('overlay.title')}</span>
        <button
          className="ingame-minimize"
          onClick={() => invoke('hide_window')}
          title={t('connection.hide')}
          aria-label={t('connection.hide')}
        >
          <Minus size={16} />
        </button>
      </div>

      {plan && (
        <div className="overlay-plan">
          <div className="overlay-plan-head">
            <ChampionIcon championKey={plan.champion_key} size="sm" />
            <span className="overlay-plan-champ">{plan.champion_name}</span>
            {plan.position && (
              <span className="overlay-plan-pos">
                {t(`overlay.position.${plan.position}`, { defaultValue: plan.position })}
              </span>
            )}
            {plan.opponent_name && (
              <span className="overlay-plan-vs">
                {t('overlay.plan.vs', { name: plan.opponent_name })}
              </span>
            )}
            {plan.opponent_key && <ChampionIcon championKey={plan.opponent_key} size="sm" />}
            <span className="overlay-plan-kda">
              {plan.kills}/{plan.deaths}/{plan.assists}
              {' · '}
              {plan.cs_per_min != null
                ? t('overlay.plan.csPace', { cs: plan.cs, pace: plan.cs_per_min })
                : t('overlay.plan.cs', { cs: plan.cs })}
            </span>
          </div>
          <PlanRow label={t('overlay.plan.win')} text={plan.win_condition} />
          <PlanRow label={t('overlay.plan.role')} text={plan.team_role} />
          {plan.spike_note && <PlanRow label={t('overlay.plan.spike')} text={plan.spike_note} />}
          {plan.spike_window && (
            <PlanRow label={t('overlay.plan.spikeWindow')} text={plan.spike_window} />
          )}
          {plan.lane_note && <PlanRow label={t('overlay.plan.lane')} text={plan.lane_note} />}
          {plan.matchup_tips.length > 0 && (
            <div className="overlay-plan-row">
              <span className="overlay-plan-label">{t('overlay.plan.matchup')}</span>
              <ul className="overlay-plan-tips">
                {plan.matchup_tips.map((tip, i) => (
                  <li key={i}>{tip}</li>
                ))}
              </ul>
            </div>
          )}
          {plan.mid_plan && <PlanRow label={t('overlay.plan.mid')} text={plan.mid_plan} />}
          {plan.late_plan && <PlanRow label={t('overlay.plan.late')} text={plan.late_plan} />}
        </div>
      )}

      {!macro ? (
        <div className="ingame-tip">
          <p className="ingame-tip-sub">{t('overlay.waiting')}</p>
        </div>
      ) : (
        <div className="overlay-macro">
          <div className="overlay-phase">
            <span className={`overlay-phase-chip overlay-phase-chip--${macro.phase}`}>
              {t(`overlay.phase.${macro.phase}`, { defaultValue: macro.phase })}
            </span>
            <span className="overlay-phase-note">{macro.phase_note}</span>
          </div>

          <ul className="overlay-objectives">
            {macro.objectives.slice(0, 4).map((o: ObjectiveTimer) => (
              <li key={o.objective} className="overlay-objective">
                <span className="overlay-objective-name">
                  {t(`overlay.objective.${o.objective}`, { defaultValue: o.objective })}
                </span>
                <span className={`overlay-objective-state overlay-objective-state--${stateKind(o.state)}`}>
                  {t(`overlay.state.${o.state}`, { defaultValue: o.state })}
                </span>
                <span className="overlay-objective-count">{countdown(o.seconds_until)}</span>
              </li>
            ))}
          </ul>

          {macro.reminders.length > 0 && (
            <ul className="overlay-reminders">
              {macro.reminders.slice(0, 2).map((r: string, i: number) => (
                <li key={i} className="overlay-reminder">
                  {r}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
};

const PlanRow: React.FC<{ label: string; text: string }> = ({ label, text }) => (
  <div className="overlay-plan-row">
    <span className="overlay-plan-label">{label}</span>
    <span className="overlay-plan-text">{text}</span>
  </div>
);
