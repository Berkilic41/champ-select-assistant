export type PickClarityLevel = 'clear' | 'edge' | 'close';

export interface PickClarity {
  level: PickClarityLevel;
  /** total_score[0] − total_score[1]. */
  gap: number;
  /** The runner-up champion key (for the "close"/"edge" framing). */
  runnerUpKey: string | null;
}

interface RecLike {
  champion_key: string;
  total_score: number;
}

/** Above this score gap, #1 is decisively the pick. */
const CLEAR_GAP = 0.06;
/** Above this, #1 has a meaningful edge; below it the top picks are a near tie. */
const EDGE_GAP = 0.025;

/**
 * How decisive is the top recommendation versus the field? An honest read of the real
 * score gap to #2 — never fabricated. `clear` = pick it; `edge` = slight lead; `close`
 * = near-tie between the top two.
 */
export function assessPickClarity(recs: RecLike[]): PickClarity | null {
  if (recs.length === 0) return null;
  if (recs.length === 1) return { level: 'clear', gap: 1, runnerUpKey: null };
  const gap = recs[0].total_score - recs[1].total_score;
  const level: PickClarityLevel = gap >= CLEAR_GAP ? 'clear' : gap >= EDGE_GAP ? 'edge' : 'close';
  return { level, gap, runnerUpKey: recs[1].champion_key };
}
