// Minimal Electron stub for vitest.
//
// Desktop unit tests exercise pure helpers (presetSize, settings, db, engine,
// scheduler) — NOT real Electron objects. Several main-process modules import
// `electron` at the top level, so loading those modules in plain Node throws
// "Electron failed to install correctly" (CI has no Electron runtime). The
// desktop vitest config aliases `electron` to this stub so the imports resolve
// and the tests run deterministically in Node. Any test that genuinely needs
// Electron behaviour should mock the specific call with `vi.mock`.

const noop = (): undefined => undefined;
const DISPLAY = { workArea: { x: 0, y: 0, width: 1920, height: 1080 } };

export class BrowserWindow {}
export class Tray {}

export const screen = {
  getPrimaryDisplay: () => DISPLAY,
  getDisplayMatching: () => DISPLAY,
};

export const app = {
  getPath: () => "/tmp",
  getVersion: () => "0.0.0-test",
  whenReady: () => Promise.resolve(),
  on: noop,
  quit: noop,
};

export const Menu = {
  buildFromTemplate: () => ({}),
  setApplicationMenu: noop,
};

export const ipcMain = { handle: noop, on: noop, removeHandler: noop };
export const ipcRenderer = { invoke: () => Promise.resolve(), on: noop, send: noop };
export const contextBridge = { exposeInMainWorld: noop };

export default {
  BrowserWindow,
  Tray,
  screen,
  app,
  Menu,
  ipcMain,
  ipcRenderer,
  contextBridge,
};
