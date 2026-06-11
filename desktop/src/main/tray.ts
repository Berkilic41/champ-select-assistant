// Sistem tepsisi — minimal yüzey: ikon + "Göster" / "Çıkış" menüsü + çift tık
// ile pencereyi öne getirme. Pencere kapatma davranışı DEĞİŞMEZ (kapat = çık,
// Tauri paritesi); tray yalnız hızlı erişim içindir, arka planda yaşatma değil.

import { app, Menu, Tray, type BrowserWindow } from "electron";

import { trayIconPath } from "./paths";

export interface TrayOptions {
  getMainWindow: () => BrowserWindow | undefined;
}

/** Tray'i kur; referansı çağıran tutar (GC ikonu yok etmesin). */
export function createTray(opts: TrayOptions): Tray {
  const tray = new Tray(trayIconPath());
  tray.setToolTip("Champ Select Assistant");

  const show = () => {
    const win = opts.getMainWindow();
    if (!win) return;
    if (win.isMinimized()) win.restore();
    win.show();
    win.focus();
  };

  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: "Göster", click: show },
      { type: "separator" },
      { label: "Çıkış", click: () => app.quit() },
    ]),
  );
  tray.on("double-click", show);
  return tray;
}
