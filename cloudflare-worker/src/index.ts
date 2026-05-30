export interface Env {
  RIOT_API_KEY: string;
}

const ALLOWED_HOSTS = new Set([
  'tr1.api.riotgames.com',
  'euw1.api.riotgames.com',
  'eune1.api.riotgames.com',
  'na1.api.riotgames.com',
  'europe.api.riotgames.com',
  'americas.api.riotgames.com',
  'asia.api.riotgames.com',
]);

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (request.method !== 'GET') {
      return new Response('Method not allowed', { status: 405 });
    }

    const url = new URL(request.url);
    // Path format: /{riot-host}/{riot-path}
    // e.g. /tr1.api.riotgames.com/lol/summoner/v4/by-puuid/...
    const pathParts = url.pathname.slice(1).split('/');
    if (pathParts.length < 2) {
      return new Response('Invalid path', { status: 400 });
    }

    const riotHost = pathParts[0];
    if (!ALLOWED_HOSTS.has(riotHost)) {
      return new Response('Host not allowed', { status: 403 });
    }

    const riotPath = '/' + pathParts.slice(1).join('/');
    const riotUrl = `https://${riotHost}${riotPath}${url.search}`;

    const riotResp = await fetch(riotUrl, {
      headers: {
        'X-Riot-Token': env.RIOT_API_KEY,
        'Accept': 'application/json',
      },
    });

    const headers = new Headers({
      'Content-Type': 'application/json',
      'Access-Control-Allow-Origin': '*',
    });

    // Pass through rate limit headers for client-side awareness
    for (const h of ['X-App-Rate-Limit', 'X-Method-Rate-Limit', 'Retry-After']) {
      const v = riotResp.headers.get(h);
      if (v) headers.set(h, v);
    }

    return new Response(riotResp.body, {
      status: riotResp.status,
      headers,
    });
  },
};
