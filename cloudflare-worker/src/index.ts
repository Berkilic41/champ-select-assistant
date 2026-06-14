import { runIngestion, readRates, readMatchups, readBuilds, type Env } from "./ingest";

export type { Env };

const CORS = { "Access-Control-Allow-Origin": "*" };

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json", ...CORS },
  });
}

/** Log the real cause (visible in `wrangler tail`) but return a generic message —
 *  never echo `String(e)` to anonymous callers (could leak query/SQL internals). */
function serverError(label: string, e: unknown): Response {
  console.error(`${label} failed:`, e);
  return json({ error: `${label} failed` }, 500);
}

/** Constant-time string compare via SHA-256 digests (length-blind): both inputs
 *  collapse to a fixed 32-byte digest, so neither timing nor length leaks the
 *  secret. Avoids the early-exit short-circuit of `a !== b`. */
export async function safeEqual(a: string, b: string): Promise<boolean> {
  const enc = new TextEncoder();
  const [da, db] = await Promise.all([
    crypto.subtle.digest("SHA-256", enc.encode(a)),
    crypto.subtle.digest("SHA-256", enc.encode(b)),
  ]);
  const va = new Uint8Array(da);
  const vb = new Uint8Array(db);
  let diff = 0;
  for (let i = 0; i < va.length; i++) diff |= va[i] ^ vb[i];
  return diff === 0;
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    if (request.method !== "GET") {
      return new Response("Method not allowed", { status: 405 });
    }
    const url = new URL(request.url);
    const path = url.pathname;

    // ── Aggregated stats API (consumed by the desktop app) ──────────────────
    if (path === "/v1/health") {
      return json({ ok: true });
    }
    if (path === "/v1/rates") {
      const region = url.searchParams.get("region");
      if (!region) return json({ error: "region required" }, 400);
      const patch = url.searchParams.get("patch") ?? undefined;
      try {
        return json(await readRates(env, region, patch));
      } catch (e) {
        return serverError("rates", e);
      }
    }
    if (path === "/v1/matchups") {
      const region = url.searchParams.get("region");
      if (!region) return json({ error: "region required" }, 400);
      const patch = url.searchParams.get("patch") ?? undefined;
      try {
        return json(await readMatchups(env, region, patch));
      } catch (e) {
        return serverError("matchups", e);
      }
    }
    if (path === "/v1/builds") {
      const region = url.searchParams.get("region");
      if (!region) return json({ error: "region required" }, 400);
      const patch = url.searchParams.get("patch") ?? undefined;
      try {
        return json(await readBuilds(env, region, patch));
      } catch (e) {
        return serverError("builds", e);
      }
    }
    // Manual ingestion trigger — secret-gated so an anonymous caller can't drive
    // the personal Riot key or race the cron. Cron runs this on schedule without
    // HTTP. Requires `Authorization: Bearer <INGEST_SECRET>` (set via wrangler secret).
    if (path === "/v1/ingest") {
      const auth = request.headers.get("Authorization") ?? "";
      const expected = env.INGEST_SECRET ? `Bearer ${env.INGEST_SECRET}` : "";
      if (!env.INGEST_SECRET || !(await safeEqual(auth, expected))) {
        return json({ error: "unauthorized" }, 401);
      }
      try {
        return json(await runIngestion(env));
      } catch (e) {
        return serverError("ingest", e);
      }
    }

    // NOTE: the per-user Riot API passthrough (/proxy/*) was removed — it exposed
    // the personal/dev key to anonymous abuse. The app is LCU-first; a Riot proxy
    // returns (auth-gated) only after a production key is approved.
    return new Response("Not found", { status: 404 });
  },

  // Cron-triggered ingestion (see [triggers] in wrangler.toml).
  async scheduled(_event: ScheduledController, env: Env, ctx: ExecutionContext): Promise<void> {
    ctx.waitUntil(runIngestion(env).then(() => undefined));
  },
};
