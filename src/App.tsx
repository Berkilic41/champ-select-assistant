import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AppShell } from './components/AppShell';
import { SettingsPanel } from './components/settings/SettingsPanel';
import { OnboardingWizard } from './components/onboarding/OnboardingWizard';
import { ToastContainer } from './components/shared/Toast';
import { useLcuStatus } from './hooks/useLcuStatus';
import { useChampSelect } from './hooks/useChampSelect';
import { useSettings } from './hooks/useSettings';
import { useToast } from './hooks/useToast';
import { AppStatus } from './types/app';

function App(): React.ReactElement {
  const { status: lcuStatus, retry } = useLcuStatus();
  const { isActive } = useChampSelect();
  const [status, setStatus] = useState<AppStatus>(lcuStatus);
  const { settings, save: saveSettings, loaded } = useSettings();
  const { toasts, addToast, removeToast } = useToast();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [onboardingDone, setOnboardingDone] = useState(true);

  // Check onboarding status on mount
  useEffect(() => {
    invoke<boolean>('is_onboarding_complete')
      .then(done => setOnboardingDone(done))
      .catch(() => setOnboardingDone(true));
  }, []);

  useEffect(() => {
    setStatus(lcuStatus);
  }, [lcuStatus]);

  useEffect(() => {
    if (isActive && lcuStatus.kind === 'lobby') {
      setStatus({
        kind: 'champ-select',
        summonerName: lcuStatus.summonerName,
        port: lcuStatus.port,
      });
    } else if (!isActive && status.kind === 'champ-select') {
      setStatus(lcuStatus);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isActive, lcuStatus]);

  const handleSaveSettings = async (next: typeof settings) => {
    await saveSettings(next);
    addToast('Ayarlar kaydedildi.', 'success');
  };

  if (!onboardingDone) {
    return (
      <OnboardingWizard
        onComplete={() => {
          setOnboardingDone(true);
          addToast('Hoş geldin! Uygulama hazır.', 'success');
        }}
      />
    );
  }

  return (
    <>
      <AppShell
        status={status}
        onRetry={retry}
        onSettingsOpen={() => setSettingsOpen(true)}
        addToast={addToast}
        settings={settings}
      />
      {settingsOpen && loaded && (
        <SettingsPanel
          settings={settings}
          onSave={handleSaveSettings}
          onClose={() => setSettingsOpen(false)}
        />
      )}
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </>
  );
}

export default App;
