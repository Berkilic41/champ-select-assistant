/**
 * Maç geçmişi listesi satırı (Epic Slice 1). Host `get_match_history` komutunun
 * döndürdüğü şekil — saf yerel DB sorgusu (matches + champions JOIN + game_reviews
 * EXISTS), core/ts-rs DEĞİL; bu yüzden elle tanımlı (krş. RankCard'ın RankedStat'i).
 * Alan adları DB ile birebir (snake_case). `played_at` = Unix SANİYE.
 */
export interface MatchHistoryEntry {
  match_id: string;
  champion_id: number;
  champion_key: string;
  position: string | null;
  queue_id: number;
  win: number; // SQLite integer: 0 | 1
  kills: number;
  deaths: number;
  assists: number;
  duration_secs: number;
  played_at: number; // Unix saniye
  cs: number | null; // V018 öncesi maçlarda null
  cs_at_10: number | null; // V020 öncesi / timeline yoksa null
  deaths_pre_14: number | null;
  vision_score: number | null;
  has_review: number; // 0 | 1 — game_reviews karnesi var mı (Slice 2 detay paneli)
}
