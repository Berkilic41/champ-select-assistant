// MIGRATED from src-tauri/src/recommendation (host emekliligi): core integration testi.
// Import donusumu: crate::recommendation::->csa_core::, host tip yollari->csa_core::types::.
//! Raw Match-V5 JSON schema-drift fixtures (Claude QA).
//!
//! Tests the pure `match_v5_mapper` against malformed / drifted Riot payloads
//! (missing ids, null participants, broken perks, partial items, weird patches,
//! string-typed numerics). The whole module is test-only — it never touches the
//! command/runtime fetch path (Codex-owned). Goal: the mapper degrades gracefully
//! (defaults, never panics), so a Riot schema change can't crash a refresh.

use csa_core::match_v5_mapper::{
    match_v5_from_detail, normalize_patch, parse_items, parse_rune_ids,
};
use serde_json::json;

#[test]
fn missing_match_id_falls_back_to_provided_id() {
    // info present but no metadata.matchId (and no metadata key at all).
    let detail = json!({ "info": { "queueId": 420, "participants": [] } });
    let mapped = match_v5_from_detail(&detail, "FALLBACK_99").expect("info exists");
    assert_eq!(mapped.match_id, "FALLBACK_99");
}

#[test]
fn null_or_missing_participants_yield_empty_list() {
    for participants in [json!(null), json!("oops"), json!({})] {
        let detail = json!({ "info": { "queueId": 420, "participants": participants } });
        let mapped = match_v5_from_detail(&detail, "id").expect("info exists");
        assert!(
            mapped.participants.is_empty(),
            "non-array participants → empty"
        );
    }
    // participants key absent entirely.
    let detail = json!({ "info": { "queueId": 420 } });
    assert!(match_v5_from_detail(&detail, "id")
        .unwrap()
        .participants
        .is_empty());
    // empty array.
    let detail = json!({ "info": { "queueId": 420, "participants": [] } });
    assert!(match_v5_from_detail(&detail, "id")
        .unwrap()
        .participants
        .is_empty());
}

#[test]
fn malformed_perks_styles_never_panic() {
    let cases = [
        json!({ "perks": "not-an-object" }),
        json!({ "perks": { "styles": "not-an-array" } }),
        json!({ "perks": { "styles": [{ "selections": "nope" }] } }),
        json!({ "perks": { "styles": [{ "selections": [{ "perk": "8010" }] }] } }), // perk is string
        json!({ "perks": { "styles": [{ "selections": [{ "notPerk": 8010 }] }] } }),
        json!({}), // no perks key
    ];
    for participant in cases {
        let runes = parse_rune_ids(&participant);
        assert!(
            runes.is_empty(),
            "malformed perks → empty runes, no panic: {participant}"
        );
    }
    // A valid-looking style still parses.
    let good =
        json!({ "perks": { "styles": [{ "selections": [{ "perk": 8010 }, { "perk": 9111 }] }] } });
    assert_eq!(parse_rune_ids(&good), vec![8010, 9111]);
}

#[test]
fn partial_or_string_item_slots_default_to_zero() {
    // Only item0 + item3 present; item2 is a string (drift) → 0. Always 7 slots.
    let participant = json!({ "item0": 1055, "item2": "3047", "item3": 3006 });
    let items = parse_items(&participant);
    assert_eq!(items.len(), 7);
    assert_eq!(items[0], 1055);
    assert_eq!(items[1], 0); // missing
    assert_eq!(items[2], 0); // string → 0
    assert_eq!(items[3], 3006);
    // No item keys at all → all zeros.
    assert_eq!(parse_items(&json!({})), vec![0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn weird_game_versions_normalize_without_panic() {
    assert_eq!(normalize_patch("16.10.1.2.3"), "16.10");
    assert_eq!(normalize_patch("16"), "unknown"); // no minor
    assert_eq!(normalize_patch("16."), "unknown"); // empty minor
    assert_eq!(normalize_patch(".10"), "unknown"); // empty major
    assert_eq!(normalize_patch("..."), "unknown");
    assert_eq!(normalize_patch("  "), "unknown"); // whitespace, no dot
                                                  // Non-numeric but well-formed passes through (not a crash; Riot is numeric).
    assert_eq!(normalize_patch("14.11b"), "14.11b");
}

#[test]
fn string_or_null_numeric_fields_default_to_zero_no_panic() {
    let detail = json!({
        "metadata": { "matchId": "EUW1_x" },
        "info": {
            "queueId": "420",      // string instead of number → 0
            "gameVersion": 1610,    // number instead of string → "unknown"
            "participants": [{
                "championId": null,    // null → 0
                "teamId": "100",       // string → 0
                "teamPosition": 5,      // wrong type → "" (as_str None)
                "win": "true",          // string → false (as_bool None)
                "kills": null,
                "summoner1Id": "4"
            }]
        }
    });
    let mapped = match_v5_from_detail(&detail, "fb").expect("info exists");
    assert_eq!(mapped.queue_id, 0, "string queueId → 0");
    assert_eq!(mapped.patch, "unknown", "numeric gameVersion → unknown");
    let p = &mapped.participants[0];
    assert_eq!(p.champion_id, 0);
    assert_eq!(p.team_id, 0);
    assert_eq!(p.team_position, "");
    assert!(!p.win);
    assert_eq!(p.summoner_spells, vec![0, 0]); // "4" string → 0
}

#[test]
fn completely_empty_object_returns_none() {
    // No `info` → None (matches the mapper's only hard-fail path).
    assert!(match_v5_from_detail(&json!({}), "id").is_none());
    assert!(match_v5_from_detail(&json!(null), "id").is_none());
}
