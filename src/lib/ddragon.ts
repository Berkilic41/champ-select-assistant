let _version = '14.10.1';

export const setDdragonVersion = (v: string): void => {
  _version = v;
};

export const getDdragonVersion = (): string => _version;

/** Backend'den gelen DDragon sürümünü uygula; "unknown" sentinel'i (henüz sync
 *  olmadı) ve boş değeri reddet — geçerli fallback (14.10.1) korunur, ikon URL'i
 *  bozulmaz. App mount + summoner sync giriş yollarının ortak guard'ı. */
export const applyDdragonVersion = (v: string | null | undefined): void => {
  if (v && v !== 'unknown') _version = v;
};

export const champIconUrl = (key: string): string =>
  `https://ddragon.leagueoflegends.com/cdn/${_version}/img/champion/${key}.png`;

export const itemIconUrl = (id: number): string =>
  `https://ddragon.leagueoflegends.com/cdn/${_version}/img/item/${id}.png`;

export const splashUrl = (key: string): string =>
  `https://ddragon.leagueoflegends.com/cdn/img/champion/loading/${key}_0.jpg`;

/** Map LCU summoner spell IDs to DDragon icon file keys. */
const SUMMONER_KEY_BY_ID: Record<number, string> = {
  1: 'SummonerBoost',     // Cleanse
  3: 'SummonerExhaust',
  4: 'SummonerFlash',
  6: 'SummonerHaste',     // Ghost
  7: 'SummonerHeal',
  11: 'SummonerSmite',
  12: 'SummonerTeleport',
  13: 'SummonerMana',     // Clarity
  14: 'SummonerDot',      // Ignite
  21: 'SummonerBarrier',
  32: 'SummonerSnowball', // ARAM Mark
};

export const summonerIconUrl = (id: number): string | null => {
  const key = SUMMONER_KEY_BY_ID[id];
  if (!key) return null;
  return `https://ddragon.leagueoflegends.com/cdn/${_version}/img/spell/${key}.png`;
};

/** CommunityDragon perk (rune) icon URL — works for keystones, tier 2/3, and trees. */
export const perkIconUrl = (id: number): string =>
  `https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/styles/${id}.png`;

/** Map stat shard IDs to short Turkish labels (icon paths are unreliable). */
const STAT_SHARD_LABEL: Record<number, string> = {
  5001: 'HP',
  5002: 'Zırh',
  5003: 'MR',
  5005: 'Saldırı Hızı',
  5007: 'Yetenek Hızı',
  5008: 'Uyumlu',
};

export const statShardLabel = (id: number): string => STAT_SHARD_LABEL[id] ?? `#${id}`;
