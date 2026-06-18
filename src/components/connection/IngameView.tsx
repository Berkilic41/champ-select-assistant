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

/** Mutlak oyun-saati (mm:ss) — objective doğuş saati için (countdown'un "now"
 *  özel-durumu olmadan; negatif → "0:00"). */
export function gameClock(secs: number): string {
  const v = Math.max(secs, 0);
  const m = Math.floor(v / 60);
  const s = v % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

// Policy-safe: only the player's own game time + public neutral-objective takes from the
// official Live Client Data API. No hidden info (enemy cooldowns / wards / summoners).
export const IngameView: React.FC<Props> = ({ summonerName }) => {
  const { t } = useTranslation();
  const [data, setData] = useState<OverlayMacroState | null>(null);
  const [plan, setPlan] = useState<IngamePlan | null>(null);
  const { settings } = useSettings();
  const prevSecsRef = useRef<Record<string, number>>({});
  // Overlay HUD: kompakt mod yoğun plan metnini gizler, yalnız glanceable görselleri
  // (header/KDA, güç eğrisi, objective'ler, faz) bırakır → sağ-üstte yüzen pencerede
  // gerçek bir HUD. Başlangıç = compact_overlay ayarı; ayar yüklenince senkronlanır,
  // kullanıcı yine maç-içinde toggle'layabilir.
  const [compact, setCompact] = useState(settings.compact_overlay);
  useEffect(() => setCompact(settings.compact_overlay), [settings.compact_overlay]);

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
    <div className={`ingame-view${compact ? ' ingame-view--compact' : ''}`}>
      <div className="ingame-header">
        <span className="ingame-title">{summonerName ?? t('overlay.title')}</span>
        <button
          className="ingame-compact-toggle"
          onClick={() => setCompact((c) => !c)}
          aria-pressed={compact}
          title={compact ? t('overlay.detailed') : t('overlay.compact')}
        >
          {compact ? t('overlay.detailed') : t('overlay.compact')}
        </button>
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
            {/* Kompakt HUD: metin etiketleri (rol / "vs") gizli — ikonlar bağlamı taşır. */}
            {!compact && plan.position && (
              <span className="overlay-plan-pos">
                {t(`overlay.position.${plan.position}`, { defaultValue: plan.position })}
              </span>
            )}
            {!compact && plan.opponent_name && (
              <span className="overlay-plan-vs">
                {t('overlay.plan.vs', { name: plan.opponent_name })}
              </span>
            )}
            {plan.opponent_key && <ChampionIcon championKey={plan.opponent_key} size="sm" />}
            <span className="overlay-plan-kda">
              {t('overlay.plan.level', { n: plan.level })}
              {' · '}
              {plan.kills}/{plan.deaths}/{plan.assists}
              {' · '}
              {plan.cs_per_min != null
                ? t('overlay.plan.csPace', { cs: plan.cs, pace: plan.cs_per_min })
                : t('overlay.plan.cs', { cs: plan.cs })}
            </span>
          </div>
          <div className="overlay-plan-row">
            <span className="overlay-plan-label">{t('overlay.plan.power.title')}</span>
            <PowerCurveBar
              early={plan.power_early}
              mid={plan.power_mid}
              late={plan.power_late}
              currentPhase={macro?.phase}
            />
          </div>
          {/* Kompakt HUD modunda yoğun plan metni gizli; görseller + macro kalır. */}
          {!compact && (
            <>
              <PlanRow label={t('overlay.plan.win')} text={plan.win_condition} />
              <PlanRow label={t('overlay.plan.role')} text={plan.team_role} />
              {plan.damage_profile && (
                <PlanRow label={t('overlay.plan.damage')} text={plan.damage_profile} />
              )}
              {plan.spike_note && <PlanRow label={t('overlay.plan.spike')} text={plan.spike_note} />}
              {plan.spike_window && (
                <PlanRow label={t('overlay.plan.spikeWindow')} text={plan.spike_window} />
              )}
              {plan.lane_note && <PlanRow label={t('overlay.plan.lane')} text={plan.lane_note} />}
              {plan.wave_note && <PlanRow label={t('overlay.plan.wave')} text={plan.wave_note} />}
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
            </>
          )}
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
                <div className="overlay-objective-timing">
                  <span className="overlay-objective-count">{countdown(o.seconds_until)}</span>
                  {o.state !== 'up' && o.seconds_until > 0 && (
                    <span className="overlay-objective-at" title={t('overlay.spawnAtHint')}>
                      @{gameClock(o.next_spawn_secs)}
                    </span>
                  )}
                </div>
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

const POWER_PHASES = ['early', 'mid', 'late'] as const;
type PowerPhase = (typeof POWER_PHASES)[number];

const pct = (v: number): number => Math.round(Math.max(0, Math.min(1, v)) * 100);

const isKnownPhase = (p?: string): p is PowerPhase =>
  p === 'early' || p === 'mid' || p === 'late';

// Glance-edilebilir güç eğrisi: 3 dikey çubuk (erken/orta/geç), her biri
// arketipin 0..1 gücüyle orantılı yükseklikte; zirve faz(lar)ı vurgulanır.
// `currentPhase` verilirse (canlı oyun fazı) o kolon "şu an buradasın" işaretiyle
// vurgulanır → statik referans canlı "neredeyim"e döner. Metinsel spike_note'u
// görsel bir HUD öğesiyle tamamlar. role=img + aria-label ile SR'a tek özet
// olarak duyurulur (a11y: B-29 Timer deseni gibi), çubuklar dekoratif.
export const PowerCurveBar: React.FC<{
  early: number;
  mid: number;
  late: number;
  currentPhase?: string;
}> = ({ early, mid, late, currentPhase }) => {
  const { t } = useTranslation();
  const vals = { early, mid, late };
  const peak = Math.max(early, mid, late);
  const baseAria = t('overlay.plan.power.aria', {
    early: pct(early),
    mid: pct(mid),
    late: pct(late),
  });
  const aria = isKnownPhase(currentPhase)
    ? t('overlay.plan.power.ariaNow', {
        base: baseAria,
        phase: t(`overlay.plan.power.${currentPhase}`),
      })
    : baseAria;
  return (
    <div className="overlay-power" role="img" aria-label={aria}>
      {POWER_PHASES.map((phase) => {
        const isPeak = peak > 0 && vals[phase] >= peak - 1e-6;
        const isCurrent = currentPhase === phase;
        return (
          <div
            key={phase}
            className={`overlay-power-col${isCurrent ? ' overlay-power-col--current' : ''}`}
            aria-hidden="true"
          >
            {isCurrent && <span className="overlay-power-now">▾</span>}
            <div className="overlay-power-track">
              <div
                className={`overlay-power-fill${isPeak ? ' overlay-power-fill--peak' : ''}`}
                style={{ height: `${pct(vals[phase])}%` }}
              />
            </div>
            <span className="overlay-power-label">{t(`overlay.plan.power.${phase}`)}</span>
          </div>
        );
      })}
    </div>
  );
};
