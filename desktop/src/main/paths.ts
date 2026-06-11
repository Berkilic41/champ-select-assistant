// Path resolution that survives bundling. electron-vite inlines src/main/** into
// a single out/main/index.js, so per-file `__dirname` depths change between dev
// (ts source) and the bundle. Instead of hardcoding depths we probe upward from
// `__dirname` for the known anchors.

import { existsSync } from "node:fs";
import { dirname, join } from "node:path";

/** Walk up from `start` until `relative` exists; throws if never found. */
export function findUp(relative: string, start: string = __dirname): string {
  let dir = start;
  for (let i = 0; i < 8; i++) {
    const candidate = join(dir, relative);
    if (existsSync(candidate)) return candidate;
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  throw new Error(`yol bulunamadı: ${relative} (başlangıç: ${start})`);
}

/** desktop/resources — pinned CA, placeholder renderer, packaged assets. */
export function resourcesDir(): string {
  return findUp(join("resources", "riotgames.pem")).replace(
    /[\\/]riotgames\.pem$/,
    "",
  );
}

/** The wasm-pack output of csa-core (dev: sibling core/pkg; packaged: resources). */
export function corePkgPath(): string {
  // Packaged builds copy core/pkg into resources/core-pkg (electron-builder
  // extraResources); dev resolves the sibling crate output.
  try {
    return findUp(join("resources", "core-pkg", "csa_core.js"));
  } catch {
    return findUp(join("core", "pkg", "csa_core.js"));
  }
}

/** refinery SQL migration dizini — desktop sahipliğinde (resources/migrations;
 *  dev'de desktop/resources, paketlide <install>/resources — tek kaynak,
 *  src-tauri emekli edildi). */
export function migrationsDir(): string {
  return join(resourcesDir(), "migrations");
}

/** Bundled seed JSON'u (resources/seeds/<name>; `name` alt dizin içerebilir,
 *  ör. meta/matchup_seed.json — yapı korunur). */
export function seedFile(name: string): string {
  return join(resourcesDir(), "seeds", name);
}

/** Tray/pencere ikonu (dev VE paketli: resources/icon.ico — src-tauri/icons kopyası). */
export function trayIconPath(): string {
  return join(resourcesDir(), "icon.ico");
}
