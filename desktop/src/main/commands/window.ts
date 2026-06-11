// Port of src-tauri/src/commands/window.rs — BrowserWindow equivalents.
//
// Project rule: setSize ONLY runs from explicit user/settings actions (these
// commands ARE those actions); nothing here listens to gameflow.

import { BrowserWindow, screen } from "electron";

/** Tauri set_window_size preset map — identical numbers. */
export function presetSize(preset: string): { width: number; height: number } {
  switch (preset) {
    case "compact":
      return { width: 320, height: 480 };
    case "wide":
      return { width: 1100, height: 700 };
    // Slim horizontal banner pinned over the LCU during champ-select.
    case "overlay":
      return { width: 560, height: 180 };
    default:
      return { width: 800, height: 600 }; // "standard"
  }
}

export function setAlwaysOnTop(win: BrowserWindow, enabled: boolean): void {
  win.setAlwaysOnTop(enabled);
}

export function setWindowSize(win: BrowserWindow, preset: string): void {
  const { width, height } = presetSize(preset);
  win.setSize(width, height);
}

export function hideWindow(win: BrowserWindow): void {
  win.hide();
}

export function showWindow(win: BrowserWindow): void {
  win.show();
  win.focus();
}

/**
 * In-game overlay mode: compact side panel pinned top-right, always-on-top —
 * floats over a borderless game without covering minimap/ability bar (ToS-safe,
 * no injection). On disable, just re-centers (frontend restores preferred size).
 */
export function setOverlayMode(win: BrowserWindow, enabled: boolean): void {
  win.setAlwaysOnTop(true);
  if (enabled) {
    const w = 400;
    const h = 720;
    win.setSize(w, h);
    const display = screen.getDisplayMatching(win.getBounds());
    const x = Math.max(display.workArea.x + display.workArea.width - w - 16, 0);
    win.setPosition(x, 16);
  } else {
    win.center();
  }
}

/** Tauri paritesi: opacity'yi CSS uygulasın diye renderer'a event olarak yollar. */
export function setWindowOpacity(win: BrowserWindow, opacity: number): void {
  win.webContents.send("opacity-changed", opacity);
}
