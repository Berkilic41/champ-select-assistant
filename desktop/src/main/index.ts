// Electron main entry — boots the host: single-instance lock, DB + migrations,
// core.wasm engine, LCU service, IPC, window.
//
// NOT: dev/parite döneminde DB, Tauri'nin dosyasından AYRI tutulur (userData
// altında csa-electron.db) — iki app aynı SQLite dosyasını paylaşMAZ.

import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { app, BrowserWindow, type Tray } from "electron";

import { LcuService } from "./commands/lcu";
import { openDatabase, runMigrations } from "./db";
import { emptyCaches } from "./ddragon";
import { Engine } from "./engine";
import { registerIpcHandlers, type AppStatus } from "./ipc";
import { createLiveClientFetcher } from "./live-client";
import { migrationsDir } from "./paths";
import { PipelineScheduler } from "./scheduler";
import { createTray } from "./tray";
import { setupAutoUpdater } from "./updater";
import { createMainWindow } from "./window";

const gotLock = app.requestSingleInstanceLock();
if (!gotLock) {
  app.quit();
} else {
  let mainWindow: BrowserWindow | undefined;
  let engine: Engine | undefined;
  let scheduler: PipelineScheduler | undefined;
  let tray: Tray | undefined;

  const status: AppStatus = {
    engine: "failed",
    db: { migrated: 0 },
    lcu: "searching",
  };

  const lcu = new LcuService({
    emit: (channel, payload) => mainWindow?.webContents.send(channel, payload),
    onStatus: (s) => {
      status.lcu = s;
      mainWindow?.webContents.send("lcu:status", s);
    },
    // Tauri paritesi: renderer ham LCU session DEĞİL parse edilmiş state alır.
    // Motor yüklenemediyse event yayınlanmaz (sessiz yanlış-şekil beslemek yasak).
    parseSession: (raw) => {
      if (!engine) {
        console.warn("champ-select-session: motor yok, event atlandı");
        return null;
      }
      return engine.parseSession(raw);
    },
  });

  app.on("second-instance", () => {
    if (mainWindow) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.focus();
    }
  });

  void app.whenReady().then(() => {
    // 1. DB + migrations (önce — Rust tarafındaki "migrations before any DB access" kuralı).
    let db: DatabaseSync | undefined;
    try {
      db = openDatabase(join(app.getPath("userData"), "csa-electron.db"));
      const result = runMigrations(db, migrationsDir());
      status.db.migrated = result.applied.length + result.skipped.length;
    } catch (err) {
      status.db.error = (err as Error).message;
    }

    // 2. core.wasm motoru.
    try {
      engine = Engine.load();
      status.engine = "ready";
    } catch (err) {
      console.error("csa-core yüklenemedi:", err);
    }

    // 3. Arka plan pipeline scheduler'ı (Tauri start_data_pipeline_scheduler
    //    paritesi: 30s gecikme + 3dk tick; champ-select guard'ı plan içinde).
    const caches = emptyCaches();
    if (engine && db) {
      scheduler = new PipelineScheduler({ engine, db, lcu, caches });
      scheduler.start();
    }

    // 4. IPC + pencere. LCU bağlantısını Tauri'deki gibi RENDERER tetikler
    //    (connect_lcu) — boot'ta otomatik bağlanma yok.
    registerIpcHandlers({
      engine,
      db,
      lcu,
      caches,
      fetchAllGameData: createLiveClientFetcher(),
      scheduler,
      getMainWindow: () => mainWindow,
      status: () => ({ ...status }),
    });
    mainWindow = createMainWindow();
    mainWindow.on("closed", () => (mainWindow = undefined));

    // 5. Sistem tepsisi (Göster/Çıkış) — ikon yüklenemezse app tray'siz devam eder.
    try {
      tray = createTray({ getMainWindow: () => mainWindow });
    } catch (err) {
      console.warn("Tray kurulamadı:", (err as Error).message);
    }

    // 6. Otomatik güncelleme (yalnız paketli build; hata best-effort loglanır).
    setupAutoUpdater();

    // CSA_SMOKE=1: boot'u doğrula, durumu yaz, çık (CI / headless duman testi).
    if (process.env.CSA_SMOKE) {
      mainWindow.webContents.once("did-finish-load", () => {
        console.log(`CSA_SMOKE ${JSON.stringify(status)}`);
        app.quit();
      });
    }
  });

  app.on("window-all-closed", () => {
    scheduler?.stop();
    lcu.stop();
    tray?.destroy();
    tray = undefined;
    app.quit();
  });
}
