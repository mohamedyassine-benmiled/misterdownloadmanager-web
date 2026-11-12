//! Fork contracts that must survive upstream IPC additions without exposing owner authority.
use super::*;

#[test]
fn web_authority_is_in_process_only_and_never_round_trips_through_json() {
    let injected: Command = serde_json::from_value(serde_json::json!({
        "remote": true, "web_owner": true, "web_request": { "op": "settings" },
        "torrent_file": [100, 101]
    }))
    .unwrap();
    assert!(!injected.remote && !injected.web_owner);
    assert!(injected.web_request.is_none() && injected.torrent_file.is_none());
    assert!(!injected.wants_reply());

    let trusted = Command {
        remote: true,
        web_owner: true,
        web_request: Some(serde_json::json!({ "op": "settings" })),
        ..Default::default()
    };
    assert!(trusted.wants_reply());
    let serialized = serde_json::to_value(&trusted).unwrap();
    for key in ["remote", "web_owner", "web_request", "torrent_file"] {
        assert!(
            serialized.get(key).is_none(),
            "internal authority leaked: {key}"
        );
    }
    let restored: Command = serde_json::from_value(serialized).unwrap();
    assert!(!restored.remote && !restored.web_owner && restored.web_request.is_none());
}

#[test]
fn additive_web_results_keep_legacy_answers_readable() {
    let legacy: api::AddAnswer = serde_json::from_str(r#"{"id":7,"pending":false}"#).unwrap();
    assert_eq!(legacy.id, Some(7));
    assert!(legacy.request_id.is_none());
    let current = api::AddAnswer {
        id: None,
        pending: true,
        request_id: Some("request-1".into()),
    };
    let value = serde_json::to_value(&current).unwrap();
    assert_eq!(value["pending"], true);
    assert_eq!(value["request_id"], "request-1");
}
