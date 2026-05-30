# LCU Champ-Select Session — Fixture Notlari

> Sprint F0 bulgulari

## assigned_position Formati

LCU, `assignedPosition` alaninda su degerleri dondurüyor:
- `"top"`, `"jungle"`, `"middle"`, `"bottom"`, `"utility"` (tümü kücük harf)

`parse_session` bu degeri `as_str().unwrap_or("").to_string()` ile oldugu gibi sakliyor — herhangi bir donüsüm yok.

`scoring.rs:lane_counter_score()` — `my_pos.to_lowercase()` ile karsilastirma yapiyor: UYUMLU

LCU zaten kücük harf gonderiyor, `to_lowercase()` cagrisinin ekstra bir maliyeti yok ve formatlar eslesiyor.

## action_type Tespiti

`parse_session` logic:
- Tüm action gruplarini (dis dizi) ve icerisindeki action'lari iterate eder
- Oyuncunun aktif action'i: `actorCellId == my_cell_id && isInProgress == true && completed != true`
- Varsa `type` alani doner ("ban"/"pick")
- Yoksa "" (bos string) doner

Önemli: `completed != true` kontrolü, `completed` alani eksik olan action'lari da "aktif" sayar.
Gercek LCU verilerinde `completed` her zaman mevcut oldugu varsayilabilir.

## is_locked Mantigi

`is_locked = true` olmasi icin gereken sart:
- Herhangi bir action grubunda `type == "pick" && completed == true && actorCellId == slot.cell_id`

Bu her iki takim icin de geçerli (`my_team` ve `their_team`). Ban action'larinin tamamlanmasi `is_locked`'i degistirmez.

## Fixture Durumlari

| Fixture | action_type | phase | local is_locked | Notlar |
|---------|-------------|-------|-----------------|--------|
| ban_acting | "ban" | BAN_PICK | false | actorCellId=2, isInProgress=true, completed=false |
| pick_acting | "pick" | BAN_PICK | false | actorCellId=2, isInProgress=true, completed=false |
| pick_watching | "" | BAN_PICK | true | actorCellId=2 pick completed=true; aktif action yok |
| finalization | "" | FINALIZATION | true | tüm pick'ler completed=true; aktif action yok |

## Finalization Fazinda locked_count

`finalization.json` fixture'inda:
- `my_team` icinde cellId=0,2,3,4 icin completed pick action'lari var → 4 slot kilitli
- cellId=1 (Lee Sin) icin completed pick action yok → `is_locked=false`
- `locked_count >= 4` sarti saglaniyor

## Bilinen Davranislar

- `bans`: `myTeamBans` ve `theirTeamBans` dizileri oldugu gibi parse ediliyor, herhangi bir filtre yok
- `time_left_ms`: `adjustedTimeLeftInPhase` degerinden aliyor, yoksa 30_000 varsayilan
- `phase`: `timer.phase` degerinden aliyor, yoksa "PLANNING" varsayilan
- `local_player`: `my_team` icinde `cell_id == my_cell_id` olan slot; bulunamazsa `Default::default()` (tüm alanlar sifir/bos)

## Sonraki Sprint (E) icin Notlar

- `assigned_position` degerleri scoring'de dogrudan kullanilabilir — format kesinligi teyit edildi
- `lane_counter_score()` `to_lowercase()` kullaniyor, LCU zaten kücük harf gonderiyor — uyumlu
- `role_fit_score` icin `role_map[champion_id]` ile karsilastirma yapilacak; CDragon rolleri "fighter", "tank", "mage", "assassin", "support", "marksman" formatinda — `assignedPosition` ("middle", "top", vb.) ile ayni format DEGIL, mapping gerekecek
- Önerilen mapping: `"middle"` → `["mage", "assassin"]`, `"top"` → `["fighter", "tank"]`, `"jungle"` → `["fighter", "assassin", "tank"]`, `"bottom"` → `["marksman"]`, `"utility"` → `["support"]`
- Bu mapping yaklasik; kesin hesap icin CDragon champion metadata'sindaki `roles` alani kullanilmali
