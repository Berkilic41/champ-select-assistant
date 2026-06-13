// SQLite layer — node:sqlite (builtin, Node ≥22.5 / Electron ≥38) + a tiny
// migration runner that executes the EXISTING `src-tauri/migrations/V0xx__*.sql`
// files unchanged (the .sql contents are the contract; only the runner moved
// from Rust/refinery to Node).
//
// node:sqlite yerine better-sqlite3 KULLANILMIYOR: native modül ABI'si Node ve
// Electron arasında uyuşmuyor (NODE_MODULE_VERSION 137 vs 139) — builtin sürücü
// bu sınıf sorunu kökten kaldırır ve native build bağımlılığını düşürür.
//
// The history table mirrors refinery's `refinery_schema_history` shape
// (version, name, applied_on, checksum) so a DB previously migrated by the
// Tauri app is recognised: already-applied versions are skipped by version
// number. New rows record a sha256 checksum (refinery's hash algorithm is not
// replicated — the parallel-running Tauri app must keep its own DB file).

import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, renameSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

export interface Migration {
  version: number;
  name: string;
  path: string;
  sql: string;
}

/** Read + order `V<version>__<name>.sql` files; throws on duplicate versions. */
export function loadMigrations(dir: string): Migration[] {
  const out: Migration[] = [];
  for (const file of readdirSync(dir)) {
    const m = /^V(\d+)__(.+)\.sql$/.exec(file);
    if (!m) continue;
    out.push({
      version: Number(m[1]),
      name: m[2],
      path: join(dir, file),
      sql: readFileSync(join(dir, file), "utf8"),
    });
  }
  out.sort((a, b) => a.version - b.version);
  for (let i = 1; i < out.length; i++) {
    if (out[i].version === out[i - 1].version) {
      throw new Error(`çift migration versiyonu: V${out[i].version}`);
    }
  }
  return out;
}

const HISTORY_TABLE = "refinery_schema_history";

export interface MigrationResult {
  applied: number[];
  skipped: number[];
}

/**
 * Apply pending migrations in version order, each inside a transaction.
 * Returns which versions ran and which were already recorded.
 */
export function runMigrations(
  db: DatabaseSync,
  migrationsDir: string,
): MigrationResult {
  db.exec(
    `CREATE TABLE IF NOT EXISTS ${HISTORY_TABLE} (
       version INTEGER PRIMARY KEY,
       name TEXT NOT NULL,
       applied_on TEXT NOT NULL,
       checksum TEXT NOT NULL
     )`,
  );
  const appliedVersions = new Set<number>(
    (
      db.prepare(`SELECT version FROM ${HISTORY_TABLE}`).all() as unknown as {
        version: number;
      }[]
    ).map((r) => Number(r.version)),
  );

  const result: MigrationResult = { applied: [], skipped: [] };

  for (const mig of loadMigrations(migrationsDir)) {
    if (appliedVersions.has(mig.version)) {
      result.skipped.push(mig.version);
      continue;
    }
    const checksum = createHash("sha256").update(mig.sql).digest("hex");
    db.exec("BEGIN");
    try {
      db.exec(mig.sql);
      db.prepare(
        `INSERT INTO ${HISTORY_TABLE} (version, name, applied_on, checksum) VALUES (?, ?, ?, ?)`,
      ).run(mig.version, mig.name, new Date().toISOString(), checksum);
      db.exec("COMMIT");
    } catch (err) {
      db.exec("ROLLBACK");
      throw new Error(
        `migration V${mig.version}__${mig.name} başarısız: ${(err as Error).message}`,
      );
    }
    result.applied.push(mig.version);
  }
  return result;
}

/** Open (or create) the app DB with WAL + foreign keys, mirroring the Rust setup. */
export function openDatabase(dbPath: string): DatabaseSync {
  const db = new DatabaseSync(dbPath);
  db.exec("PRAGMA journal_mode = WAL");
  db.exec("PRAGMA foreign_keys = ON");
  return db;
}

export interface RecoveredOpen {
  db: DatabaseSync;
  migrations: MigrationResult;
  /** true = bozuk dosya kenara alınıp taze şema kuruldu. */
  recovered: boolean;
}

/**
 * G2 kurtarma yolu: aç + integrity check + migrate; herhangi biri patlarsa
 * bozuk dosya SİLİNMEZ — `.corrupt-<ts>` olarak (WAL/SHM artıklarıyla) kenara
 * alınır ve taze şema kurulur. Maç/meta verileri LCU + kaynak sync'leriyle
 * yeniden dolar; feedback/not/hedef kaybı bozuk dosyadan elle kurtarılabilir.
 */
export function openDatabaseWithRecovery(
  dbPath: string,
  migrationsDir: string,
): RecoveredOpen {
  const tryOpen = (): RecoveredOpen => {
    // openDatabase KULLANILMAZ: pragma aşamasında patlarsa tanıtıcı açık kalır
    // ve Windows'ta bozuk dosya yeniden adlandırılamaz (EPERM). Burada her hata
    // yolunda handle kapatılır.
    const db = new DatabaseSync(dbPath);
    try {
      db.exec("PRAGMA journal_mode = WAL");
      db.exec("PRAGMA foreign_keys = ON");
      // integrity_check "ok" dönmezse bozulmuş kabul et (sessiz dejenerasyon yok).
      const integrity = db
        .prepare("PRAGMA integrity_check")
        .get() as unknown as { integrity_check?: string };
      if ((integrity?.integrity_check ?? "ok") !== "ok") {
        throw new Error(`integrity_check: ${integrity.integrity_check}`);
      }
      return { db, migrations: runMigrations(db, migrationsDir), recovered: false };
    } catch (err) {
      try {
        db.close();
      } catch {
        /* zaten kapalı */
      }
      throw err;
    }
  };

  try {
    return tryOpen();
  } catch (firstErr) {
    console.warn(
      `DB açılamadı (${(firstErr as Error).message}) — bozuk dosya kenara alınıp taze şema kuruluyor`,
    );
    const stamp = Math.floor(Date.now() / 1000);
    for (const suffix of ["", "-wal", "-shm"]) {
      const p = `${dbPath}${suffix}`;
      if (existsSync(p)) {
        try {
          renameSync(p, `${dbPath}.corrupt-${stamp}${suffix}`);
        } catch {
          /* kilitli artık dosya — taze açılış yine denenir */
        }
      }
    }
    const fresh = tryOpen();
    return { ...fresh, recovered: true };
  }
}
