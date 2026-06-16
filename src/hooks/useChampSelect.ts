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

/** Cancellable "derive state from the current champ-select session" fetch,
 *  re-run whenever `signature` (or the puuid) changes. Centralizes the identical
 *  derived-coaching effects: clears to `fallback` when no session, latest-wins
 *  cancel guard, `fallback` on error/nullish. `withPuuid=null` omits the puuid
 *  arg + dep; a string threads it into both the args and the dep list. */
function useSessionDerived<T>(
  session: ChampSelectSession | null,
  signature: string,
  command: string,
  fallback: T,
  withPuuid: string | null = null,
): T {
  const [value, setValue] = useState<T>(fallback);
  useEffect(() => {
    if (!session) {
      setValue(fallback);
      return;
    }
    let cancelled = false;
    const args =
      withPuuid !== null
        ? { sessionJson: session, puuid: withPuuid }
        : { sessionJson: session };
    invoke<T>(command, args)
      .then((v) => {
        if (!cancelled) setValue((v ?? fallback) as T);
      })
      .catch(() => {
        if (!cancelled) setValue(fallback);
      });
    return () => {
      cancelled = true;
    };
    // `session` is read fresh each run; keyed on the signature (+ puuid).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [signature, withPuuid]);
  return value;
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

  const fetchSeqRef = useRef(0);
  const fetchRecommendations = useCallback(
    async (payload: ChampSelectSession) => {
      // Sonuç-yarışı koruması: her çağrı bir sıra no alır ve yalnız EN GÜNCEL
      // çağrının sonucu uygulanır. Out-of-order yanıtın eskiyi ezmesini ve session
      // bittikten (null) sonra bayat recs yazılmasını engeller.
      const seq = ++fetchSeqRef.current;
      setLoading(true);
      setError(null);
      try {
        const recs = await invoke<Recommendation[]>('get_draft_brain_recommendations', {
          sessionJson: payload,
          puuid,
        });
        if (seq === fetchSeqRef.current) setRecommendations(recs);
      } catch (e) {
        if (seq === fetchSeqRef.current) setError('Öneri alınamadı: ' + String(e));
      } finally {
        if (seq === fetchSeqRef.current) setLoading(false);
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
          fetchSeqRef.current++; // uçuştaki fetch'leri geçersiz kıl (bayat recs yazılmasın)
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

  // puuid mount'ta asenkron çözülür (ChampSelectWrapper). İlk 'champ-select-session'
  // event'i puuid '' iken gelmiş olabilir → öneriler kişiselleştirmesiz (mastery'siz)
  // hesaplanır ve session listener son event'i replay etmediğinden yeniden tetiklenmez.
  // puuid çözülünce mevcut session için önerileri YENİDEN çek (sonraki hover/lock'a
  // bağlı kalmasın).
  const puuidRef = useRef(puuid);
  useEffect(() => {
    const prevPuuid = puuidRef.current;
    puuidRef.current = puuid;
    if (puuid && puuid !== prevPuuid && rawSessionRef.current) {
      const eff = applyRole(rawSessionRef.current);
      if (eff) fetchRecommendations(eff);
    }
  }, [puuid, applyRole, fetchRecommendations]);

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

  // Shared signature for the session-derived coaching outputs below — changes
  // whenever the visible composition or the local player's role changes (any lock
  // on either team, hover, role pick), re-running every derived fetch.
  const teamSignature = session
    ? JSON.stringify([
        ...session.my_team.map((s) => s.champion_id),
        ...session.their_team.map((s) => s.champion_id),
        session.local_player.intent_champion_id,
        effectivePos,
      ])
    : '';
  // All seven coaching outputs derive from the same composition signature via the
  // shared cancellable fetch helper (clears on no-session, latest-wins guard).
  // The two personalized reads (counter-picks, draft verdict) thread the puuid;
  // counter-picks return [] backend-side when no opponent is locked, so fetching
  // on any composition change is safe.
  const gamePlan = useSessionDerived<GamePlan | null>(
    session, teamSignature, 'get_game_plan', null,
  );
  const counterPicks = useSessionDerived<CounterPickHint[]>(
    session, teamSignature, 'get_counter_picks', [], puuid,
  );
  const teamComp = useSessionDerived<TeamCompBoard | null>(
    session, teamSignature, 'get_team_comp', null,
  );
  const comboBoard = useSessionDerived<ComboBoardEntry[]>(
    session, teamSignature, 'get_combo_board', [],
  );
  const draftVerdict = useSessionDerived<DraftVerdict | null>(
    session, teamSignature, 'get_draft_verdict', null, puuid,
  );
  const counterItems = useSessionDerived<CounterItemHint[]>(
    session, teamSignature, 'get_counter_items', [],
  );
  const laneMatchup = useSessionDerived<LaneMatchup | null>(
    session, teamSignature, 'get_lane_matchup', null,
  );

  // Effective role + its provenance (for the RoleSelector UI).
  const role = manualRole || lcuRole || preferredRef.current || '';
  const roleSource: RoleSource = manualRole
    ? 'manual'
    : lcuRole
      ? 'lcu'
      : preferredRef.current
        ? 'preferred'
        : 'none';

  return { session, recommendations, lockedAnalysis, gamePlan, counterPicks, teamComp, comboBoard, draftVerdict, counterItems, laneMatchup, role, roleSource, setRole, isActive, loading, error };
}
