import { afterEach, describe, expect, it, vi } from "vitest";

import worker, { safeEqual } from "../src/index";
import { readRates, runIngestion, perRegionBudget, type Env } from "../src/ingest";
import {
  backoffMs,
  matchDetail,
  patchFromGameVersion,
  regionalHost,
} from "../src/riot";

// ── Pure helpers ──────────────────────────────────────────────────────────────

describe("safeEqual (timing-safe secret compare)", () => {
  it("is true for equal strings, false otherwise", async () => {
    expect(await safeEqual("Bearer abc", "Bearer abc")).toBe(true);
    expect(await safeEqual("Bearer abc", "Bearer abd")).toBe(false);
  });

  it("is length-blind (different lengths never throw, return false)", async () => {
    expect(await safeEqual("", "Bearer secret")).toBe(false);
    expect(await safeEqual("short", "a much longer candidate value")).toBe(false);
  });
});

describe("backoffMs", () => {
  it("honours Retry-After seconds when present", () => {
    expect(backoffMs("3", 0)).toBe(3000);
    expect(backoffMs("120", 0)).toBe(30_000); // capped at 30s
  });

  it("falls back to exponential 0.5s·2^n capped at 8s", () => {
    expect(backoffMs(null, 0)).toBe(500);
    expect(backoffMs(null, 1)).toBe(1000);
    expect(backoffMs(null, 2)).toBe(2000);
    expect(backoffMs(null, 10)).toBe(8000); // capped
    expect(backoffMs("0", 1)).toBe(1000); // non-positive header ignored
  });
});

describe("perRegionBudget (KR starvation fix)", () => {
  it("splits the total fairly across regions, min 1 each", () => {
    expect(perRegionBudget(40, 4)).toBe(10);
    expect(perRegionBudget(8, 4)).toBe(2);
    expect(perRegionBudget(3, 4)).toBe(1); // ceil → every region gets ≥1
    expect(perRegionBudget(40, 0)).toBe(0); // no regions
  });
});

describe("pure riot helpers", () => {
  it("maps platform → regional routing host", () => {
    expect(regionalHost("kr")).toBe("asia.api.riotgames.com");
    expect(regionalHost("euw1")).toBe("europe.api.riotgames.com");
    expect(regionalHost("na1")).toBe("americas.api.riotgames.com");
  });
  it("buckets game version to patch", () => {
    expect(patchFromGameVersion("16.11.581.0000")).toBe("16.11");
    expect(patchFromGameVersion("16")).toBe("16");
  });
});

// ── riotGet backoff via the matchDetail seam ─────────────────────────────────

const okResp = (body: unknown) =>
  ({ ok: true, status: 200, headers: { get: () => null }, json: async () => body }) as unknown as Response;
const retryResp = (status: number, retryAfter?: string) =>
  ({
    ok: false,
    status,
    headers: { get: (h: string) => (h === "Retry-After" ? (retryAfter ?? null) : null) },
    json: async () => ({}),
  }) as unknown as Response;

describe("riotGet retry behaviour", () => {
  it("retries a 429 then succeeds, sleeping with the backoff", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce(retryResp(429, "2"))
      .mockResolvedValueOnce(okResp({ info: { gameVersion: "16.11.1.1", participants: [], teams: [] } }));
    const sleepImpl = vi.fn(async () => {});

    const dto = await matchDetail("euw1", "EUW1_1", "key", { fetchImpl, sleepImpl });

    expect(fetchImpl).toHaveBeenCalledTimes(2);
    expect(sleepImpl).toHaveBeenCalledTimes(1);
    expect(sleepImpl.mock.calls[0][0]).toBeGreaterThanOrEqual(2000); // Retry-After 2s + jitter
    expect(dto.info.gameVersion).toBe("16.11.1.1");
  });

  it("gives up after maxAttempts on persistent 500", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(retryResp(500));
    const sleepImpl = vi.fn(async () => {});
    await expect(
      matchDetail("kr", "KR_1", "key", { fetchImpl, sleepImpl, maxAttempts: 3 }),
    ).rejects.toThrow(/Riot 500/);
    expect(fetchImpl).toHaveBeenCalledTimes(3);
  });
});

// ── Worker HTTP surface (auth gate + error sanitization + method/route) ───────

const ctx = { waitUntil() {}, passThroughOnException() {} } as unknown as ExecutionContext;

function throwingDb(): D1Database {
  const stmt = {
    bind: () => stmt,
    first: async () => {
      throw new Error("SECRET internal SQL detail");
    },
    all: async () => {
      throw new Error("SECRET internal SQL detail");
    },
    run: async () => ({ success: true, meta: { changes: 1 } }),
  };
  return { prepare: () => stmt, batch: async () => [] } as unknown as D1Database;
}

const baseEnv = (over: Partial<Env> = {}): Env =>
  ({
    DB: throwingDb(),
    RIOT_API_KEY: "k",
    INGEST_REGIONS: "tr1,euw1,na1,kr",
    MAX_MATCHES_PER_RUN: "8",
    SEED_PLAYERS_PER_REGION: "2",
    MATCHES_PER_PLAYER: "2",
    ...over,
  }) as Env;

describe("worker HTTP surface", () => {
  it("rejects non-GET with 405", async () => {
    const res = await worker.fetch(new Request("https://w/v1/health", { method: "POST" }), baseEnv(), ctx);
    expect(res.status).toBe(405);
  });

  it("serves /v1/health", async () => {
    const res = await worker.fetch(new Request("https://w/v1/health"), baseEnv(), ctx);
    expect(res.status).toBe(200);
    expect(await res.json()).toEqual({ ok: true });
  });

  it("404s an unknown route (no /proxy passthrough)", async () => {
    const res = await worker.fetch(new Request("https://w/proxy/foo"), baseEnv(), ctx);
    expect(res.status).toBe(404);
  });

  it("sanitizes server errors — never echoes the raw exception", async () => {
    const res = await worker.fetch(new Request("https://w/v1/rates?region=tr1"), baseEnv(), ctx);
    expect(res.status).toBe(500);
    const body = (await res.json()) as { error: string };
    expect(body.error).toBe("rates failed");
    expect(body.error).not.toContain("SECRET");
  });

  it("gates /v1/ingest: 401 when secret unset or wrong", async () => {
    // No INGEST_SECRET configured → always 401.
    const noSecret = await worker.fetch(
      new Request("https://w/v1/ingest", { headers: { Authorization: "Bearer x" } }),
      baseEnv({ INGEST_SECRET: undefined }),
      ctx,
    );
    expect(noSecret.status).toBe(401);

    // Wrong secret → 401.
    const wrong = await worker.fetch(
      new Request("https://w/v1/ingest", { headers: { Authorization: "Bearer nope" } }),
      baseEnv({ INGEST_SECRET: "right" }),
      ctx,
    );
    expect(wrong.status).toBe(401);
  });
});

// ── Per-region fairness (KR no longer starved) ───────────────────────────────

function fairDb(): D1Database {
  const claimed = new Set<string>();
  const stmt = (sql: string, args: unknown[]) => ({
    async run() {
      if (sql.includes("INSERT OR IGNORE INTO processed_matches")) {
        const id = String(args[0]);
        if (claimed.has(id)) return { success: true, meta: { changes: 0 } };
        claimed.add(id);
        return { success: true, meta: { changes: 1 } };
      }
      if (sql.includes("DELETE FROM processed_matches")) {
        claimed.delete(String(args[0]));
        return { success: true, meta: { changes: 1 } };
      }
      return { success: true, meta: { changes: 1 } };
    },
    async first() {
      return null;
    },
    async all() {
      return { results: [] };
    },
  });
  return {
    prepare(sql: string) {
      return { bind: (...args: unknown[]) => stmt(sql, args) };
    },
    async batch() {
      return [];
    },
  } as unknown as D1Database;
}

describe("runIngestion per-region budget", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("gives the last region (kr) a fair share instead of starving it", async () => {
    let counter = 0;
    vi.stubGlobal("fetch", async (input: RequestInfo | URL) => {
      const u = String(input);
      if (u.includes("/challengerleagues/")) {
        return okResp({ entries: [{ puuid: "p1" }, { puuid: "p2" }] });
      }
      if (u.includes("/ids?")) {
        // Unique ids per region so each region has its own candidate pool.
        const region = u.split(".")[0].replace("https://", "");
        return okResp([`${region}_M${counter++}`, `${region}_M${counter++}`]);
      }
      // matchDetail
      return okResp({
        info: {
          gameVersion: "16.11.1.1",
          participants: [{ championId: 1, teamId: 100, teamPosition: "TOP", win: true }],
          teams: [{ bans: [] }],
        },
      });
    });

    const result = await runIngestion(baseEnv({ DB: fairDb() }));
    // total=8 over 4 regions → cap 2 each; kr must get a non-zero share.
    expect(result.perRegion.kr ?? 0).toBeGreaterThan(0);
    expect(result.processed).toBeLessThanOrEqual(8);
  });
});

// ── Patch resolution honours ingest recency, not lexical patch order ──────────

type MetaRow = { patch: string; region: string; total_games: number; updated_at: number };

/** Mock D1 that honours the ORDER BY of the patch-resolution query so the test
 *  distinguishes the fix (updated_at DESC) from the lexical bug (patch DESC). */
function metaDb(metaRows: MetaRow[]): D1Database {
  const prepare = (sql: string) => ({
    bind: (...args: unknown[]) => ({
      async first() {
        if (sql.includes("SELECT patch FROM ingest_meta")) {
          const sorted = [...metaRows].sort((a, b) =>
            sql.includes("ORDER BY updated_at DESC")
              ? b.updated_at - a.updated_at
              : a.patch < b.patch
                ? 1
                : a.patch > b.patch
                  ? -1
                  : 0, // leksik patch DESC = eski hatalı davranış
          );
          return sorted[0] ? { patch: sorted[0].patch } : null;
        }
        if (sql.includes("total_games")) {
          const row = metaRows.find((m) => m.patch === args[0] && m.region === args[1]);
          return row ? { total_games: row.total_games } : null;
        }
        return null;
      },
      async all() {
        return { results: [] };
      },
      async run() {
        return { success: true, meta: { changes: 0 } };
      },
    }),
  });
  return { prepare, batch: async () => [] } as unknown as D1Database;
}

describe("readRates patch resolution", () => {
  it("picks the freshest patch by ingest recency, not lexical order (16.9 < 16.10)", async () => {
    const env = baseEnv({
      DB: metaDb([
        { patch: "16.9", region: "tr1", total_games: 0, updated_at: 100 },
        { patch: "16.10", region: "tr1", total_games: 0, updated_at: 200 },
      ]),
    });
    const res = await readRates(env, "tr1");
    // "16.9" > "16.10" leksik olarak; ama 16.10 daha yeni ingest edildi → seçilmeli.
    expect(res.patch).toBe("16.10");
  });
});
