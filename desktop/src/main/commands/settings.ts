// Port of src-tauri/src/commands/settings.rs — app_config key-value store.

import type { DatabaseSync } from "node:sqlite";

export interface AppSettings {
  weight_comfort: number;
  weight_matchup: number;
  weight_team_counter: number;
  weight_synergy: number;
  weight_meta: number;
  weight_role_fit: number;
  always_on_top: boolean;
  window_size: string; // "compact" | "standard" | "wide"
  auto_hide_in_game: boolean;
  sounds_enabled: boolean;
  language: string; // "tr" | "en"
  platform_region: string; // e.g. "tr1", "euw1"
  // Privacy: explicit opt-in to upload anonymized recommendation feedback.
  // OFF by default — nothing leaves the device unless the user turns this on.
  share_anonymous_feedback: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  weight_comfort: 0.2,
  weight_matchup: 0.25,
  weight_team_counter: 0.15,
  weight_synergy: 0.1,
  weight_meta: 0.15,
  weight_role_fit: 0.15,
  always_on_top: true,
  window_size: "standard",
  auto_hide_in_game: false,
  sounds_enabled: false,
  language: "tr",
  platform_region: "tr1",
  share_anonymous_feedback: false,
};

/** Rust serde paritesi: bu alanlardan biri eksikse TÜM ayarlar default'a düşer
 *  (`language`/`platform_region` hariç — onların serde default'u var). */
const REQUIRED_KEYS: (keyof AppSettings)[] = [
  "weight_comfort",
  "weight_team_counter",
  "weight_synergy",
  "weight_meta",
  "weight_role_fit",
  "always_on_top",
  "window_size",
  "auto_hide_in_game",
  "sounds_enabled",
];

function configGet(db: DatabaseSync, key: string): string | undefined {
  const row = db
    .prepare("SELECT value FROM app_config WHERE key = ?")
    .get(key) as unknown as { value: string } | undefined;
  return row?.value;
}

function configSet(db: DatabaseSync, key: string, value: string): void {
  db.prepare("INSERT OR REPLACE INTO app_config (key, value) VALUES (?, ?)").run(
    key,
    value,
  );
}

export function getSettings(db: DatabaseSync): AppSettings {
  const raw = configGet(db, "settings");
  if (!raw) return { ...DEFAULT_SETTINGS };
  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(raw) as Record<string, unknown>;
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
  // serde alias: weight_lane_counter → weight_matchup (Sprint E rename).
  if (parsed.weight_matchup === undefined && parsed.weight_lane_counter !== undefined) {
    parsed.weight_matchup = parsed.weight_lane_counter;
  }
  if (parsed.weight_matchup === undefined) return { ...DEFAULT_SETTINGS };
  for (const key of REQUIRED_KEYS) {
    if (parsed[key] === undefined) return { ...DEFAULT_SETTINGS };
  }
  return {
    ...parsed,
    language: (parsed.language as string) ?? DEFAULT_SETTINGS.language,
    platform_region:
      (parsed.platform_region as string) ?? DEFAULT_SETTINGS.platform_region,
    // Optional-default (NOT in REQUIRED_KEYS) so old stored settings without
    // this key keep all their values instead of resetting to defaults.
    share_anonymous_feedback:
      typeof parsed.share_anonymous_feedback === "boolean"
        ? parsed.share_anonymous_feedback
        : DEFAULT_SETTINGS.share_anonymous_feedback,
  } as AppSettings;
}

export function saveSettings(db: DatabaseSync, settings: AppSettings): void {
  configSet(db, "settings", JSON.stringify(settings));
}

export function isOnboardingComplete(db: DatabaseSync): boolean {
  return configGet(db, "onboarding_complete") === "true";
}

export function completeOnboarding(db: DatabaseSync): void {
  configSet(db, "onboarding_complete", "true");
}
