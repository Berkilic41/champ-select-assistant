import { describe, it, expect, beforeEach } from 'vitest';
import {
  setDdragonVersion,
  applyDdragonVersion,
  getDdragonVersion,
  champIconUrl,
  itemIconUrl,
  splashUrl,
  summonerIconUrl,
  perkIconUrl,
  statShardLabel,
} from './ddragon';

describe('ddragon version', () => {
  beforeEach(() => setDdragonVersion('14.10.1'));

  it('get returns the current version and set updates it', () => {
    expect(getDdragonVersion()).toBe('14.10.1');
    setDdragonVersion('15.1.1');
    expect(getDdragonVersion()).toBe('15.1.1');
  });

  it('champ/item icon URLs embed the active version', () => {
    setDdragonVersion('15.1.1');
    expect(champIconUrl('Aatrox')).toBe(
      'https://ddragon.leagueoflegends.com/cdn/15.1.1/img/champion/Aatrox.png',
    );
    expect(itemIconUrl(3157)).toBe(
      'https://ddragon.leagueoflegends.com/cdn/15.1.1/img/item/3157.png',
    );
  });

  it('splash URL is version-independent', () => {
    setDdragonVersion('15.1.1');
    expect(splashUrl('Zed')).toBe(
      'https://ddragon.leagueoflegends.com/cdn/img/champion/loading/Zed_0.jpg',
    );
  });

  it('applyDdragonVersion applies a valid version but rejects the unknown sentinel + empty/null', () => {
    applyDdragonVersion('15.2.1');
    expect(getDdragonVersion()).toBe('15.2.1');
    applyDdragonVersion('unknown'); // sentinel → değişmez (ikon URL'i bozulmasın)
    expect(getDdragonVersion()).toBe('15.2.1');
    applyDdragonVersion(''); // boş → değişmez
    expect(getDdragonVersion()).toBe('15.2.1');
    applyDdragonVersion(null); // null → değişmez
    expect(getDdragonVersion()).toBe('15.2.1');
  });
});

describe('summonerIconUrl', () => {
  beforeEach(() => setDdragonVersion('14.10.1'));

  it('maps known spell IDs to DDragon spell keys', () => {
    expect(summonerIconUrl(4)).toContain('SummonerFlash.png');
    expect(summonerIconUrl(12)).toContain('SummonerTeleport.png');
    expect(summonerIconUrl(14)).toContain('SummonerDot.png'); // Ignite
  });

  it('returns null for an unknown spell ID', () => {
    expect(summonerIconUrl(999)).toBeNull();
  });
});

describe('perkIconUrl', () => {
  it('builds a CommunityDragon perk image URL', () => {
    expect(perkIconUrl(8112)).toBe(
      'https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/styles/8112.png',
    );
  });
});

describe('statShardLabel', () => {
  it('maps known shard IDs to short labels', () => {
    expect(statShardLabel(5001)).toBe('HP');
    expect(statShardLabel(5005)).toBe('Saldırı Hızı');
  });

  it('falls back to #id for unknown shards', () => {
    expect(statShardLabel(1234)).toBe('#1234');
  });
});
