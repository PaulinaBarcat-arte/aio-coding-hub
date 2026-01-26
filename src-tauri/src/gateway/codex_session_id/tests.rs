use super::*;

#[test]
fn prefers_prompt_cache_key_over_metadata_session_id() {
    let mut cache = CodexSessionIdCache::default();
    let now_unix = 123;
    let now_unix_ms = 123_000;
    let mut headers = HeaderMap::new();
    let mut body = serde_json::json!({
        "prompt_cache_key": "01234567-89ab-cdef-0123-456789abcdef",
        "metadata": { "session_id": "11111111-2222-3333-4444-555555555555" }
    });

    let result = complete_codex_session_identifiers(
        &mut cache,
        now_unix,
        now_unix_ms,
        &mut headers,
        Some(&mut body),
    );

    assert!(result.applied);
    assert_eq!(result.source, "body_prompt_cache_key");
    assert_eq!(result.action, "completed_missing_fields");
    assert_eq!(
        result.session_id,
        "01234567-89ab-cdef-0123-456789abcdef".to_string()
    );
    assert_eq!(
        headers.get("session_id").unwrap().to_str().unwrap(),
        result.session_id
    );
    assert_eq!(
        headers.get("x-session-id").unwrap().to_str().unwrap(),
        result.session_id
    );
    assert_eq!(
        body.get("prompt_cache_key").unwrap().as_str().unwrap(),
        result.session_id
    );
}

#[test]
fn uses_metadata_session_id_when_prompt_cache_key_missing() {
    let mut cache = CodexSessionIdCache::default();
    let now_unix = 123;
    let now_unix_ms = 123_000;
    let mut headers = HeaderMap::new();
    let mut body = serde_json::json!({
        "metadata": { "session_id": "01234567-89ab-cdef-0123-456789abcdef" }
    });

    let result = complete_codex_session_identifiers(
        &mut cache,
        now_unix,
        now_unix_ms,
        &mut headers,
        Some(&mut body),
    );

    assert!(result.applied);
    assert_eq!(result.source, "body_metadata_session_id");
    assert_eq!(result.action, "completed_missing_fields");
    assert_eq!(
        headers.get("session_id").unwrap().to_str().unwrap(),
        result.session_id
    );
    assert_eq!(
        body.get("prompt_cache_key").unwrap().as_str().unwrap(),
        result.session_id
    );
}

#[test]
fn uses_previous_response_id_when_other_sources_missing() {
    let mut cache = CodexSessionIdCache::default();
    let now_unix = 123;
    let now_unix_ms = 123_000;
    let mut headers = HeaderMap::new();
    let mut body = serde_json::json!({
        "previous_response_id": "resp_01234567-89ab-cdef-0123-456789abcdef"
    });

    let result = complete_codex_session_identifiers(
        &mut cache,
        now_unix,
        now_unix_ms,
        &mut headers,
        Some(&mut body),
    );

    assert!(result.applied);
    assert_eq!(result.source, "body_previous_response_id");
    assert_eq!(result.action, "completed_missing_fields");
    assert_eq!(
        result.session_id,
        "codex_prev_resp_01234567-89ab-cdef-0123-456789abcdef".to_string()
    );
    assert_eq!(
        headers.get("session_id").unwrap().to_str().unwrap(),
        result.session_id
    );
    assert_eq!(
        body.get("prompt_cache_key").unwrap().as_str().unwrap(),
        result.session_id
    );
}
