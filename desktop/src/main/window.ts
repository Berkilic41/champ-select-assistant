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
      sandbox: false,
    },
  });

  win.once("ready-to-show", () => win.show());

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
