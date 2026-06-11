import { useState, useEffect, useCallback, useRef } from 'react';
import { listen } from '../lib/host';
import { invoke } from '../lib/host';
import { ChampSelectSession, Recommendation, GamePlan, CounterPickHint, TeamCompBoard, ComboBoardEntry, DraftVerdict, CounterItemHint, LaneMatchup } from '../types/recommendation';
import type { Role, RoleSource } from '../components/champ-select/RoleSelector';

/** localStorage key for the player's last manually-chosen role (cross-game default). */
const PREFERRED_ROLE_KEY = 'preferredRole';

/** Trim + lowercase an assigned position; '' when absent. */
function normPos(pos: string | undefined | null): string {
  return (pos ?? '').trim().toLowerCase();
}

export function useChampSelect(puuid: string = ''): {
  session: ChampSelectSession | null;
  recommendations: Recommendation[];
  /** Full analysis (build + game plan) of the local player's LOCKED champion.
   *  null until a champion is locked. Powers post-lock coaching — the locked
   *  pick is excluded from `recommendations`, so the UI pins to this instead. */
  lockedAnalysis: Recommendation | null;
  /** Team-level macro game plan, recomputed as the draft composition changes. */
  gamePlan: GamePlan | null;
  /** Counters from the player's pool vs the visible lane opponent (≤3). */
  counterPicks: CounterPickHint[];
  /** Both teams' composition summaries for the draft board. */
  teamComp: TeamCompBoard | null;
  /** Ally combos for the local player's pick (strongest first). */
  comboBoard: ComboBoardEntry[];
  /** Single decisive draft read (favorability + dodge + top action). */
  draftVerdict: DraftVerdict | null;
  /** Defensive counter-itemization advice vs the enemy comp. */
  counterItems: CounterItemHint[];
  /** Lane matchup read for the local pick vs the visible lane opponent. */
  laneMatchup: LaneMatchup | null;
  /** Effective lane role driving all coaching ('' when unknown). */
  role: string;
  /** Where the effective role came from — 'none' prompts the user to pick. */
  roleSource: RoleSource;
  /** Manually set the local player's role (overrides LCU for this champ-select). */
  setRole: (role: Role) => void;
  isActive: boolean;
  loading: boolean;
  error: string | null;
} {
  const [session, setSession] = useState<ChampSelectSession | null>(null);
  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);
  const [lockedAnalysis, setLockedAnalysis] = useState<Recommendation | null>(null);
  const [gamePlan, setGamePlan] = useState<GamePlan | null>(null);
  const [counterPicks, setCounterPicks] = useState<CounterPickHint[]>([]);
  const [teamComp, setTeamComp] = useState<TeamCompBoard | null>(null);
  const [comboBoard, setComboBoard] = useState<ComboBoardEntry[]>([]);
  const [draftVerdict, setDraftVerdict] = useState<DraftVerdict | null>(null);
  const [counterItems, setCounterItems] = useState<CounterItemHint[]>([]);
  const [laneMatchup, setLaneMatchup] = useState<LaneMatchup | null>(null);
  const [isActive, setIsActive] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // ── Role resolution ─────────────────────────────────────────────────────
  // The whole coaching stack keys off the local player's assigned_position, but
  // LCU leaves it empty in Blind/Normal/Quickplay. Precedence:
  //   manual (this champ-select)  >  LCU-detected  >  persisted preferred  >  ''
  // The manual choice resets when champ-select ends; the persisted one seeds a
  // sensible default next game (e.g. a one-trick bot laner).
  const [manualRole, setManualRole] = useState<string | null>(null);
  const manualRoleRef = useRef<string | null>(null);
  const [lcuRole, setLcuRole] = useState<string>('');
  const preferredRef = useRef<string>(normPos(localStorage.getItem(PREFERRED_ROLE_KEY)));
  const rawSessionRef = useRef<ChampSelectSession | null>(null);

  const prevSessionRef = useRef<ChampSelectSession | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  /** Patch a raw session so the local player's role reflects the effective role.
   *  Returns the raw session unchanged when no override is needed. */
  const applyRole = useCallback((raw: ChampSelectSession | null): ChampSelectSession | null => {
    if (!raw) return null;
    const detected = normPos(raw.local_player.assigned_position);
    const eff = manualRoleRef.current || detected || preferredRef.current || '';
    if (!eff || eff === detected) return raw;
    return {
      ...raw,
      local_player: { ...raw.local_player, assigned_position: eff },
      my_team: raw.my_team.map((s) =>
        s.cell_id === raw.my_cell_id ? { ...s, assigned_position: eff } : s,
      ),
    };
  }, []);

  const fetchRecommendations = useCallback(
    async (payload: ChampSelectSession) => {
      setLoading(true);
      setError(null);
      try {
        const recs = await invoke<Recommendation[]>('get_draft_brain_recommendations', {
          sessionJson: payload,
          puuid,
        });
        setRecommendations(recs);
      } catch (e) {
        setError('Öneri alınamadı: ' + String(e));
      } finally {
        setLoading(false);
      }
    },
    [puuid],
  );

  // User picks a role manually — overrides LCU for this champ-select, persists as
  // the cross-game default, and immediately re-runs the role-dependent coaching.
  const setRole = useCallback(
    (role: Role) => {
      manualRoleRef.current = role;
      setManualRole(role);
      preferredRef.current = role;
      localStorage.setItem(PREFERRED_ROLE_KEY, role);

      const eff = applyRole(rawSessionRef.current);
      if (eff) {
        prevSessionRef.current = eff;
        setSession(eff);
        fetchRecommendations(eff);
      }
    },
    [applyRole, fetchRecommendations],
  );

  useEffect(() => {
    const unlistenPromise = listen<ChampSelectSession | null>(
      'champ-select-session',
      (event) => {
        if (!event.payload) {
          setSession(null);
          setIsActive(false);
          setRecommendations([]);
          setError(null);
          prevSessionRef.current = null;
          rawSessionRef.current = null;
          // Champ-select ended — drop the manual override so the next draft
          // starts from LCU detection again.
          manualRoleRef.current = null;
          setManualRole(null);
          setLcuRole('');
          if (debounceRef.current) clearTimeout(debounceRef.current);
          return;
        }

        const raw = event.payload;
        rawSessionRef.current = raw;
        setLcuRole(normPos(raw.local_player.assigned_position));

        const next = applyRole(raw);
        if (!next) return;

        const prev = prevSessionRef.current;
        prevSessionRef.current = next;

        setSession(next);
        setIsActive(true);

        const phaseChanged = prev?.phase !== next.phase;
        const locksChanged = JSON.stringify(
          [...next.my_team, ...next.their_team].map(s => s.champion_id),
        ) !== JSON.stringify(
          [...(prev?.my_team ?? []), ...(prev?.their_team ?? [])].map(s => s.champion_id),
        );

        if (phaseChanged || locksChanged) {
          if (debounceRef.current) clearTimeout(debounceRef.current);
          fetchRecommendations(next);
        } else {
          if (debounceRef.current) clearTimeout(debounceRef.current);
          debounceRef.current = setTimeout(() => fetchRecommendations(next), 800);
        }
      },
    );

    return () => {
      unlistenPromise.then((fn) => fn());
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [fetchRecommendations, applyRole]);

  // Fetch the LOCKED champion's full analysis so the UI can pin to YOUR pick
  // after lock. `compute_recommendations` excludes already-picked champions, so
  // without this the finalization view falls back to recommendations[0] — a
  // different champion (the "random champion + wrong build" bug).
  const lockedChampionId = session?.local_player.champion_id ?? 0;
  const effectivePos = normPos(session?.local_player.assigned_position);
  useEffect(() => {
    if (!session || lockedChampionId <= 0) {
      setLockedAnalysis(null);
      return;
    }
    let cancelled = false;
    invoke<Recommendation | null>('get_champion_analysis', {
      sessionJson: session,
      championId: lockedChampionId,
      puuid,
    })
      .then((rec) => {
        if (!cancelled) setLockedAnalysis(rec ?? null);
      })
      .catch(() => {
        if (!cancelled) setLockedAnalysis(null);
      });
    return () => {
      cancelled = true;
    };
    // Re-fetch when the locked champion, player identity, or role changes;
    // `session` is read fresh on each run.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lockedChampionId, puuid, effectivePos]);

  // Team-level macro game plan — recompute whenever the visible composition or
  // the local player's role changes (any lock on either team, hover, role pick).
  const teamSignature = session
    ? JSON.stringify([
        ...session.my_team.map((s) => s.champion_id),
        ...session.their_team.map((s) => s.champion_id),
        session.local_player.intent_champion_id,
        effectivePos,
      ])
    : '';
  useEffect(() => {
    if (!session) {
      setGamePlan(null);
      return;
    }
    let cancelled = false;
    invoke<GamePlan>('get_game_plan', { sessionJson: session })
      .then((p) => {
        if (!cancelled) setGamePlan(p);
      })
      .catch(() => {
        if (!cancelled) setGamePlan(null);
      });
    return () => {
      cancelled = true;
    };
    // `session` is read fresh each run; keyed on the composition signature.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [teamSignature]);

  // Counter-picks from the player's pool vs the visible lane opponent.
  // Backend returns [] when no opponent is locked, so we can fetch on any
  // composition change without gating on opponent presence here.
  useEffect(() => {
    if (!session) {
      setCounterPicks([]);
      return;
    }
    let cancelled = false;
    invoke<CounterPickHint[]>('get_counter_picks', { sessionJson: session, puuid })
      .then((c) => {
        if (!cancelled) setCounterPicks(c ?? []);
      })
      .catch(() => {
        if (!cancelled) setCounterPicks([]);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [teamSignature, puuid]);

  // Both teams' composition summaries for the draft board.
  useEffect(() => {
    if (!session) {
      setTeamComp(null);
      return;
    }
    let cancelled = false;
    invoke<TeamCompBoard>('get_team_comp', { sessionJson: session })
      .then((tc) => {
        if (!cancelled) setTeamComp(tc);
      })
      .catch(() => {
        if (!cancelled) setTeamComp(null);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [teamSignature]);

  // Ally combos for the local player's pick (locked or hovered).
  useEffect(() => {
    if (!session) {
      setComboBoard([]);
      return;
    }
    let cancelled = false;
    invoke<ComboBoardEntry[]>('get_combo_board', { sessionJson: session })
      .then((c) => {
        if (!cancelled) setComboBoard(c ?? []);
      })
      .catch(() => {
        if (!cancelled) setComboBoard([]);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [teamSignature]);

  // Draft verdict — one decisive read; recomputed as the composition changes.
  useEffect(() => {
    if (!session) {
      setDraftVerdict(null);
      return;
    }
    let cancelled = false;
    invoke<DraftVerdict>('get_draft_verdict', { sessionJson: session, puuid })
      .then((v) => {
        if (!cancelled) setDraftVerdict(v);
      })
      .catch(() => {
        if (!cancelled) setDraftVerdict(null);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [teamSignature, puuid]);

  // Counter-item advice vs the enemy comp.
  useEffect(() => {
    if (!session) {
      setCounterItems([]);
      return;
    }
    let cancelled = false;
    invoke<CounterItemHint[]>('get_counter_items', { sessionJson: session })
      .then((c) => {
        if (!cancelled) setCounterItems(c ?? []);
      })
      .catch(() => {
        if (!cancelled) setCounterItems([]);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [teamSignature]);

  // Lane matchup for the local pick vs the visible lane opponent.
  useEffect(() => {
    if (!session) {
      setLaneMatchup(null);
      return;
    }
    let cancelled = false;
    invoke<LaneMatchup | null>('get_lane_matchup', { sessionJson: session })
      .then((m) => {
        if (!cancelled) setLaneMatchup(m ?? null);
      })
      .catch(() => {
        if (!cancelled) setLaneMatchup(null);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [teamSignature]);

  // Effective role + its provenance (for the RoleSelector UI).
  const role = manualRole || lcuRole || preferredRef.current || '';
  const roleSource: RoleSource = manualRole
    ? 'manual'
    : lcuRole
      ? 'lcu'
      : preferredRef.current
        ? 'manual'
        : 'none';

  return { session, recommendations, lockedAnalysis, gamePlan, counterPicks, teamComp, comboBoard, draftVerdict, counterItems, laneMatchup, role, roleSource, setRole, isActive, loading, error };
}
