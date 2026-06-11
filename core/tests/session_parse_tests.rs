// MIGRATED from src-tauri/src/lcu/session.rs (host emekliligi): parse_session
// fixture testleri - saf core mantigi, LCU fixture'lari core/tests/fixtures'ta.
mod tests {
    use csa_core::session_parse::parse_session;
    use csa_core::types::ChampSelectState;
    use serde_json::json;

    #[test]
    fn parse_minimal_session() {
        let v = json!({
            "localPlayerCellId": 2,
            "myTeam": [
                {"cellId": 2, "championId": 99, "championPickIntent": 0,
                 "assignedPosition": "middle"}
            ],
            "theirTeam": [],
            "bans": {"myTeamBans": [], "theirTeamBans": []},
            "actions": [],
            "timer": {"phase": "BAN_PICK", "adjustedTimeLeftInPhase": 27000}
        });
        let state = parse_session(&v).expect("should parse");
        assert_eq!(state.my_cell_id, 2);
        assert_eq!(state.local_player.champion_id, 99);
        assert_eq!(state.phase, "BAN_PICK");
    }

    #[test]
    fn parse_missing_local_cell() {
        // Returns None when localPlayerCellId is absent
        let v = json!({"myTeam": []});
        assert!(parse_session(&v).is_none());
    }

    #[test]
    fn parse_pick_order_ban_phase_returns_zero() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/ban_acting.json")).unwrap();
        let state = parse_session(&v).expect("ban_acting fixture parse edilemedi");
        assert_eq!(
            state.pick_order, 0,
            "Ban fazında pick_order bilinmiyor, 0 dönmeli"
        );
    }

    #[test]
    fn parse_pick_order_pick_acting() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/pick_acting.json")).unwrap();
        let state = parse_session(&v).expect("pick_acting fixture parse edilemedi");
        assert_eq!(
            state.pick_order, 2,
            "pick_acting fixture'da oyuncu 2. sırada seçiyor, pick_order=2 bekleniyor"
        );
    }

    #[test]
    fn parse_pick_order_finalization() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/finalization.json")).unwrap();
        let state = parse_session(&v).expect("finalization fixture parse edilemedi");
        assert_eq!(
            state.pick_order, 2,
            "finalization fixture'da oyuncu 2. sırada seçti, pick_order=2 bekleniyor"
        );
    }

    /// Regression: the live flow is raw LCU → parse_session → emit (serialize) →
    /// frontend → get_recommendations (deserialize). The serialized ChampSelectState
    /// must NOT look like a raw LCU session (no "actions") and must round-trip back
    /// via `from_value`. Guards the get_recommendations parse-branch (Bug: live sent
    /// a serialized state into parse_session → "Geçersiz session JSON").
    #[test]
    fn parsed_state_roundtrips_for_get_recommendations() {
        let raw: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/pick_acting.json")).unwrap();
        let state = parse_session(&raw).expect("raw parse");

        let serialized = serde_json::to_value(&state).expect("serialize state");
        assert!(
            serialized.get("actions").is_none(),
            "serialized ChampSelectState must not carry raw LCU 'actions' (would mis-route to parse_session)"
        );

        let roundtripped: ChampSelectState =
            serde_json::from_value(serialized).expect("serialized state must deserialize back");
        assert_eq!(roundtripped.queue_id, state.queue_id);
        assert_eq!(roundtripped.action_type, state.action_type);
        assert_eq!(roundtripped.my_cell_id, state.my_cell_id);
    }

    #[test]
    fn parse_ban_acting_fixture() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/ban_acting.json")).unwrap();
        let state = parse_session(&v).expect("ban_acting fixture parse edilemedi");
        assert_eq!(state.my_cell_id, 2);
        assert_eq!(
            state.action_type, "ban",
            "Ban fazinda action_type 'ban' olmali"
        );
        assert_eq!(state.phase, "BAN_PICK");
        assert_eq!(state.local_player.assigned_position, "middle");
        assert_eq!(state.my_bans.len(), 1); // Zed banli
    }

    #[test]
    fn parse_pick_acting_fixture() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/pick_acting.json")).unwrap();
        let state = parse_session(&v).expect("pick_acting fixture parse edilemedi");
        assert_eq!(
            state.action_type, "pick",
            "Pick fazinda action_type 'pick' olmali"
        );
        assert_eq!(
            state.local_player.intent_champion_id, 238,
            "Hover'daki champion dogru"
        );
        assert!(
            !state.local_player.is_locked,
            "Pick tamamlanmadi, locked olmamali"
        );
    }

    #[test]
    fn parse_pick_watching_fixture() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/pick_watching.json")).unwrap();
        let state = parse_session(&v).expect("pick_watching fixture parse edilemedi");
        assert_eq!(
            state.action_type, "",
            "Baskasinin sirasi -- action_type bos olmali"
        );
        assert!(
            state.local_player.is_locked,
            "Oyuncu kilitli olmali (completed pick var)"
        );
    }

    #[test]
    fn parse_finalization_fixture() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/finalization.json")).unwrap();
        let state = parse_session(&v).expect("finalization fixture parse edilemedi");
        assert_eq!(state.phase, "FINALIZATION");
        assert_eq!(
            state.local_player.champion_id, 238,
            "Kilit fazinda champion_id dolu olmali"
        );
        assert!(
            state.local_player.is_locked,
            "Kilit fazinda oyuncu kilitli olmali"
        );
        let locked_count = state.my_team.iter().filter(|s| s.is_locked).count();
        assert!(
            locked_count >= 4,
            "En az 4 oyuncu kilitli olmali, got {}",
            locked_count
        );
    }

    /// Blind/Normal (queue 430): the LCU leaves every `assignedPosition` empty.
    /// This locks the data shape the Faz 8 role-fallback relies on — when the
    /// local player's position is empty, the UI's RoleSelector supplies it.
    #[test]
    fn parse_blind_pick_empty_positions() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/blind_pick.json")).unwrap();
        let state = parse_session(&v).expect("blind_pick fixture parse edilemedi");
        assert_eq!(
            state.local_player.assigned_position, "",
            "Blind modda yerel pozisyon boş gelmeli (RoleSelector fallback'ı bu durumda devreye girer)"
        );
        assert!(
            state.my_team.iter().all(|s| s.assigned_position.is_empty()),
            "Blind modda tüm pozisyonlar boş olmalı"
        );
        assert_eq!(
            state.action_type, "pick",
            "Yerel oyuncunun pick sırası aktif"
        );
        assert_eq!(state.queue_id, 430, "Normal Blind queue id 430");
        assert!(
            state.their_team.iter().any(|s| s.champion_id == 99),
            "Kilitli düşman pickleri (Lux=99) görünür olmalı — lane çıkarımı bunları kullanır"
        );
    }
}
