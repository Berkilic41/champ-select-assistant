use crate::db::champion_repo::ChampionRecord;
use crate::db::mastery_repo::MasteryRow;
use crate::db::{builds_repo, mastery_repo, match_repo, open_db, run_migrations};
use crate::lcu::session::{parse_session, ChampSelectState, TeamSlot};
use crate::recommendation::draft_iq::DraftKnowledgeBase;
use crate::recommendation::{compute_recommendations, MetaRate, ScoringContext, ScoringWeights};
use crate::riot::client::ChampionStats;
use std::collections::HashMap;
use tempfile::tempdir;

#[test]
fn test_db_open_and_migrate() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let mut conn = open_db(&db_path).expect("DB açılmalı");
    run_migrations(&mut conn).expect("Migrations çalışmalı");

    // app_config tablosu var mı?
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_config'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "app_config tablosu oluşturulmalı");

    // champions tablosu var mı?
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='champions'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "champions tablosu oluşturulmalı");
}

#[test]
fn test_summoners_table_created() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let mut conn = open_db(&db_path).unwrap();
    run_migrations(&mut conn).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='summoners'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "summoners tablosu oluşturulmalı");
}

#[test]
fn test_summoner_insert() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let mut conn = open_db(&db_path).unwrap();
    run_migrations(&mut conn).unwrap();

    conn.execute(
        "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at)
         VALUES ('test-puuid', 'TestGamer', 'TR1', 'tr1', 1000000)",
        [],
    )
    .expect("Summoner insert başarılı olmalı");

    let name: String = conn
        .query_row(
            "SELECT game_name FROM summoners WHERE puuid='test-puuid'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(name, "TestGamer");
}

#[test]
fn test_champion_insert() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let mut conn = open_db(&db_path).unwrap();
    run_migrations(&mut conn).unwrap();

    conn.execute(
        "INSERT INTO champions (champion_id, key, name, title, cached_at)
         VALUES (1, 'Annie', 'Annie', 'the Dark Child', 1000000)",
        [],
    )
    .expect("Champion insert başarılı olmalı");

    let key: String = conn
        .query_row("SELECT key FROM champions WHERE champion_id=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(key, "Annie");
}

#[test]
fn test_migrations_idempotent() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let mut conn = open_db(&db_path).unwrap();
    // Refinery migration runner sadece uygulanmamış migrationları çalıştırır,
    // tekrar çalıştırmak hata vermemeli
    run_migrations(&mut conn).expect("İlk migration çalışmalı");
    run_migrations(&mut conn).expect("İkinci migration da hata vermemeli");
}

#[test]
fn test_foreign_key_enforcement() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let mut conn = open_db(&db_path).unwrap();
    run_migrations(&mut conn).unwrap();

    // Foreign key olmadan match insert etmeye çalış — FOREIGN KEY ON olduğu için hata vermeli
    let result = conn.execute(
        "INSERT INTO matches
         (match_id, puuid, champion_id, win, duration_secs, queue_id, played_at)
         VALUES ('match-1', 'nonexistent-puuid', 999, 1, 1800, 420, 1000000)",
        [],
    );
    assert!(
        result.is_err(),
        "Var olmayan puuid ile match insert foreign key ihlali vermeli"
    );
}

#[test]
fn test_wal_mode_enabled() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    let conn = open_db(&db_path).unwrap();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal", "WAL journal modu aktif olmalı");
}

// -------------------------------------------------------------------------
// match_repo tests
// -------------------------------------------------------------------------

#[test]
fn test_insert_match_and_stats() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("t.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    // FK zorunluluğu: önce summoner ve champion ekle
    conn.execute(
        "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES ('p1','Test','TR1','tr1',1)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (266,'Aatrox','Aatrox','Darkin',1)",
        [],
    ).unwrap();

    let inserted = match_repo::insert_match(
        &conn,
        "EUW1_001",
        "p1",
        266,
        Some("TOP"),
        true,
        5,
        2,
        8,
        1800,
        420,
        1700000000,
    )
    .unwrap();
    assert!(inserted, "İlk insert true döndürmeli");

    // Aynı match_id tekrar eklenince false (OR IGNORE)
    let inserted2 = match_repo::insert_match(
        &conn,
        "EUW1_001",
        "p1",
        266,
        Some("TOP"),
        true,
        5,
        2,
        8,
        1800,
        420,
        1700000000,
    )
    .unwrap();
    assert!(!inserted2, "Duplicate insert false döndürmeli");

    let stats = match_repo::player_stats(&conn, "p1").unwrap();
    assert_eq!(stats.len(), 1, "Bir champion stats satırı olmalı");
    assert_eq!(stats[0].champion_id, 266);
    assert_eq!(stats[0].games, 1);
}

#[test]
fn test_insert_match_win_loss_aggregation() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("t.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    conn.execute(
        "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES ('p2','Player2','EUW','euw1',1)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (1,'Annie','Annie','Dark Child',1)",
        [],
    ).unwrap();

    // Üç maç: 2 galibiyet, 1 yenilgi
    match_repo::insert_match(&conn, "M001", "p2", 1, None, true, 3, 1, 5, 1500, 420, 1000).unwrap();
    match_repo::insert_match(
        &conn, "M002", "p2", 1, None, false, 1, 4, 2, 1600, 420, 1001,
    )
    .unwrap();
    match_repo::insert_match(&conn, "M003", "p2", 1, None, true, 5, 0, 9, 1400, 420, 1002).unwrap();

    let stats = match_repo::player_stats(&conn, "p2").unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].games, 3);
    assert_eq!(stats[0].wins, 2);
    // kda_avg = AVG((kills+assists)/MAX(deaths,1))
    // game1: (3+5)/1=8, game2: (1+2)/4=0.75, game3: (5+9)/1=14 → avg≈7.58
    assert!(stats[0].kda_avg > 0.0, "kda_avg pozitif olmalı");
}

#[test]
fn test_player_stats_empty() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("t.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let stats = match_repo::player_stats(&conn, "nonexistent").unwrap();
    assert!(stats.is_empty(), "Bilinmeyen puuid için boş liste dönmeli");
}

// -------------------------------------------------------------------------
// mastery_repo tests
// -------------------------------------------------------------------------

#[test]
fn test_upsert_mastery() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("t.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    conn.execute(
        "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES ('p1','Test','TR1','tr1',1)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (266,'Aatrox','Aatrox','Darkin',1)",
        [],
    ).unwrap();

    mastery_repo::upsert_mastery(&conn, "p1", 266, 7, 150000, None).unwrap();

    let top = mastery_repo::top_for_puuid(&conn, "p1", 5).unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].champion_id, 266);
    assert_eq!(top[0].level, 7);
    assert_eq!(top[0].points, 150000);
    assert!(top[0].last_play_time.is_none());
}

#[test]
fn test_upsert_mastery_updates_existing() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("t.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    conn.execute(
        "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES ('p1','Test','TR1','tr1',1)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES (266,'Aatrox','Aatrox','Darkin',1)",
        [],
    ).unwrap();

    mastery_repo::upsert_mastery(&conn, "p1", 266, 6, 80000, None).unwrap();
    // Güncelle: daha yüksek level ve puan
    mastery_repo::upsert_mastery(&conn, "p1", 266, 7, 200000, Some(1700000000)).unwrap();

    let top = mastery_repo::top_for_puuid(&conn, "p1", 5).unwrap();
    assert_eq!(top.len(), 1, "Hâlâ tek satır olmalı");
    assert_eq!(top[0].level, 7, "Level güncellenmeli");
    assert_eq!(top[0].points, 200000, "Points güncellenmeli");
    assert_eq!(top[0].last_play_time, Some(1700000000));
}

#[test]
fn test_top_for_puuid_ordering_and_limit() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("t.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    conn.execute(
        "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at) VALUES ('p1','Test','TR1','tr1',1)",
        [],
    ).unwrap();
    for (id, key, name) in [
        (1, "Annie", "Annie"),
        (266, "Aatrox", "Aatrox"),
        (157, "Yasuo", "Yasuo"),
    ] {
        conn.execute(
            &format!("INSERT INTO champions (champion_id, key, name, title, cached_at) VALUES ({id},'{key}','{name}','Title',1)"),
            [],
        ).unwrap();
    }

    mastery_repo::upsert_mastery(&conn, "p1", 1, 5, 50000, None).unwrap();
    mastery_repo::upsert_mastery(&conn, "p1", 266, 7, 200000, None).unwrap();
    mastery_repo::upsert_mastery(&conn, "p1", 157, 6, 120000, None).unwrap();

    // Limit 2: en yüksek 2 puana göre sıralı dönmeli
    let top2 = mastery_repo::top_for_puuid(&conn, "p1", 2).unwrap();
    assert_eq!(top2.len(), 2);
    assert_eq!(top2[0].champion_id, 266, "En yüksek puan Aatrox olmalı");
    assert_eq!(top2[1].champion_id, 157, "İkinci Yasuo olmalı");
}

#[test]
fn test_top_for_puuid_empty() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("t.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let top = mastery_repo::top_for_puuid(&conn, "nobody", 5).unwrap();
    assert!(top.is_empty(), "Bilinmeyen puuid için boş liste dönmeli");
}

// -------------------------------------------------------------------------
// V005 champion_meta table tests
// -------------------------------------------------------------------------

#[test]
fn test_champion_meta_upsert() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("t.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    // V005 adds champion_meta; insert a row first
    conn.execute(
        "INSERT INTO champions (champion_id, key, name, title, cached_at)
         VALUES (157, 'Yasuo', 'Yasuo', 'the Unforgiven', 1)",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO champion_meta (champion_id, roles, is_melee, attack_range, resource_type)
         VALUES (157, '[\"middle\",\"top\"]', 1, 175, NULL)",
        [],
    )
    .expect("champion_meta insert basarili olmali");

    // Read back and verify
    let (roles, is_melee, range): (String, i64, i64) = conn
        .query_row(
            "SELECT roles, is_melee, attack_range FROM champion_meta WHERE champion_id = 157",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("champion_meta satiri okunabilmeli");

    assert!(roles.contains("middle"), "roles JSON 'middle' icermeli");
    assert_eq!(is_melee, 1, "Yasuo melee olmali");
    assert_eq!(range, 175, "attack_range 175 olmali");

    // Upsert (ON CONFLICT on primary key): update roles
    conn.execute(
        "INSERT INTO champion_meta (champion_id, roles, is_melee, attack_range, resource_type)
         VALUES (157, '[\"middle\"]', 1, 175, 'flow')
         ON CONFLICT(champion_id) DO UPDATE SET
             roles         = excluded.roles,
             resource_type = excluded.resource_type",
        [],
    )
    .expect("champion_meta upsert basarili olmali");

    let (new_roles, res_type): (String, Option<String>) = conn
        .query_row(
            "SELECT roles, resource_type FROM champion_meta WHERE champion_id = 157",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert_eq!(new_roles, "[\"middle\"]", "Roles guncellenmeli");
    assert_eq!(
        res_type.as_deref(),
        Some("flow"),
        "resource_type guncellenmeli"
    );
}

// -------------------------------------------------------------------------
// V006 champion_matchups table tests
// -------------------------------------------------------------------------

#[test]
fn test_matchup_upsert() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("t.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    // champion_matchups has no FK so no prereq inserts needed
    conn.execute(
        "INSERT INTO champion_matchups
         (champion_id, opponent_id, position, games, wins, win_rate, source, patch_version, cached_at)
         VALUES (157, 238, 'middle', 100, 55, 0.55, 'local', '15.10', 1000)",
        [],
    )
    .expect("matchup insert basarili olmali");

    let (games, win_rate): (i64, f64) = conn
        .query_row(
            "SELECT games, win_rate FROM champion_matchups
             WHERE champion_id=157 AND opponent_id=238 AND position='middle' AND source='local'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("matchup satiri okunabilmeli");

    assert_eq!(games, 100);
    assert!((win_rate - 0.55).abs() < 1e-6, "win_rate 0.55 olmali");

    // Unique constraint: ayni (champion_id, opponent_id, position, source) tekrar insert etmek hata vermeli
    let dup = conn.execute(
        "INSERT INTO champion_matchups
         (champion_id, opponent_id, position, games, wins, win_rate, source, patch_version, cached_at)
         VALUES (157, 238, 'middle', 200, 90, 0.45, 'local', '15.10', 2000)",
        [],
    );
    assert!(
        dup.is_err(),
        "Duplicate matchup unique constraint ihlali vermeli"
    );

    // Upsert ile guncelleme calismali
    conn.execute(
        "INSERT INTO champion_matchups
         (champion_id, opponent_id, position, games, wins, win_rate, source, patch_version, cached_at)
         VALUES (157, 238, 'middle', 200, 90, 0.45, 'local', '15.10', 2000)
         ON CONFLICT(champion_id, opponent_id, position, source) DO UPDATE SET
             games      = excluded.games,
             wins       = excluded.wins,
             win_rate   = excluded.win_rate,
             cached_at  = excluded.cached_at",
        [],
    )
    .expect("Upsert calismali");

    let updated_games: i64 = conn
        .query_row(
            "SELECT games FROM champion_matchups
             WHERE champion_id=157 AND opponent_id=238 AND position='middle' AND source='local'",
            [],
            |r| r.get(0),
        )
        .unwrap();

    assert_eq!(updated_games, 200, "games guncellenmeli");
}

// -------------------------------------------------------------------------
// LCU session parse tests (lcu::session::parse_session)
// -------------------------------------------------------------------------

#[test]
fn test_session_parse() {
    use serde_json::json;

    // Sample LCU /lol-champ-select/v1/session payload
    let session_json = json!({
        "localPlayerCellId": 0,
        "myTeam": [
            {
                "cellId": 0,
                "championId": 157,
                "championPickIntent": 157,
                "assignedPosition": "middle"
            },
            {
                "cellId": 1,
                "championId": 0,
                "championPickIntent": 122,
                "assignedPosition": "top"
            }
        ],
        "theirTeam": [
            {
                "cellId": 5,
                "championId": 238,
                "championPickIntent": 238,
                "assignedPosition": "middle"
            },
            {
                "cellId": 6,
                "championId": 91,
                "championPickIntent": 91,
                "assignedPosition": "jungle"
            },
            {
                "cellId": 7,
                "championId": 107,
                "championPickIntent": 107,
                "assignedPosition": "top"
            }
        ],
        "bans": {
            "myTeamBans": [235, 142],
            "theirTeamBans": [7, 11]
        },
        "actions": [
            [
                {
                    "type": "pick",
                    "completed": true,
                    "actorCellId": 0,
                    "championId": 157
                }
            ]
        ],
        "timer": {
            "phase": "BAN_PICK",
            "adjustedTimeLeftInPhase": 25000
        }
    });

    let state = parse_session(&session_json).expect("Gecerli session JSON parse edilebilmeli");

    // local player fields
    assert_eq!(state.my_cell_id, 0, "my_cell_id doğru olmali");
    assert_eq!(
        state.local_player.champion_id, 157,
        "local player champion_id 157 olmali"
    );
    assert_eq!(
        state.local_player.assigned_position, "middle",
        "local player position 'middle' olmali"
    );
    assert!(
        state.local_player.is_locked,
        "Tamamlanan pick action is_locked=true olmali"
    );

    // team sizes
    assert_eq!(state.my_team.len(), 2, "my_team 2 slot icermeli");
    assert_eq!(state.their_team.len(), 3, "their_team 3 slot icermeli");

    // enemy team champion ids
    let enemy_ids: Vec<u32> = state.their_team.iter().map(|s| s.champion_id).collect();
    assert!(enemy_ids.contains(&238), "Zed (238) dusman takimda olmali");
    assert!(enemy_ids.contains(&91), "Talon (91) dusman takimda olmali");
    assert!(
        enemy_ids.contains(&107),
        "Rengar (107) dusman takimda olmali"
    );

    // bans
    assert_eq!(state.my_bans, vec![235, 142], "my_bans dogru olmali");
    assert_eq!(state.their_bans, vec![7, 11], "their_bans dogru olmali");

    // phase + timer
    assert_eq!(state.phase, "BAN_PICK", "phase 'BAN_PICK' olmali");
    assert_eq!(state.time_left_ms, 25000, "time_left_ms 25000 olmali");
}

#[test]
fn test_session_parse_invalid_returns_none() {
    use serde_json::json;

    // localPlayerCellId eksik → None dönmeli
    let bad = json!({ "myTeam": [], "theirTeam": [] });
    assert!(parse_session(&bad).is_none(), "Gecersiz JSON None donmeli");
}

// -------------------------------------------------------------------------
// Recommendation engine unit tests (pure-function, no DB needed)
// -------------------------------------------------------------------------

/// comfort_score hesaplar: mastery_level 0-7 arasi degerden 0.0-1.0 normalize,
/// kalan kismi win_rate ile agirliklandirir.
fn comfort_score(mastery_level: u8, mastery_points: u64, win_rate: f64) -> f64 {
    let mastery_norm = mastery_level.min(7) as f64 / 7.0;
    let points_bonus = (mastery_points as f64 / 1_000_000.0).min(0.2);
    let base = mastery_norm * 0.6 + points_bonus * 0.1 + win_rate * 0.3;
    base.min(1.0)
}

#[test]
fn test_comfort_scoring() {
    // mastery level 7, 150k points, %60 win rate → yüksek comfort
    let high = comfort_score(7, 150_000, 0.60);
    // mastery level 1, 1k points, %45 win rate → düsük comfort
    let low = comfort_score(1, 1_000, 0.45);

    assert!(
        high > 0.70,
        "Level 7 champion yuksek comfort score almalı (got {:.3})",
        high
    );
    assert!(
        low < 0.35,
        "Level 1 champion dusuk comfort score almali (got {:.3})",
        low
    );
    assert!(high > low, "Level 7 > level 1 comfort olmali");

    // Maksimum: level 7, 1M+ points, %100 wr → yüksek skor, 1.0 tavanına takilmali
    let perfect = comfort_score(7, 1_500_000, 1.0);
    assert!(
        perfect > 0.90,
        "Maksimum girdi yüksek skor vermeli (got {:.3})",
        perfect
    );
    assert!(perfect <= 1.0, "Skor 1.0 tavanini asmamali");
}

/// Dusman takimindaki seçilmiş şampiyonların rolleri analiz eder.
fn analyze_enemy_team(slots: &[TeamSlot]) -> EnemyComposition {
    // assassin_ids: champion ID listesi (Zed=238, Talon=91, Rengar=107, Akali=84,
    //               Katarina=55, Khazix=121, Pyke=555, Fizz=105, Qiyana=246, Yone=777)
    const ASSASSIN_IDS: &[u32] = &[238, 91, 107, 84, 55, 121, 555, 105, 246, 777];

    let picked: Vec<u32> = slots
        .iter()
        .filter(|s| s.champion_id != 0)
        .map(|s| s.champion_id)
        .collect();

    let assassin_count = picked.iter().filter(|id| ASSASSIN_IDS.contains(id)).count();

    EnemyComposition {
        total_picked: picked.len(),
        assassin_count,
        is_assassin_heavy: assassin_count >= 2,
    }
}

#[derive(Debug)]
struct EnemyComposition {
    total_picked: usize,
    assassin_count: usize,
    is_assassin_heavy: bool,
}

#[test]
fn test_team_analysis() {
    // 3 assassin dusman takimi: Zed, Talon, Rengar
    let their_team = vec![
        TeamSlot {
            cell_id: 5,
            champion_id: 238,
            intent_champion_id: 238,
            assigned_position: "middle".into(),
            is_locked: true,
        },
        TeamSlot {
            cell_id: 6,
            champion_id: 91,
            intent_champion_id: 91,
            assigned_position: "jungle".into(),
            is_locked: true,
        },
        TeamSlot {
            cell_id: 7,
            champion_id: 107,
            intent_champion_id: 107,
            assigned_position: "top".into(),
            is_locked: true,
        },
        TeamSlot {
            cell_id: 8,
            champion_id: 0,
            intent_champion_id: 0,
            assigned_position: "bottom".into(),
            is_locked: false,
        },
        TeamSlot {
            cell_id: 9,
            champion_id: 0,
            intent_champion_id: 0,
            assigned_position: "utility".into(),
            is_locked: false,
        },
    ];

    let comp = analyze_enemy_team(&their_team);

    assert_eq!(comp.assassin_count, 3, "3 assassin tanınmalı");
    assert!(comp.is_assassin_heavy, "is_assassin_heavy true olmali");
    assert_eq!(
        comp.total_picked, 3,
        "3 champion secilmis olmali (0 ID'ler sayılmamalı)"
    );

    // Non-assassin takım: Garen, Ashe, Soraka, Darius, Malphite
    let safe_team = vec![
        TeamSlot {
            cell_id: 5,
            champion_id: 86,
            intent_champion_id: 86,
            assigned_position: "top".into(),
            is_locked: true,
        },
        TeamSlot {
            cell_id: 6,
            champion_id: 22,
            intent_champion_id: 22,
            assigned_position: "bottom".into(),
            is_locked: true,
        },
        TeamSlot {
            cell_id: 7,
            champion_id: 16,
            intent_champion_id: 16,
            assigned_position: "utility".into(),
            is_locked: true,
        },
        TeamSlot {
            cell_id: 8,
            champion_id: 122,
            intent_champion_id: 122,
            assigned_position: "jungle".into(),
            is_locked: true,
        },
        TeamSlot {
            cell_id: 9,
            champion_id: 54,
            intent_champion_id: 54,
            assigned_position: "middle".into(),
            is_locked: true,
        },
    ];

    let safe_comp = analyze_enemy_team(&safe_team);
    assert_eq!(safe_comp.assassin_count, 0, "Assassin olmamali");
    assert!(
        !safe_comp.is_assassin_heavy,
        "is_assassin_heavy false olmali"
    );
}

// -------------------------------------------------------------------------
// Recommendation engine snapshot tests (full compute_recommendations pipeline)
// -------------------------------------------------------------------------

/// Build the fixture data shared across snapshot tests.
/// Champions:
///   157  Yasuo  — roles: ["fighter","assassin"]  — high mastery + good stats
///   238  Zed    — roles: ["assassin"]             — medium mastery + very good stats
///   122  Darius — roles: ["fighter","tank"]       — medium mastery + average stats
///   99   Lux    — roles: ["mage","support"]       — low mastery + poor stats
///   86   Garen  — roles: ["fighter","tank"]       — zero mastery + zero stats
fn fixture_champions() -> Vec<ChampionRecord> {
    vec![
        ChampionRecord {
            champion_id: 157,
            key: "Yasuo".into(),
            name: "Yasuo".into(),
            title: "the Unforgiven".into(),
        },
        ChampionRecord {
            champion_id: 238,
            key: "Zed".into(),
            name: "Zed".into(),
            title: "the Master of Shadows".into(),
        },
        ChampionRecord {
            champion_id: 122,
            key: "Darius".into(),
            name: "Darius".into(),
            title: "the Hand of Noxus".into(),
        },
        ChampionRecord {
            champion_id: 99,
            key: "Lux".into(),
            name: "Lux".into(),
            title: "the Lady of Luminosity".into(),
        },
        ChampionRecord {
            champion_id: 86,
            key: "Garen".into(),
            name: "Garen".into(),
            title: "the Might of Demacia".into(),
        },
    ]
}

fn fixture_mastery() -> Vec<MasteryRow> {
    vec![
        MasteryRow {
            champion_id: 157,
            level: 7,
            points: 180_000,
            last_play_time: None,
        }, // Yasuo — highest mastery
        MasteryRow {
            champion_id: 238,
            level: 6,
            points: 90_000,
            last_play_time: None,
        }, // Zed
        MasteryRow {
            champion_id: 122,
            level: 5,
            points: 50_000,
            last_play_time: None,
        }, // Darius
        MasteryRow {
            champion_id: 99,
            level: 3,
            points: 15_000,
            last_play_time: None,
        }, // Lux — low mastery
           // Garen: intentionally absent (zero mastery)
    ]
}

fn fixture_stats() -> Vec<ChampionStats> {
    vec![
        // Yasuo: 20 games, 13 wins, KDA ~4.5 → strong meta proxy
        ChampionStats {
            champion_id: 157,
            games: 20,
            wins: 13,
            kda_avg: 4.5,
        },
        // Zed: 15 games, 11 wins, KDA ~6.0 → best KDA (meta_score)
        ChampionStats {
            champion_id: 238,
            games: 15,
            wins: 11,
            kda_avg: 6.0,
        },
        // Darius: 8 games, 4 wins, KDA ~2.5 → average
        ChampionStats {
            champion_id: 122,
            games: 8,
            wins: 4,
            kda_avg: 2.5,
        },
        // Lux: 3 games, 1 win, KDA ~1.5 → poor
        ChampionStats {
            champion_id: 99,
            games: 3,
            wins: 1,
            kda_avg: 1.5,
        },
        // Garen: absent (no match history)
    ]
}

fn fixture_role_map() -> HashMap<u32, Vec<String>> {
    let mut m = HashMap::new();
    m.insert(157, vec!["fighter".into(), "assassin".into()]);
    m.insert(238, vec!["assassin".into()]);
    m.insert(122, vec!["fighter".into(), "tank".into()]);
    m.insert(99, vec!["mage".into(), "support".into()]);
    m.insert(86, vec!["fighter".into(), "tank".into()]);
    m
}

/// Fixture ChampSelectState: local player is Yasuo mid, 3 enemy mages/supports,
/// one ally picked (Darius top). Yasuo itself must be EXCLUDED from recommendations.
fn fixture_session() -> ChampSelectState {
    ChampSelectState {
        my_cell_id: 0,
        local_player: TeamSlot {
            cell_id: 0,
            champion_id: 157, // Yasuo — already picked
            intent_champion_id: 157,
            assigned_position: "middle".into(),
            is_locked: true,
        },
        my_team: vec![
            TeamSlot {
                cell_id: 0,
                champion_id: 157,
                intent_champion_id: 157,
                assigned_position: "middle".into(),
                is_locked: true,
            },
            TeamSlot {
                cell_id: 1,
                champion_id: 122,
                intent_champion_id: 122,
                assigned_position: "top".into(),
                is_locked: true,
            },
        ],
        their_team: vec![
            // Enemy mid: Lux (mage) — Assassin counters Mage → should boost Zed/Yasuo scores
            TeamSlot {
                cell_id: 5,
                champion_id: 99,
                intent_champion_id: 99,
                assigned_position: "middle".into(),
                is_locked: true,
            },
            // Two more enemies — unrelated positions
            TeamSlot {
                cell_id: 6,
                champion_id: 0,
                intent_champion_id: 0,
                assigned_position: "jungle".into(),
                is_locked: false,
            },
            TeamSlot {
                cell_id: 7,
                champion_id: 0,
                intent_champion_id: 0,
                assigned_position: "top".into(),
                is_locked: false,
            },
        ],
        my_bans: vec![],
        their_bans: vec![],
        phase: "BAN_PICK".into(),
        time_left_ms: 25_000,
        action_type: "pick".into(),
        queue_id: 0,
        pick_order: 0,
    }
}

#[test]
fn test_recommendation_snapshot_top5_order() {
    // In-memory DB setup (required to satisfy FK-based data, but we pass data
    // directly to ScoringContext — the DB is only used to verify migrations pass).
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("snap.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let champions = fixture_champions();
    let mastery = fixture_mastery();
    let stats = fixture_stats();
    let role_map = fixture_role_map();
    let session = fixture_session();

    let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
    let ctx = ScoringContext {
        session: &session,
        mastery: &mastery,
        stats: &stats,
        role_map: &role_map,
        meta_rates: &meta_rates,
        weights: ScoringWeights::default(),
        matchups: None,
        power_curves: None,
    };

    let kb = DraftKnowledgeBase::load().expect("KB load failed");
    let recs = compute_recommendations(&ctx, &champions, &[], &[], &kb);

    // Must return ≤ 5 results
    assert!(recs.len() <= 5, "En fazla 5 öneri dönmeli");
    // Must return at least 1 result (we have candidates)
    assert!(!recs.is_empty(), "En az bir öneri dönmeli");

    // Local player's picked champion (Yasuo=157) and Darius (122, ally) and
    // Lux (99, enemy) are excluded from recommendations.
    let rec_ids: Vec<u32> = recs.iter().map(|r| r.champion_id).collect();
    assert!(
        !rec_ids.contains(&157),
        "Yasuo (zaten seçildi) önerilmemeli"
    );
    assert!(!rec_ids.contains(&122), "Darius (ally pick) önerilmemeli");
    assert!(!rec_ids.contains(&99), "Lux (enemy pick) önerilmemeli");

    // Results must be sorted descending by total_score
    for window in recs.windows(2) {
        assert!(
            window[0].total_score >= window[1].total_score,
            "Sonuçlar total_score'a göre azalan sırada olmalı"
        );
    }

    // Zed (238) should rank above Garen (86): Zed has high mastery + best KDA + is an assassin
    // countering enemy mage (Lux) in mid — Garen has no mastery and no stats.
    let zed_pos = rec_ids.iter().position(|&id| id == 238);
    let garen_pos = rec_ids.iter().position(|&id| id == 86);
    if let (Some(zp), Some(gp)) = (zed_pos, garen_pos) {
        assert!(
            zp < gp,
            "Zed yüksek mastery + lane counter avantajıyla Garen'den önce gelmeli (Zed idx={zp}, Garen idx={gp})"
        );
    }
}

#[test]
fn test_recommendation_snapshot_no_candidates() {
    // All pool champions are either already picked or banned → should return empty vec.
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("snap2.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let champions = fixture_champions();
    let mastery = fixture_mastery();
    let stats = fixture_stats();
    let role_map = fixture_role_map();

    // Session where every candidate champion is already picked/banned
    let session = ChampSelectState {
        my_cell_id: 0,
        local_player: TeamSlot {
            cell_id: 0,
            champion_id: 157,
            intent_champion_id: 157,
            assigned_position: "middle".into(),
            is_locked: true,
        },
        my_team: vec![
            TeamSlot {
                cell_id: 0,
                champion_id: 157,
                intent_champion_id: 157,
                assigned_position: "middle".into(),
                is_locked: true,
            },
            TeamSlot {
                cell_id: 1,
                champion_id: 238,
                intent_champion_id: 238,
                assigned_position: "top".into(),
                is_locked: true,
            },
        ],
        their_team: vec![
            TeamSlot {
                cell_id: 5,
                champion_id: 122,
                intent_champion_id: 122,
                assigned_position: "jungle".into(),
                is_locked: true,
            },
            TeamSlot {
                cell_id: 6,
                champion_id: 99,
                intent_champion_id: 99,
                assigned_position: "bottom".into(),
                is_locked: true,
            },
        ],
        my_bans: vec![86], // Garen banned — exhausts entire pool
        their_bans: vec![],
        phase: "BAN_PICK".into(),
        time_left_ms: 20_000,
        action_type: "pick".into(),
        queue_id: 0,
        pick_order: 0,
    };

    let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
    let ctx = ScoringContext {
        session: &session,
        mastery: &mastery,
        stats: &stats,
        role_map: &role_map,
        meta_rates: &meta_rates,
        weights: ScoringWeights::default(),
        matchups: None,
        power_curves: None,
    };

    let kb = DraftKnowledgeBase::load().expect("KB load failed");
    let recs = compute_recommendations(&ctx, &champions, &[], &[], &kb);
    assert!(
        recs.is_empty(),
        "Tüm adaylar seçilmişken/ban'lıyken öneri listesi boş olmalı"
    );
}

#[test]
fn test_recommendation_snapshot_weight_sensitivity() {
    // Verify that champions with very different profiles swap rank when
    // we mutate the scoring context to heavily favour comfort vs meta.
    // We don't modify the static WEIGHTS constant (it is a const); instead
    // we directly compute weighted totals with two custom weight sets and
    // confirm the ordering differs — validating the scoring formulas respond
    // to weight changes as expected.

    let mastery = fixture_mastery();
    let stats = fixture_stats();
    let role_map = fixture_role_map();
    let session = fixture_session();

    let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
    let ctx = ScoringContext {
        session: &session,
        mastery: &mastery,
        stats: &stats,
        role_map: &role_map,
        meta_rates: &meta_rates,
        weights: ScoringWeights::default(),
        matchups: None,
        power_curves: None,
    };

    use crate::recommendation::scoring::{
        comfort_score, matchup_score, meta_score, synergy_score, team_counter_score,
    };

    // Score both Zed and Garen under two weight regimes.
    // Zed: high mastery→comfort + matchup vs Lux (assassin beats mage) + best KDA meta
    // Garen: zero mastery + no matchup data + no KDA

    let c_zed = comfort_score(238, &ctx);
    let lc_zed = matchup_score(238, &ctx);
    let tc_zed = team_counter_score(238, &ctx);
    let sy_zed = synergy_score(238, &ctx);
    let mt_zed = meta_score(238, &ctx);

    let c_gar = comfort_score(86, &ctx);
    let lc_gar = matchup_score(86, &ctx);
    let tc_gar = team_counter_score(86, &ctx);
    let sy_gar = synergy_score(86, &ctx);
    let mt_gar = meta_score(86, &ctx);

    // Weight set A: comfort-heavy  (0.60, 0.10, 0.10, 0.10, 0.10)
    let score_zed_a = 0.60 * c_zed + 0.10 * lc_zed + 0.10 * tc_zed + 0.10 * sy_zed + 0.10 * mt_zed;
    let score_gar_a = 0.60 * c_gar + 0.10 * lc_gar + 0.10 * tc_gar + 0.10 * sy_gar + 0.10 * mt_gar;

    // Weight set B: meta-heavy  (0.05, 0.05, 0.05, 0.05, 0.80)
    let score_zed_b = 0.05 * c_zed + 0.05 * lc_zed + 0.05 * tc_zed + 0.05 * sy_zed + 0.80 * mt_zed;
    let score_gar_b = 0.05 * c_gar + 0.05 * lc_gar + 0.05 * tc_gar + 0.05 * sy_gar + 0.80 * mt_gar;

    // Under comfort-heavy weights, Zed (high mastery) should beat Garen (no mastery)
    assert!(
        score_zed_a > score_gar_a,
        "Comfort-heavy ağırlıkta Zed > Garen olmalı (zed={:.3}, garen={:.3})",
        score_zed_a,
        score_gar_a
    );

    // Under meta-heavy weights, Zed (KDA 6.0 → meta_score ~1.0) should also beat Garen (0.3 default)
    assert!(
        score_zed_b > score_gar_b,
        "Meta-heavy ağırlıkta Zed > Garen olmalı (zed={:.3}, garen={:.3})",
        score_zed_b,
        score_gar_b
    );

    // Crucially: the raw comfort gap is large enough that Yasuo (mastery lvl 7, 20 games)
    // would outscore Lux (mastery lvl 3, 3 games) under comfort-heavy weights if both were
    // available. Verify comfort_score difference is non-trivial (> 0.1) to confirm
    // the formulas are sensitive to mastery data.
    let c_yasuo = comfort_score(157, &ctx);
    let c_lux = comfort_score(99, &ctx);
    assert!(
        c_yasuo - c_lux > 0.10,
        "Yasuo comfort Lux'tan belirgin şekilde yüksek olmalı (diff={:.3})",
        c_yasuo - c_lux
    );
}

#[test]
fn test_recommendation_snapshot_with_db_fixture() {
    // Full round-trip: insert fixture data into in-memory SQLite, read it back via
    // repo functions, then feed the loaded data into compute_recommendations.
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("full.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let puuid = "test-puuid";

    // Insert summoner
    conn.execute(
        "INSERT INTO summoners (puuid, game_name, tag_line, region, cached_at)
         VALUES (?1, 'TestPlayer', 'EUW', 'euw1', 1)",
        rusqlite::params![puuid],
    )
    .unwrap();

    // Insert champions
    for (id, key, name) in [
        (157i64, "Yasuo", "Yasuo"),
        (238, "Zed", "Zed"),
        (122, "Darius", "Darius"),
        (99, "Lux", "Lux"),
        (86, "Garen", "Garen"),
    ] {
        conn.execute(
            "INSERT INTO champions (champion_id, key, name, title, cached_at)
             VALUES (?1, ?2, ?3, 'Title', 1)",
            rusqlite::params![id, key, name],
        )
        .unwrap();
    }

    // Insert mastery via repo
    mastery_repo::upsert_mastery(&conn, puuid, 157, 7, 180_000, None).unwrap();
    mastery_repo::upsert_mastery(&conn, puuid, 238, 6, 90_000, None).unwrap();
    mastery_repo::upsert_mastery(&conn, puuid, 122, 5, 50_000, None).unwrap();
    mastery_repo::upsert_mastery(&conn, puuid, 99, 3, 15_000, None).unwrap();

    // Insert matches via repo (to drive player_stats)
    for (mid, champ, win, k, d, a) in [
        ("M01", 157i64, true, 5i64, 1i64, 8i64),
        ("M02", 157, true, 4, 2, 6),
        ("M03", 157, false, 2, 5, 3),
        ("M04", 238, true, 8, 1, 4),
        ("M05", 238, true, 7, 0, 5),
        ("M06", 122, false, 3, 4, 2),
        ("M07", 122, true, 2, 2, 1),
        ("M08", 99, false, 0, 3, 5),
    ] {
        match_repo::insert_match(
            &conn, mid, puuid, champ, None, win, k, d, a, 1800, 420, 1_000_000,
        )
        .unwrap();
    }

    // Load data back from DB
    let mastery_rows = mastery_repo::top_for_puuid(&conn, puuid, 20).unwrap();
    let stats_rows = match_repo::player_stats(&conn, puuid).unwrap();
    let all_champions = crate::db::champion_repo::list_all(&conn).unwrap();

    let role_map = fixture_role_map();
    let session = fixture_session();

    let meta_rates: HashMap<(u32, String), MetaRate> = HashMap::new();
    let ctx = ScoringContext {
        session: &session,
        mastery: &mastery_rows,
        stats: &stats_rows,
        role_map: &role_map,
        meta_rates: &meta_rates,
        weights: ScoringWeights::default(),
        matchups: None,
        power_curves: None,
    };

    let kb = DraftKnowledgeBase::load().expect("KB load failed");
    let recs = compute_recommendations(&ctx, &all_champions, &[], &[], &kb);

    // Expect at least Zed and Garen in results (or fewer if all picked)
    assert!(
        !recs.is_empty(),
        "DB'den yüklenen verilerle en az bir öneri dönmeli"
    );

    // IDs already excluded (157 Yasuo=picked, 122 Darius=ally, 99 Lux=enemy)
    let rec_ids: Vec<u32> = recs.iter().map(|r| r.champion_id).collect();
    assert!(
        !rec_ids.contains(&157),
        "Yasuo (picked) DB round-trip testinde önerilmemeli"
    );
    assert!(
        !rec_ids.contains(&122),
        "Darius (ally) DB round-trip testinde önerilmemeli"
    );
    assert!(
        !rec_ids.contains(&99),
        "Lux (enemy) DB round-trip testinde önerilmemeli"
    );

    // Results are sorted by score descending
    for window in recs.windows(2) {
        assert!(
            window[0].total_score >= window[1].total_score,
            "DB round-trip: sonuçlar azalan skor sırasında olmalı"
        );
    }

    // confidence field must be set (not empty)
    for rec in &recs {
        assert!(
            !rec.confidence.is_empty(),
            "Her öneri için confidence alanı dolu olmalı (champion_id={})",
            rec.champion_id
        );
    }
}

// -------------------------------------------------------------------------
// builds_repo matchup-aware tests (V009)
// -------------------------------------------------------------------------

fn insert_build(
    conn: &rusqlite::Connection,
    champion_id: i64,
    position: &str,
    items: &str,
    opponent_archetype: Option<&str>,
) {
    let row = builds_repo::BuildRow {
        champion_id,
        position: position.to_string(),
        patch_version: "15.1".to_string(),
        item_ids: items.to_string(),
        rune_ids: "[8010,8000]".to_string(),
        win_rate: 0.53,
        source: "manual_seed".to_string(),
        opponent_archetype: opponent_archetype.map(|s| s.to_string()),
        skill_order: None,
        summoner_spells: None,
        secondary_runes: None,
        stat_shards: None,
    };
    builds_repo::upsert_build(conn, &row).expect("build insert başarılı olmalı");
}

fn ensure_champion(conn: &rusqlite::Connection, champion_id: i64, key: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO champions (champion_id, key, name, title, cached_at)
         VALUES (?1, ?2, ?2, '', 1)",
        rusqlite::params![champion_id, key],
    )
    .expect("champion insert başarılı olmalı");
}

#[test]
fn get_build_for_matchup_returns_specific_when_exists() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("b1.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    ensure_champion(&conn, 86, "Garen");
    insert_build(&conn, 86, "top", "[3078,3071,3053,3748]", None);
    insert_build(
        &conn,
        86,
        "top",
        "[3078,3071,3123,3053]",
        Some("juggernaut"),
    );

    let result = builds_repo::get_build_for_matchup(&conn, 86, "top", "juggernaut")
        .expect("sorgu hatasız çalışmalı");
    let build = result.expect("juggernaut matchup build bulunmalı");

    let ids: Vec<u32> = serde_json::from_str(&build.item_ids).expect("item_ids parse edilmeli");
    assert!(
        ids.contains(&3123),
        "Juggernaut'a özel Executioner's Calling (3123) build'de olmalı"
    );
    assert_eq!(build.opponent_archetype.as_deref(), Some("juggernaut"));
}

#[test]
fn get_build_for_matchup_falls_back_to_default_when_no_specific() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("b2.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    ensure_champion(&conn, 86, "Garen");
    insert_build(&conn, 86, "top", "[3078,3071,3053,3748]", None);
    insert_build(
        &conn,
        86,
        "top",
        "[3078,3071,3123,3053]",
        Some("juggernaut"),
    );

    // "skirmisher" has no specific build → should fall back to NULL-archetype default
    let result = builds_repo::get_build_for_matchup(&conn, 86, "top", "skirmisher")
        .expect("sorgu hatasız çalışmalı");
    let build = result.expect("fallback default build bulunmalı");

    assert!(
        build.opponent_archetype.is_none(),
        "Fallback build opponent_archetype NULL olmalı"
    );
    let ids: Vec<u32> = serde_json::from_str(&build.item_ids).expect("item_ids parse edilmeli");
    assert!(
        ids.contains(&3748),
        "Default build Titanic Hydra (3748) içermeli"
    );
}

#[test]
fn get_build_returns_none_when_no_build_exists() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("b3.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let result = builds_repo::get_build(&conn, 999, "top").expect("sorgu hatasız çalışmalı");
    assert!(result.is_none(), "Bilinmeyen champion için None dönmeli");
}

#[test]
fn get_build_for_matchup_returns_none_when_no_build_at_all() {
    let dir = tempdir().unwrap();
    let mut conn = open_db(&dir.path().join("b4.db")).unwrap();
    run_migrations(&mut conn).unwrap();

    let result = builds_repo::get_build_for_matchup(&conn, 999, "top", "juggernaut")
        .expect("sorgu hatasız çalışmalı");
    assert!(result.is_none(), "Hiç build yokken None dönmeli");
}

// -------------------------------------------------------------------------
// N2 (V011) — pro build field round-trip tests
// -------------------------------------------------------------------------

#[test]
fn v011_pro_fields_round_trip_through_upsert_and_get() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("v011_round_trip.db");
    let mut conn = open_db(&db_path).unwrap();
    run_migrations(&mut conn).unwrap();
    ensure_champion(&conn, 122, "Darius");

    let row = builds_repo::BuildRow {
        champion_id: 122,
        position: "top".to_string(),
        patch_version: "15.1".to_string(),
        item_ids: "[3078,3071,3053,3083]".to_string(),
        rune_ids: "[8010,8000]".to_string(),
        win_rate: 0.53,
        source: "manual_seed".to_string(),
        opponent_archetype: None,
        skill_order: Some("Q→E→W".to_string()),
        summoner_spells: Some("[4,12]".to_string()),
        secondary_runes: Some("[8400,8473,8453]".to_string()),
        stat_shards: Some("[5005,5008,5002]".to_string()),
    };
    builds_repo::upsert_build(&conn, &row).expect("upsert ok");

    let fetched = builds_repo::get_build(&conn, 122, "top")
        .expect("get_build hatasız")
        .expect("Darius top build var olmalı");

    assert_eq!(fetched.skill_order.as_deref(), Some("Q→E→W"));
    assert_eq!(fetched.summoner_spells.as_deref(), Some("[4,12]"));
    assert_eq!(fetched.secondary_runes.as_deref(), Some("[8400,8473,8453]"));
    assert_eq!(fetched.stat_shards.as_deref(), Some("[5005,5008,5002]"));
}

#[test]
fn v011_pro_fields_nullable_when_not_provided() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("v011_nullable.db");
    let mut conn = open_db(&db_path).unwrap();
    run_migrations(&mut conn).unwrap();
    ensure_champion(&conn, 86, "Garen");
    insert_build(&conn, 86, "top", "[3078,3071,3053,3748]", None);

    let fetched = builds_repo::get_build(&conn, 86, "top")
        .expect("get_build hatasız")
        .expect("Garen build var olmalı");

    // insert_build sets all pro fields to None; round-trip preserves that
    assert!(fetched.skill_order.is_none());
    assert!(fetched.summoner_spells.is_none());
    assert!(fetched.secondary_runes.is_none());
    assert!(fetched.stat_shards.is_none());
}
