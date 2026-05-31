import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import { AppSettings } from '../../hooks/useSettings';
import './SettingsPanel.css';

interface Props {
  settings: AppSettings;
  onSave: (s: AppSettings) => void;
  onClose: () => void;
}

interface WeightSliderProps {
  label: string;
  value: number;        // raw relative importance 0..1 (slider position)
  effectivePct: number; // normalized share of the final score (%)
  onChange: (v: number) => void;
}

function WeightSlider({ label, value, effectivePct, onChange }: WeightSliderProps) {
  return (
    <div className="sp-weight-row">
      <span className="sp-weight-label">{label}</span>
      <input
        type="range"
        min={0}
        max={100}
        step={5}
        value={Math.round(value * 100)}
        onChange={e => onChange(parseInt(e.target.value) / 100)}
        className="sp-slider"
      />
      <span className="sp-weight-val">{effectivePct}%</span>
    </div>
  );
}

export const SettingsPanel: React.FC<Props> = ({ settings, onSave, onClose }) => {
  const { t } = useTranslation();
  const [draft, setDraft] = React.useState<AppSettings>(settings);
  const update = (patch: Partial<AppSettings>) => setDraft(d => ({ ...d, ...patch }));

  // Weights are RELATIVE — the engine normalizes by their sum, so they never
  // need to add up to 100%. Show each factor's live effective share instead.
  const weightSum =
    draft.weight_comfort + draft.weight_matchup + draft.weight_team_counter +
    draft.weight_synergy + draft.weight_meta + draft.weight_role_fit;
  const effPct = (v: number) => (weightSum > 0 ? Math.round((v / weightSum) * 100) : 0);

  type SyncState = 'idle' | 'syncing' | 'done' | 'error';
  const [metaSync, setMetaSync] = React.useState<SyncState>('idle');
  const handleSyncMeta = async () => {
    setMetaSync('syncing');
    try {
      await invoke('sync_meraki_rates');
      setMetaSync('done');
    } catch {
      setMetaSync('error');
    }
  };

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-panel" onClick={e => e.stopPropagation()}>
        <div className="settings-panel__header">
          <h2>{t('settings.title')}</h2>
          <button className="settings-panel__close" onClick={onClose} aria-label="Kapat">
            <X size={18} />
          </button>
        </div>

        <section className="sp-section">
          <h3 className="sp-section-title">{t('settings.languageSection')}</h3>
          <div className="sp-lang-row">
            <button
              className={`sp-lang-btn${(draft.language ?? 'tr') === 'tr' ? ' sp-lang-btn--active' : ''}`}
              onClick={() => update({ language: 'tr' })}
              type="button"
            >
              TR
            </button>
            <button
              className={`sp-lang-btn${draft.language === 'en' ? ' sp-lang-btn--active' : ''}`}
              onClick={() => update({ language: 'en' })}
              type="button"
            >
              EN
            </button>
          </div>
        </section>

        <section className="sp-section">
          <h3 className="sp-section-title">{t('settings.serverSection')}</h3>
          <select
            value={draft.platform_region}
            onChange={e => update({ platform_region: e.target.value })}
            className="sp-select"
          >
            <option value="tr1">{t('settings.regions.tr1')}</option>
            <option value="euw1">{t('settings.regions.euw1')}</option>
            <option value="eune1">{t('settings.regions.eune1')}</option>
            <option value="na1">{t('settings.regions.na1')}</option>
            <option value="kr">{t('settings.regions.kr')}</option>
            <option value="jp1">{t('settings.regions.jp1')}</option>
            <option value="br1">{t('settings.regions.br1')}</option>
            <option value="la1">{t('settings.regions.la1')}</option>
            <option value="la2">{t('settings.regions.la2')}</option>
            <option value="oc1">{t('settings.regions.oc1')}</option>
            <option value="ru">{t('settings.regions.ru')}</option>
          </select>
        </section>

        <section className="sp-section">
          <h3 className="sp-section-title">{t('settings.windowSection')}</h3>
          <label className="sp-toggle">
            <input
              type="checkbox"
              checked={draft.always_on_top}
              onChange={e => update({ always_on_top: e.target.checked })}
            />
            {t('settings.alwaysOnTop')}
          </label>
          <label className="sp-toggle">
            <input
              type="checkbox"
              checked={draft.auto_hide_in_game}
              onChange={e => update({ auto_hide_in_game: e.target.checked })}
            />
            {t('settings.autoHideInGame')}
          </label>
          <div className="sp-row">
            <span>{t('settings.size')}</span>
            <select
              value={draft.window_size}
              onChange={e =>
                update({ window_size: e.target.value as AppSettings['window_size'] })
              }
              className="sp-select"
            >
              <option value="compact">{t('settings.sizeCompact')}</option>
              <option value="standard">{t('settings.sizeStandard')}</option>
              <option value="wide">{t('settings.sizeWide')}</option>
            </select>
          </div>
        </section>

        <section className="sp-section">
          <h3 className="sp-section-title">{t('settings.weightsSection')}</h3>
          <p className="sp-hint">{t('settings.weightsHint')}</p>
          <WeightSlider
            label={t('settings.weightComfort')}
            value={draft.weight_comfort}
            effectivePct={effPct(draft.weight_comfort)}
            onChange={v => update({ weight_comfort: v })}
          />
          <WeightSlider
            label={t('settings.weightMatchup')}
            value={draft.weight_matchup}
            effectivePct={effPct(draft.weight_matchup)}
            onChange={v => update({ weight_matchup: v })}
          />
          <WeightSlider
            label={t('settings.weightTeamCounter')}
            value={draft.weight_team_counter}
            effectivePct={effPct(draft.weight_team_counter)}
            onChange={v => update({ weight_team_counter: v })}
          />
          <WeightSlider
            label={t('settings.weightSynergy')}
            value={draft.weight_synergy}
            effectivePct={effPct(draft.weight_synergy)}
            onChange={v => update({ weight_synergy: v })}
          />
          <WeightSlider
            label={t('settings.weightMeta')}
            value={draft.weight_meta}
            effectivePct={effPct(draft.weight_meta)}
            onChange={v => update({ weight_meta: v })}
          />
          <WeightSlider
            label={t('settings.weightRoleFit')}
            value={draft.weight_role_fit}
            effectivePct={effPct(draft.weight_role_fit)}
            onChange={v => update({ weight_role_fit: v })}
          />
        </section>

        <section className="sp-section">
          <h3 className="sp-section-title">{t('settings.metaSection')}</h3>
          <div className="sp-row">
            <button
              className="sp-btn sp-btn--meta"
              onClick={handleSyncMeta}
              disabled={metaSync === 'syncing'}
              type="button"
            >
              {metaSync === 'syncing' ? t('settings.syncMetaSyncing') : t('settings.syncMetaBtn')}
            </button>
            {metaSync === 'done' && (
              <span className="sp-meta-ok">{t('settings.syncMetaDone')}</span>
            )}
            {metaSync === 'error' && (
              <span className="sp-meta-err">{t('settings.syncMetaFail')}</span>
            )}
          </div>
        </section>

        <section className="sp-section">
          <h3 className="sp-section-title">{t('settings.soundSection')}</h3>
          <label className="sp-toggle">
            <input
              type="checkbox"
              checked={draft.sounds_enabled}
              onChange={e => update({ sounds_enabled: e.target.checked })}
            />
            {t('settings.soundsEnabled')}
          </label>
        </section>

        <div className="sp-footer">
          <span className="sp-weight-total">{t('settings.weightsRelativeNote')}</span>
          <button className="sp-btn sp-btn--cancel" onClick={onClose}>
            {t('settings.cancel')}
          </button>
          <button
            className="sp-btn sp-btn--save"
            onClick={() => { onSave(draft); onClose(); }}
          >
            {t('settings.save')}
          </button>
        </div>
      </div>
    </div>
  );
};
