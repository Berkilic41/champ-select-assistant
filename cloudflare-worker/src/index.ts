import { runIngestion, readRates, type Env } from "./ingest";

export type { Env };

const ALLOWED_HOSTS = new Set([
  "tr1.api.riotgames.com",
  "euw1.api.riotgames.com",
  "eune1.api.riotgames.com",
  "na1.api.riotgames.com",
  "kr.api.riotgames.com",
  "europe.api.riotgames.com",
  "americas.api.riotgames.com",
  "asia.api.riotgames.com",
]);

const CORS = { "Access-Control-Allow-Origin": "*" };

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json", ...CORS },
  });
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
        return json({ error: String(e) }, 500);
      }
    }
    // Manual ingestion trigger (handy for `wrangler dev`; cron does this on schedule).
    if (path === "/v1/ingest") {
      try {
        return json(await runIngestion(env));
      } catch (e) {
        return json({ error: String(e) }, 500);
      }
    }

    // ── Per-user Riot API passthrough proxy: /proxy/{riot-host}/{riot-path} ──
    if (path.startsWith("/proxy/")) {
      const parts = path.slice("/proxy/".length).split("/");
      if (parts.length < 2) return new Response("Invalid path", { status: 400 });
      const riotHost = parts[0];
      if (!ALLOWED_HOSTS.has(riotHost)) {
        return new Response("Host not allowed", { status: 403 });
      }
      const riotPath = "/" + parts.slice(1).join("/");
      const riotResp = await fetch(`https://${riotHost}${riotPath}${url.search}`, {
        headers: { "X-Riot-Token": env.RIOT_API_KEY, Accept: "application/json" },
      });
      const headers = new Headers({ "Content-Type": "application/json", ...CORS });
      for (const h of ["X-App-Rate-Limit", "X-Method-Rate-Limit", "Retry-After"]) {
        const v = riotResp.headers.get(h);
        if (v) headers.set(h, v);
      }
      return new Response(riotResp.body, { status: riotResp.status, headers });
    }

    return new Response("Not found", { status: 404 });
  },

  // Cron-triggered ingestion (see [triggers] in wrangler.toml).
  async scheduled(_event: ScheduledController, env: Env, ctx: ExecutionContext): Promise<void> {
    ctx.waitUntil(runIngestion(env).then(() => undefined));
  },
};
