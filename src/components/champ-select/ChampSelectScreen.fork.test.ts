import { describe, expect, it } from 'vitest';
import { selectDraftForkIds } from './ChampSelectScreen';
import type { Recommendation } from '../../types/recommendation';

function rec(championId: number): Recommendation {
  return { champion_id: championId } as Recommendation;
}

describe('selectDraftForkIds', () => {
  it('compares the active recommendation against the best different alternative', () => {
    expect(selectDraftForkIds([rec(89), rec(111), rec(157)], 2)).toEqual([157, 89]);
  });

  it('returns null when there is no valid two-option fork', () => {
    expect(selectDraftForkIds([rec(89)], 0)).toBeNull();
    expect(selectDraftForkIds([rec(89), rec(89)], 0)).toBeNull();
    expect(selectDraftForkIds([rec(89), rec(111)], 4)).toBeNull();
  });
});
