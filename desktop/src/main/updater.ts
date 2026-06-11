// Otomatik güncelleme — electron-updater + GitHub Releases (electron-builder.yml
// `publish` hedefi). Yalnız PAKETLİ build'de koşar (dev'de feed yok); her hata
// dürüstçe loglanır ve uygulamayı ASLA düşürmez (güncelleme best-effort).
//
// İmza notu: kod-imzalama sertifikası YOK — Windows'ta imzasız→imzasız NSIS
// güncellemesi çalışır; sertifika alınırsa `verifyUpdateCodeSignature` ve
// SmartScreen ayrı release diliminde ele alınır.

import { app } from "electron";
import { autoUpdater } from "electron-updater";

export function setupAutoUpdater(): void {
  if (!app.isPackaged) return; // dev ortamında güncelleme kontrolü anlamsız

  autoUpdater.on("error", (err) =>
    console.warn("Güncelleme hatası:", err.message),
  );
  autoUpdater.on("update-available", (info) =>
    console.log(`Güncelleme bulundu: v${info.version} (indiriliyor)`),
  );
  autoUpdater.on("update-not-available", () =>
    console.log("Uygulama güncel."),
  );
  autoUpdater.on("update-downloaded", (info) =>
    console.log(
      `Güncelleme indirildi: v${info.version} — uygulama kapanınca kurulacak.`,
    ),
  );

  void autoUpdater.checkForUpdatesAndNotify().catch((err) => {
    console.warn("Güncelleme kontrolü başarısız:", (err as Error).message);
  });
}
