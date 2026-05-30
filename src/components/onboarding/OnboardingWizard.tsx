import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import './OnboardingWizard.css';

interface Props {
  onComplete: () => void;
}

const STEP_ICONS = ['⚔', '🔑', '🧠', '🎮'];
const STEP_KEYS = ['step1', 'step2', 'step3', 'step4'] as const;

export const OnboardingWizard: React.FC<Props> = ({ onComplete }) => {
  const { t } = useTranslation();
  const [step, setStep] = useState(0);
  const isLast = step === STEP_KEYS.length - 1;

  const handleDone = async () => {
    await invoke('complete_onboarding').catch(() => {});
    onComplete();
  };

  const key = STEP_KEYS[step];

  return (
    <div className="onboarding-overlay">
      <div className="onboarding-card animate-hero-in">
        <div className="onboarding-icon">{STEP_ICONS[step]}</div>
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
          >
            {isLast ? t('onboarding.start') : t('onboarding.next')}
          </button>
        </div>
        <p className="onboarding-disclaimer">{t('onboarding.disclaimer')}</p>
      </div>
    </div>
  );
};
