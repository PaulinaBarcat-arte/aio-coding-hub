use serde_json::json;

pub(super) fn is_anthropic_warmup_request(forwarded_path: &str, body_bytes: &[u8]) -> bool {
    if forwarded_path != "/v1/messages" {
        return false;
    }

    let Ok(root) = serde_json::from_slice::<serde_json::Value>(body_bytes) else {
        return false;
    };

    let Some(messages) = root.get("messages").and_then(|v| v.as_array()) else {
        return false;
    };
    if messages.len() != 1 {
        return false;
    }

    let Some(first_message) = messages.first().and_then(|v| v.as_object()) else {
        return false;
    };
    if first_message.get("role").and_then(|v| v.as_str()) != Some("user") {
        return false;
    }

    let Some(content) = first_message.get("content").and_then(|v| v.as_array()) else {
        return false;
    };
    if content.len() != 1 {
        return false;
    }

    let Some(first_block) = content.first().and_then(|v| v.as_object()) else {
        return false;
    };
    if first_block.get("type").and_then(|v| v.as_str()) != Some("text") {
        return false;
    }

    let text = first_block
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if text != "warmup" {
        return false;
    }

    let Some(cache_control) = first_block.get("cache_control").and_then(|v| v.as_object()) else {
        return false;
    };
    cache_control.get("type").and_then(|v| v.as_str()) == Some("ephemeral")
}

pub(super) fn build_warmup_response_body(model: Option<&str>, trace_id: &str) -> serde_json::Value {
    json!({
        "model": model.unwrap_or("unknown"),
        "id": format!("msg_aio_{trace_id}"),
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "text",
                "text": "I'm ready to help you."
            }
        ],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warmup_request_matches_strict_shape() {
        let body = serde_json::json!({
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": " Warmup ",
                            "cache_control": { "type": "ephemeral" }
                        }
                    ]
                }
            ]
        });
        let bytes = serde_json::to_vec(&body).unwrap();
        assert!(is_anthropic_warmup_request("/v1/messages", &bytes));
    }

    #[test]
    fn warmup_request_rejects_wrong_path() {
        let body = serde_json::json!({
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "warmup",
                            "cache_control": { "type": "ephemeral" }
                        }
                    ]
                }
            ]
        });
        let bytes = serde_json::to_vec(&body).unwrap();
        assert!(!is_anthropic_warmup_request(
            "/v1/messages/count_tokens",
            &bytes
        ));
    }

    #[test]
    fn warmup_request_rejects_missing_cache_control() {
        let body = serde_json::json!({
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "warmup"
                        }
                    ]
                }
            ]
        });
        let bytes = serde_json::to_vec(&body).unwrap();
        assert!(!is_anthropic_warmup_request("/v1/messages", &bytes));
    }
}
