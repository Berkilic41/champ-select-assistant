export type AppStatus =
  | { kind: 'disconnected'; error?: string }
  | { kind: 'connecting' }
  | { kind: 'lobby'; summonerName: string; port: number }
  | { kind: 'champ-select'; summonerName: string; port: number }
  | { kind: 'in-game'; summonerName: string };
