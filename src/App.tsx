import React, { useEffect, useRef, useState } from 'react';
import { invoke } from './lib/host';
import { listen } from './lib/host';
import { applyDdragonVersion } from './lib/ddragon';
import { useTranslation } from 'react-i18next';
import i18n from './i18n';
import { AppShell } from './components/AppShell';
import { SettingsPanel } from './components/settings/SettingsPanel';
import { OnboardingWizard } from './components/onboarding/OnboardingWizard';
import { ToastContainer } from './components/shared/Toast';
import { useLcuStatus } from './hooks/useLcuStatus';
import { useChampSelect } from './hooks/useChampSelect';
import { useSettings } from './hooks/useSettings';
import { useToast } from './hooks/useToast';
import { AppStatus } from './types/app';

// LCU gameflow phases that mean "a match is live" → show the in-game overlay.
const IN_GAME_PHASES = ['GameStart', 'InProgress', 'Reconnect'];

function App(): React.ReactElement {
  const { t } = useTranslation();
  const { status: lcuStatus, retry } = useLcuStatus();
  const { isActive } = useChampSelect();
  const [status, setStatus] = useState<AppStatus>(lcuStatus);
  const { settings, save: saveSettings, loaded } = useSettings();
  const { toasts, addToast, removeToast } = useToast();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [onboardingDone, setOnboardingDone] = useState(true);
  // True while we briefly hold the champ-select view after an LCU hiccup, so a
  // momentary disconnect doesn't wipe the live coaching mid-draft (Faz B).
  const [reconnecting, setReconnecting] = useState(false);
  const [gamePhase, setGamePhase] = useState<string>('');
  const lcuErrorAtRef = useRef(0);

  // Check onboarding status on mount
  useEffect(() => {
    invoke<boolean>('is_onboarding_complete')
      .then(done => setOnboardingDone(done))
      .catch(() => setOnboardingDone(true));
  }, []);

  // Renderer ikon URL'leri için canlı DDragon patch'ini mount'ta çöz — yalnız
  // LobbyView sync'iyle değil. Onboarding'den ya da doğrudan champ-select'e açılan
  // yolda da renderer canlı patch'i alsın (yoksa 14.10.1 fallback'iyle o patch'ten
  // sonra eklenen şampiyon/item ikonları 404 → baş-harf yedeği).
  useEffect(() => {
    invoke<string>('get_ddragon_version').then(applyDdragonVersion).catch(() => {});
  }, []);

  useEffect(() => {
    setStatus(lcuStatus);
  }, [lcuStatus]);

  // Apply the saved UI language to i18n once settings load / change.
  useEffect(() => {
    const lang = settings.language ?? 'tr';
    if (loaded && i18n.language !== lang) {
      i18n.changeLanguage(lang);
    }
  }, [settings.language, loaded]);

  // Remember when the backend last reported an LCU communication error, to tell a
  // transient hiccup apart from a clean champ-select end.
  useEffect(() => {
    const un = listen('lcu-error', () => {
      lcuErrorAtRef.current = Date.now();
    });
    return () => {
      un.then((fn) => fn());
    };
  }, []);

  // Track the LCU gameflow phase so we can switch to the in-game overlay when a match
  // starts and back to the lobby when it ends. The backend's gameflow watcher emits
  // this on every phase change.
  useEffect(() => {
    const un = listen<string>('gameflow-phase', (e) => setGamePhase(e.payload ?? ''));
    return () => {
      un.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    // 1) A live match takes precedence over everything → show the in-game overlay
    //    (game plan + macro timers). This is what makes alt-tab review work.
    if (IN_GAME_PHASES.includes(gamePhase)) {
      if (status.kind !== 'in-game') {
        const summonerName =
          status.kind === 'champ-select'
            ? status.summonerName
            : lcuStatus.kind === 'lobby'
              ? lcuStatus.summonerName
              : 'Summoner';
        setReconnecting(false);
        setStatus({ kind: 'in-game', summonerName });
      }
      return;
    }

    // 2) Champ-select is active → live draft coaching. Gate on the gameflow phase
    //    so a stuck `isActive` (a missed `champ-select-session: null`) can't pin
    //    us to the draft after it ends. Empty phase = host hasn't reported one
    //    yet → trust isActive (back-compat with the seed-on-connect timing).
    const inChampSelectPhase = gamePhase === '' || gamePhase === 'ChampSelect';
    if (isActive && lcuStatus.kind === 'lobby' && inChampSelectPhase) {
      setStatus({
        kind: 'champ-select',
        summonerName: lcuStatus.summonerName,
        port: lcuStatus.port,
      });
      setReconnecting(false);
      return;
    }

    // 2b) Fail-safe for the "stuck on the draft screen" bug: we're still showing
    //     champ-select but the gameflow phase has clearly left ChampSelect (dodge
    //     or draft finished without a clean null event) → drop back to the lobby.
    if (
      status.kind === 'champ-select' &&
      gamePhase !== '' &&
      gamePhase !== 'ChampSelect'
    ) {
      setReconnecting(false);
      setStatus(lcuStatus);
      return;
    }

    // 3) Leaving champ-select: a momentary LCU drop holds the coaching for a grace
    //    window so a hiccup doesn't wipe the draft mid-pick.
    if (!isActive && status.kind === 'champ-select') {
      const transientDrop = Date.now() - lcuErrorAtRef.current < 2000;
      if (transientDrop) {
        setReconnecting(true);
        const id = setTimeout(() => {
          setReconnecting(false);
          setStatus(lcuStatus);
        }, 8000);
        return () => clearTimeout(id);
      }
      setStatus(lcuStatus); // clean session end (dodge / finished) → leave at once
      return;
    }

    // 4) The match just ended (we were in-game) → drop back to the lobby/LCU status.
    if (status.kind === 'in-game') {
      setStatus(lcuStatus);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isActive, lcuStatus, gamePhase]);

  // In-game window behaviour, fired once per transition:
  //   • auto_hide_in_game ON  → hide the window entirely (zero distraction).
  //   • auto_hide_in_game OFF → compact overlay pinned top-right, always-on-top, so
  //     the detailed game plan floats over a BORDERLESS game (no alt-tab needed).
  // On match end we restore the user's window size + center.
  const inGameWindowRef = useRef<'none' | 'hidden' | 'overlay'>('none');
  useEffect(() => {
    const inGame = IN_GAME_PHASES.includes(gamePhase);
    if (inGame && inGameWindowRef.current === 'none') {
      if (settings.auto_hide_in_game) {
        inGameWindowRef.current = 'hidden';
        invoke('hide_window').catch(() => {});
      } else {
        inGameWindowRef.current = 'overlay';
        invoke('set_overlay_mode', { enabled: true }).catch(() => {});
      }
    } else if (!inGame && inGameWindowRef.current !== 'none') {
      const was = inGameWindowRef.current;
      inGameWindowRef.current = 'none';
      if (was === 'hidden') {
        invoke('show_window').catch(() => {});
      } else {
        invoke('set_overlay_mode', { enabled: false }).catch(() => {});
        invoke('set_window_size', { preset: settings.window_size }).catch(() => {});
      }
    }
  }, [gamePhase, settings.auto_hide_in_game, settings.window_size]);

  const handleSaveSettings = async (next: typeof settings) => {
    await saveSettings(next);
    addToast(t('app.settingsSaved'), 'success');
  };

  if (!onboardingDone) {
    return (
      <OnboardingWizard
        onComplete={(syncOk) => {
          setOnboardingDone(true);
          if (syncOk) {
            addToast(t('app.welcomeReady'), 'success');
          } else {
            addToast(t('app.welcomeNoLeague'), 'info');
          }
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
        reconnecting={reconnecting}
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
