// Electron IPC adapter — the ONLY place the renderer touches the shell.
// Tauri host emekli edildi (commit 26d1471): tek host Electron; preload'un
// contextBridge ile açtığı `window.api` zorunlu. Yokluğu sessiz boş veri DEĞİL
// dürüst hatadır (preload yüklenmemiş demektir).

/** Eski Tauri Event<T> zarfının bileşenlerce kullanılan kısmı (şekil korundu). */
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

function requireApi(): ElectronApi {
  const api = typeof window !== 'undefined' ? window.api : undefined;
  if (!api) {
    throw new Error(
      'Electron host bulunamadı: window.api tanımsız (preload yüklenmedi mi?)',
    );
  }
  return api;
}

/** Komut adı + named-args objesi (eski Tauri invoke imzasıyla birebir). */
export async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return requireApi().invoke<T>(cmd, args);
}

/** `{ payload }` zarfı ve Promise<UnlistenFn> dönüşü (eski imza korundu). */
export async function listen<T>(
  event: string,
  handler: (event: HostEvent<T>) => void,
): Promise<UnlistenFn> {
  return requireApi().on(event, (payload) => handler({ payload: payload as T }));
}