// Main window factory.
//
// Project rule (feedback_ingame_window): the window must NOT auto-resize when a
// game starts — players alt-tab to read the plan. Any setSize lives behind the
// user's explicit settings action only; nothing here listens to gameflow.

import { existsSync } from "node:fs";
import { join } from "node:path";

import { BrowserWindow } from "electron";

import { resourcesDir } from "./paths";

export function createMainWindow(): BrowserWindow {
  const win = new BrowserWindow({
    width: 1180,
    height: 760,
    minWidth: 900,
    minHeight: 600,
    show: false,
    autoHideMenuBar: true,
    webPreferences: {
      preload: join(__dirname, "..", "preload", "index.js"),
      contextIsolation: true,
      nodeIntegration: false,
      // Safe: the preload imports only contextBridge/ipcRenderer (no Node APIs).
      sandbox: true,
    },
  });

  win.once("ready-to-show", () => win.show());

  // Hardening: this is a single-page app — it never opens child windows and never
  // navigates the top frame to a remote origin. Deny both; allow only the dev
  // server (HMR) and the packaged file:// renderer.
  win.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  win.webContents.on("will-navigate", (event, url) => {
    const devUrl = process.env.ELECTRON_RENDERER_URL;
    if (devUrl && url.startsWith(devUrl)) return;
    if (url.startsWith("file://")) return;
    event.preventDefault();
  });

  const builtRenderer = join(__dirname, "..", "renderer", "index.html");
  if (process.env.ELECTRON_RENDERER_URL) {
    // electron-vite dev server — gerçek React renderer (kökteki src/).
    void win.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else if (existsSync(builtRenderer)) {
    void win.loadFile(builtRenderer);
  } else {
    // Renderer build'i yoksa: statik durum sayfası (main+preload+IPC kanıtı).
    void win.loadFile(join(resourcesDir(), "placeholder.html"));
  }
  return win;
}
