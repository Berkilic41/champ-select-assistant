// Host-agnostic IPC adapter — the ONLY place the renderer touches the shell.
//
// Tauri (bugün): @tauri-apps/api invoke/listen'a delege eder.
// Electron (P1 göçü): preload'un contextBridge ile açtığı `window.api`'ye
// delege eder. Tetkik: `window.api` varlığı. İki host da aynı komut adlarını ve
// Tauri'nin `{ payload }` event zarfını kullanır — bileşenler host'u bilmez.

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';

/** Tauri Event<T> zarfının bileşenlerce kullanılan kısmı. */
export interface HostEvent<T> {
  payload: T;
}

export type UnlistenFn = () => void;

interface ElectronApi {
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  on(channel: string, listener: (payload: unknown) => void): UnlistenFn;
}

declare global {
  interface Window {
    api?: ElectronApi;
  }
}

function electronApi(): ElectronApi | undefined {
  return typeof window !== 'undefined' ? window.api : undefined;
}

/** Tauri `invoke` paritesi: komut adı + named-args objesi. */
export async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const api = electronApi();
  if (api) return api.invoke<T>(cmd, args);
  // args verilmediğinde Tauri'ye HİÇ geçirme — mevcut çağrı imzaları (ve testlerin
  // birebir arg assertion'ları) değişmesin.
  return args === undefined ? tauriInvoke<T>(cmd) : tauriInvoke<T>(cmd, args);
}

/** Tauri `listen` paritesi: `{ payload }` zarfı ve Promise<UnlistenFn> dönüşü. */
export async function listen<T>(
  event: string,
  handler: (event: HostEvent<T>) => void,
): Promise<UnlistenFn> {
  const api = electronApi();
  if (api) {
    return api.on(event, (payload) => handler({ payload: payload as T }));
  }
  return tauriListen<T>(event, (e) => handler({ payload: e.payload }));
}
