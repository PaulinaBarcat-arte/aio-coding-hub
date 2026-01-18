pub(super) type ThinkingSignatureRectifierTrigger = &'static str;

pub(super) const TRIGGER_INVALID_SIGNATURE_IN_THINKING_BLOCK: ThinkingSignatureRectifierTrigger =
    "invalid_signature_in_thinking_block";
pub(super) const TRIGGER_ASSISTANT_MESSAGE_MUST_START_WITH_THINKING:
    ThinkingSignatureRectifierTrigger = "assistant_message_must_start_with_thinking";
pub(super) const TRIGGER_INVALID_REQUEST: ThinkingSignatureRectifierTrigger = "invalid_request";

#[derive(Debug, Clone, Copy)]
pub(super) struct ThinkingSignatureRectifierResult {
    pub(super) applied: bool,
    pub(super) removed_thinking_blocks: usize,
    pub(super) removed_redacted_thinking_blocks: usize,
    pub(super) removed_signature_fields: usize,
    pub(super) removed_top_level_thinking: bool,
}

pub(super) fn detect_trigger(error_message: &str) -> Option<ThinkingSignatureRectifierTrigger> {
    if error_message.trim().is_empty() {
        return None;
    }

    let lower = error_message.to_lowercase();

    let looks_like_thinking_enabled_but_missing_thinking_prefix = lower
        .contains("must start with a thinking block")
        || (lower.contains("expected")
            && lower.contains("thinking")
            && (lower.contains("redacted_thinking") || lower.contains("redacted thinking"))
            && lower.contains("found")
            && (lower.contains("tool_use") || lower.contains("tool use")));

    if looks_like_thinking_enabled_but_missing_thinking_prefix {
        return Some(TRIGGER_ASSISTANT_MESSAGE_MUST_START_WITH_THINKING);
    }

    let looks_like_invalid_signature_in_thinking_block = lower.contains("invalid")
        && lower.contains("signature")
        && lower.contains("thinking")
        && lower.contains("block");
    if looks_like_invalid_signature_in_thinking_block {
        return Some(TRIGGER_INVALID_SIGNATURE_IN_THINKING_BLOCK);
    }

    if error_message.contains("非法请求")
        || lower.contains("illegal request")
        || lower.contains("invalid request")
    {
        return Some(TRIGGER_INVALID_REQUEST);
    }

    None
}

pub(super) fn rectify_anthropic_request_message(
    message: &mut serde_json::Value,
) -> ThinkingSignatureRectifierResult {
    let mut removed_thinking_blocks = 0usize;
    let mut removed_redacted_thinking_blocks = 0usize;
    let mut removed_signature_fields = 0usize;
    let mut removed_top_level_thinking = false;
    let mut applied = false;

    let Some(message_obj) = message.as_object_mut() else {
        return ThinkingSignatureRectifierResult {
            applied: false,
            removed_thinking_blocks,
            removed_redacted_thinking_blocks,
            removed_signature_fields,
            removed_top_level_thinking,
        };
    };

    let thinking_enabled = message_obj
        .get("thinking")
        .and_then(|v| v.as_object())
        .and_then(|obj| obj.get("type"))
        .and_then(|v| v.as_str())
        == Some("enabled");

    let mut should_remove_top_level_thinking = false;

    {
        let Some(messages) = message_obj
            .get_mut("messages")
            .and_then(|v| v.as_array_mut())
        else {
            return ThinkingSignatureRectifierResult {
                applied: false,
                removed_thinking_blocks,
                removed_redacted_thinking_blocks,
                removed_signature_fields,
                removed_top_level_thinking,
            };
        };

        for msg in messages.iter_mut() {
            let Some(msg_obj) = msg.as_object_mut() else {
                continue;
            };

            let Some(content) = msg_obj.get_mut("content").and_then(|v| v.as_array_mut()) else {
                continue;
            };

            let original = std::mem::take(content);
            let mut new_content: Vec<serde_json::Value> = Vec::with_capacity(original.len());
            let mut content_modified = false;

            for mut block in original {
                let Some(block_obj) = block.as_object_mut() else {
                    new_content.push(block);
                    continue;
                };

                match block_obj.get("type").and_then(|v| v.as_str()) {
                    Some("thinking") => {
                        removed_thinking_blocks += 1;
                        content_modified = true;
                        continue;
                    }
                    Some("redacted_thinking") => {
                        removed_redacted_thinking_blocks += 1;
                        content_modified = true;
                        continue;
                    }
                    _ => {}
                }

                if block_obj.remove("signature").is_some() {
                    removed_signature_fields += 1;
                    content_modified = true;
                }

                new_content.push(block);
            }

            if content_modified {
                applied = true;
            }
            *content = new_content;
        }

        // Fallback: if top-level thinking is enabled, but the final assistant message doesn't start
        // with thinking/redacted_thinking AND contains tool_use, remove top-level thinking to avoid
        // Anthropic 400 "Expected thinking..., but found tool_use".
        if thinking_enabled {
            let last_assistant_content = messages.iter().rev().find_map(|msg| {
                let msg_obj = msg.as_object()?;
                if msg_obj.get("role").and_then(|v| v.as_str()) != Some("assistant") {
                    return None;
                }
                msg_obj.get("content").and_then(|v| v.as_array())
            });

            if let Some(content) = last_assistant_content {
                if let Some(first_block) = content.first() {
                    let first_block_type = first_block
                        .as_object()
                        .and_then(|obj| obj.get("type"))
                        .and_then(|v| v.as_str());

                    let missing_thinking_prefix = first_block_type != Some("thinking")
                        && first_block_type != Some("redacted_thinking");

                    if missing_thinking_prefix {
                        let has_tool_use = content.iter().any(|block| {
                            block
                                .as_object()
                                .and_then(|obj| obj.get("type"))
                                .and_then(|v| v.as_str())
                                == Some("tool_use")
                        });

                        if has_tool_use {
                            should_remove_top_level_thinking = true;
                        }
                    }
                }
            }
        }
    }

    if should_remove_top_level_thinking && message_obj.remove("thinking").is_some() {
        removed_top_level_thinking = true;
        applied = true;
    }

    ThinkingSignatureRectifierResult {
        applied,
        removed_thinking_blocks,
        removed_redacted_thinking_blocks,
        removed_signature_fields,
        removed_top_level_thinking,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detect_trigger_invalid_signature_in_thinking_block() {
        let trigger =
            detect_trigger("messages.1.content.0: Invalid `signature` in `thinking` block");
        assert_eq!(trigger, Some(TRIGGER_INVALID_SIGNATURE_IN_THINKING_BLOCK));

        let trigger2 = detect_trigger("Messages.1.Content.0: invalid signature in thinking block");
        assert_eq!(trigger2, Some(TRIGGER_INVALID_SIGNATURE_IN_THINKING_BLOCK));
    }

    #[test]
    fn detect_trigger_missing_thinking_prefix() {
        let trigger = detect_trigger(
            "messages.69.content.0.type: Expected `thinking` or `redacted_thinking`, but found `tool_use`. When `thinking` is enabled, a final `assistant` message must start with a thinking block (preceeding the lastmost set of `tool_use` and `tool_result` blocks). To avoid this requirement, disable `thinking`.",
        );
        assert_eq!(
            trigger,
            Some(TRIGGER_ASSISTANT_MESSAGE_MUST_START_WITH_THINKING)
        );
    }

    #[test]
    fn detect_trigger_invalid_request_variants() {
        assert_eq!(detect_trigger("非法请求"), Some(TRIGGER_INVALID_REQUEST));
        assert_eq!(
            detect_trigger("illegal request format"),
            Some(TRIGGER_INVALID_REQUEST)
        );
        assert_eq!(
            detect_trigger("invalid request: malformed JSON"),
            Some(TRIGGER_INVALID_REQUEST)
        );
    }

    #[test]
    fn detect_trigger_unrelated_error() {
        assert_eq!(detect_trigger("Request timeout"), None);
    }

    #[test]
    fn rectify_removes_thinking_blocks_and_signature_fields() {
        let mut message = json!({
            "model": "claude-test",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        { "type": "thinking", "thinking": "t", "signature": "sig_thinking" },
                        { "type": "text", "text": "hello", "signature": "sig_text_should_remove" },
                        { "type": "tool_use", "id": "toolu_1", "name": "WebSearch", "input": { "query": "q" }, "signature": "sig_tool_should_remove" },
                        { "type": "redacted_thinking", "data": "r", "signature": "sig_redacted" }
                    ]
                },
                {
                    "role": "user",
                    "content": [ { "type": "text", "text": "hi" } ]
                }
            ]
        });

        let result = rectify_anthropic_request_message(&mut message);
        assert!(result.applied);
        assert_eq!(result.removed_thinking_blocks, 1);
        assert_eq!(result.removed_redacted_thinking_blocks, 1);
        assert_eq!(result.removed_signature_fields, 2);

        let content = message["messages"][0]["content"]
            .as_array()
            .expect("content should be array");
        let types: Vec<_> = content
            .iter()
            .map(|v| v["type"].as_str().unwrap_or(""))
            .collect();
        assert_eq!(types, vec!["text", "tool_use"]);
        assert!(content[0].get("signature").is_none());
        assert!(content[1].get("signature").is_none());
    }

    #[test]
    fn rectify_no_messages_should_not_modify() {
        let mut message = json!({ "model": "claude-test" });
        let result = rectify_anthropic_request_message(&mut message);
        assert!(!result.applied);
        assert_eq!(result.removed_thinking_blocks, 0);
        assert_eq!(result.removed_redacted_thinking_blocks, 0);
        assert_eq!(result.removed_signature_fields, 0);
    }

    #[test]
    fn rectify_removes_top_level_thinking_when_tool_use_without_thinking_prefix() {
        let mut message = json!({
            "model": "claude-test",
            "thinking": { "type": "enabled", "budget_tokens": 1024 },
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        { "type": "tool_use", "id": "toolu_1", "name": "WebSearch", "input": { "query": "q" } }
                    ]
                },
                {
                    "role": "user",
                    "content": [ { "type": "tool_result", "tool_use_id": "toolu_1", "content": "ok" } ]
                }
            ]
        });

        let result = rectify_anthropic_request_message(&mut message);
        assert!(result.applied);
        assert!(result.removed_top_level_thinking);
        assert!(message.get("thinking").is_none());
    }
}
