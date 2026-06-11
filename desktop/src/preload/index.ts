// Preload — the ONLY bridge between renderer and main (contextIsolation on).
// Mirrors the Tauri `invoke`/`listen` surface as `window.api.*` so the renderer's
// host adapter (src/lib/host.ts) can target either shell unchanged.

import { contextBridge, ipcRenderer } from "electron";

/** Main'in renderer'a yayınladığı kanallar (Tauri event adlarıyla birebir). */
const EVENT_CHANNELS = [
  "champ-select-session",
  "gameflow-phase",
  "lcu-error",
  "lcu:status",
  "opacity-changed",
] as const;

type EventChannel = (typeof EVENT_CHANNELS)[number];

const api = {
  /** Tauri `invoke` paritesi: tek dispatcher kanalı üzerinden komut + named args. */
  invoke: <T>(cmd: string, args?: Record<string, unknown>): Promise<T> =>
    ipcRenderer.invoke("cmd", cmd, args),

  coreSmoke: (): Promise<string> => ipcRenderer.invoke("core:smoke"),
  appStatus: (): Promise<unknown> => ipcRenderer.invoke("app:status"),

  /** Tauri `listen` paritesi: dönen fonksiyon aboneliği kaldırır. */
  on: (
    channel: EventChannel,
    listener: (payload: unknown) => void,
  ): (() => void) => {
    if (!EVENT_CHANNELS.includes(channel)) {
      throw new Error(`bilinmeyen event kanalı: ${channel}`);
    }
    const wrapped = (_e: Electron.IpcRendererEvent, payload: unknown) =>
      listener(payload);
    ipcRenderer.on(channel, wrapped);
    return () => ipcRenderer.removeListener(channel, wrapped);
  },
};

export type DesktopApi = typeof api;

contextBridge.exposeInMainWorld("api", api);
