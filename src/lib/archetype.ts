/** KB archetype key → display label (LoL class names, used as-is in TR/EN). */
export const ARCHETYPE_LABELS: Record<string, string> = {
  juggernaut: 'Juggernaut',
  skirmisher: 'Skirmisher',
  diver: 'Diver',
  assassin: 'Assassin',
  burst_mage: 'Burst Mage',
  control_mage: 'Control Mage',
  battle_mage: 'Battle Mage',
  artillery: 'Artillery',
  marksman: 'Marksman',
  catcher: 'Catcher',
  enchanter: 'Enchanter',
  vanguard: 'Vanguard',
  warden: 'Warden',
};

export function archetypeLabel(a: string): string {
  return ARCHETYPE_LABELS[a] ?? a;
}
