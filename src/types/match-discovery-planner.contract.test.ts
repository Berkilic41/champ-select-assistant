import { describe, it, expect } from 'vitest';
import type { MatchDiscoveryPlan } from './generated/MatchDiscoveryPlan';
import type { PlayerCrawlDecision } from './generated/PlayerCrawlDecision';
import type { MatchDiscoveryDecision } from './generated/MatchDiscoveryDecision';

// Compile-time contract guard for the Match Discovery Planner output (Sprint). A
// Rust field add/remove/retype regenerates these and breaks `pnpm typecheck` here
// before Codex's crawl/fetch-history binding can drift. No bigint (priority/counts
// are number). PII-safe: only `puuid_hash`, never a raw PUUID field.

const playerDecision: PlayerCrawlDecision = {
  puuid_hash: 'h1',
  region: 'euw1',
  decision: 'crawl',
  reason: 'r',
  priority: 3,
};

const matchDecision: MatchDiscoveryDecision = {
  match_id: 'EUW1_1',
  region: 'euw1',
  decision: 'new',
  reason: 'r',
};

describe('Match discovery planner contract', () => {
  it('MatchDiscoveryPlan bundles crawl/match decisions + counts', () => {
    const keys: Record<keyof MatchDiscoveryPlan, true> = {
      to_crawl: true,
      new_match_ids: true,
      player_decisions: true,
      match_decisions: true,
      selected_crawl_count: true,
      new_match_count: true,
      skipped_count: true,
    };
    expect(Object.keys(keys).sort()).toEqual([
      'match_decisions',
      'new_match_count',
      'new_match_ids',
      'player_decisions',
      'selected_crawl_count',
      'skipped_count',
      'to_crawl',
    ]);

    const plan: MatchDiscoveryPlan = {
      to_crawl: ['h1'],
      new_match_ids: ['EUW1_1'],
      player_decisions: [playerDecision],
      match_decisions: [matchDecision],
      selected_crawl_count: 1,
      new_match_count: 1,
      skipped_count: 0,
    };
    expect(plan.to_crawl[0]).toBe('h1');
    expect(plan.player_decisions[0].puuid_hash).toBe('h1');
    expect(typeof plan.selected_crawl_count).toBe('number');
    expect(typeof plan.new_match_count).toBe('number');
    expect(typeof plan.skipped_count).toBe('number');
  });

  it('nested shapes are exact + PII-safe', () => {
    const p: Record<keyof PlayerCrawlDecision, true> = {
      puuid_hash: true, region: true, decision: true, reason: true, priority: true,
    };
    const m: Record<keyof MatchDiscoveryDecision, true> = {
      match_id: true, region: true, decision: true, reason: true,
    };
    expect(Object.keys(p).length).toBe(5);
    expect(Object.keys(m).length).toBe(4);
    // PII guard: the crawl decision only exposes a hash, never a raw PUUID field.
    const playerKeys = Object.keys(p);
    expect(playerKeys).toContain('puuid_hash');
    expect(playerKeys).not.toContain('puuid');
    expect(playerKeys).not.toContain('summoner_name');

    // Decision token vocabulary (Rust-locked in PLAYER_DECISIONS / MATCH_DECISIONS).
    const playerDecisions: Array<PlayerCrawlDecision['decision']> = [
      'crawl', 'skip_already_crawled', 'skip_champ_select',
      'skip_budget', 'skip_breadth_full', 'skip_invalid',
    ];
    const matchDecisions: Array<MatchDiscoveryDecision['decision']> = [
      'new', 'skip_known', 'skip_invalid', 'skip_player_cap',
    ];
    expect(playerDecisions).toContain(playerDecision.decision);
    expect(matchDecisions).toContain(matchDecision.decision);
  });
});
