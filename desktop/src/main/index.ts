// Electron main entry — boots the host: single-instance lock, DB + migrations,
// core.wasm engine, LCU service, IPC, window.
//
// NOT: dev/parite döneminde DB, Tauri'nin dosyasından AYRI tutulur (userData
// altında csa-electron.db) — iki app aynı SQLite dosyasını paylaşMAZ.

import { join } from "node:path";
import type { DatabaseSync } from "node:sqlite";

import { app, BrowserWindow, session, type Tray } from "electron";

import { LcuService } from "./commands/lcu";
import { OutcomeTracker, RecsCache } from "./commands/outcomes";
import { trainLocalModelPack } from "./commands/model-training";
import { syncLcuPlayerData } from "./commands/player-data";
import { openDatabaseWithRecovery } from "./db";
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
  let db: DatabaseSync | undefined;
  let scheduler: PipelineScheduler | undefined;
  let tray: Tray | undefined;
  let outcomeTracker: OutcomeTracker | undefined;

  // 2B: son öneri listesi cache'i + öneri→pick→sonuç köprüsü (gameflow-driven).
  const recsCache = new RecsCache();

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
    // 2B: pick yakalama + oyun-sonu resolve OutcomeTracker'a delege edilir
    // (emit'e EK — renderer event akışı aynen korunur).
    onChampSelectSession: (state) => outcomeTracker?.onChampSelectSession(state),
    onGameflowPhase: (phase) => outcomeTracker?.onGameflowPhase(phase),
  });

  // LcuService'ten SONRA kur (syncMatches `lcu`'yu kapsar); callback'ler boot
  // sonrası ateşlendiği için `outcomeTracker`/`db` o ana dek atanmış olur.
  outcomeTracker = new OutcomeTracker({
    getDb: () => db,
    recs: recsCache,
    syncMatches: async () => {
      if (db) await syncLcuPlayerData(db, lcu, undefined);
      // 3B: oyun-sonu yeni etiketler çözülünce ModelPack'i tazele (best-effort).
      if (db && engine) {
        try {
          trainLocalModelPack(engine, db);
        } catch (err) {
          console.warn("oyun-sonu ModelPack eğitimi atlandı:", (err as Error).message);
        }
      }
    },
  });

  app.on("second-instance", () => {
    if (mainWindow) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.focus();
    }
  });

  void app.whenReady().then(() => {
    // 0. Content-Security-Policy for the PACKAGED renderer (file://). Skipped in
    //    dev so electron-vite's HMR (inline scripts + ws) keeps working. Champion
    //    art loads from DDragon + CommunityDragon; everything else is same-origin.
    if (!process.env.ELECTRON_RENDERER_URL) {
      session.defaultSession.webRequest.onHeadersReceived((details, cb) => {
        cb({
          responseHeaders: {
            ...details.responseHeaders,
            "Content-Security-Policy": [
              "default-src 'self'; " +
                "img-src 'self' data: https://ddragon.leagueoflegends.com https://raw.communitydragon.org; " +
                "script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; " +
                "connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
            ],
          },
        });
      });
    }

    // 1. DB + migrations (önce — Rust tarafındaki "migrations before any DB access"
    //    kuralı). G2: bozulmada dosya kenara alınır + taze şema kurulur.
    //    `db` dış kapsamda (OutcomeTracker.getDb + syncMatches onu kapsar).
    try {
      const opened = openDatabaseWithRecovery(
        join(app.getPath("userData"), "csa-electron.db"),
        migrationsDir(),
      );
      db = opened.db;
      status.db.migrated =
        opened.migrations.applied.length + opened.migrations.skipped.length;
      if (opened.recovered) {
        // Dürüst sinyal: veri sıfırdan dolacak; bozuk dosya .corrupt-* olarak duruyor.
        status.db.error = "kurtarıldı: bozuk DB kenara alındı, taze şema kuruldu";
      }
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
      // 3B: boot'ta birikmiş etiketlerden ModelPack'i tazele (best-effort; gate
      // altıysa no-op → local_rules fallback).
      try {
        trainLocalModelPack(engine, db);
      } catch (err) {
        console.warn("boot ModelPack eğitimi atlandı:", (err as Error).message);
      }
    }

    // 4. IPC + pencere. LCU bağlantısını Tauri'deki gibi RENDERER tetikler
    //    (connect_lcu) — boot'ta otomatik bağlanma yok.
    registerIpcHandlers({
      engine,
      db,
      lcu,
      caches,
      recsCache,
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
