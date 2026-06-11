import React, { useState } from 'react';
import { invoke } from '../../lib/host';
import { useTranslation } from 'react-i18next';
import { Swords, ShieldCheck, BrainCircuit, Gamepad2 } from 'lucide-react';
import './OnboardingWizard.css';

interface Props {
  /** Called when the wizard finishes. `syncOk` is true when the best-effort LCU
   *  data pull succeeded (League running), false when it failed (League closed). */
  onComplete: (syncOk: boolean) => void;
}

const STEP_ICONS = [Swords, ShieldCheck, BrainCircuit, Gamepad2];
const STEP_KEYS = ['step1', 'step2', 'step3', 'step4'] as const;

export const OnboardingWizard: React.FC<Props> = ({ onComplete }) => {
  const { t } = useTranslation();
  const [step, setStep] = useState(0);
  const [preparing, setPreparing] = useState(false);
  const isLast = step === STEP_KEYS.length - 1;

  const handleDone = async () => {
    await invoke('complete_onboarding').catch(() => {});
    // Best-effort: pull mastery + match history from the running LCU so a brand-new
    // user lands with personalized data instead of an empty champ-select (Faz C).
    // Fails fast (and silently) if League isn't running — lobby still offers sync.
    setPreparing(true);
    const syncOk = await invoke('sync_lcu_player_data', {})
      .then(() => true)
      .catch(() => false);
    setPreparing(false);
    onComplete(syncOk);
  };

  const key = STEP_KEYS[step];
  const StepIcon = STEP_ICONS[step];

  return (
    <div className="onboarding-overlay">
      <div className="onboarding-card animate-hero-in">
        <div className="onboarding-icon"><StepIcon size={28} strokeWidth={2} /></div>
        <h2 className="onboarding-title">{t(`onboarding.${key}.title`)}</h2>
        <p className="onboarding-body">{t(`onboarding.${key}.body`)}</p>
        <p className="onboarding-hint">{t(`onboarding.${key}.hint`)}</p>
        <div className="onboarding-dots">
          {STEP_KEYS.map((_, i) => (
            <span
              key={i}
              className={`onboarding-dot${i === step ? ' onboarding-dot--active' : ''}`}
            />
          ))}
        </div>
        <div className="onboarding-footer">
          {step > 0 && (
            <button
              className="onboarding-btn onboarding-btn--back"
              onClick={() => setStep(prev => prev - 1)}
            >
              {t('onboarding.back')}
            </button>
          )}
          <button
            className="onboarding-btn onboarding-btn--next"
            onClick={isLast ? handleDone : () => setStep(prev => prev + 1)}
            disabled={preparing}
          >
            {preparing
              ? t('onboarding.preparing')
              : isLast
                ? t('onboarding.start')
                : t('onboarding.next')}
          </button>
        </div>
        <p className="onboarding-disclaimer">{t('onboarding.disclaimer')}</p>
      </div>
    </div>
  );
};
