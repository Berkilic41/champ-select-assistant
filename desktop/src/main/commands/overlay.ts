// In-game overlay commands — port of src-tauri/src/commands/overlay.rs
// (get_ingame_plan / get_macro_state). Thin orchestration: fetch the official
// Live Client Data, run the core engine, return the overlay state. No game
// logic here. "No live game" is a normal `null` / `{live:false}` (NOT an error)
// so frontend polling stays silent.

import type { DatabaseSync } from "node:sqlite";

import type { Engine } from "../engine";
import type { FetchAllGameData } from "../live-client";
import { getChampions } from "./champions";

/** OverlayMacroState parity: `live=false` ⇒ port 2999 unreachable, `state` null. */
export interface OverlayMacroState {
  live: boolean;
  state: unknown | null;
}

/** Objective timers / reminders / phase from the live game; polled by the UI. */
export async function getMacroState(
  engine: Engine,
  fetchAllGameData: FetchAllGameData,
): Promise<OverlayMacroState> {
  const raw = await fetchAllGameData();
  if (raw === null) return { live: false, state: null };
  return { live: true, state: engine.macroStateFromAllgamedata(raw) };
}

/** Your in-game game plan (KB-regenerated) + live score. null = no game / not in KB. */
export async function getIngamePlan(
  engine: Engine,
  db: DatabaseSync,
  fetchAllGameData: FetchAllGameData,
): Promise<unknown | null> {
  const raw = await fetchAllGameData();
  if (raw === null) return null;
  return engine.ingamePlan({
    allgamedata: raw,
    all_champions: getChampions(db),
  });
}
