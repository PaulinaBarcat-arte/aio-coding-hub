//! Usage: Detect non-retryable client input errors from upstream error bodies (align with
//! claude-code-hub's error rules approach).

/// Limit how much of the upstream error body we scan (defensive against huge error payloads).
const MAX_SCAN_BYTES: usize = 64 * 1024;

/// Returns whether we should attempt to scan the upstream error body to detect non-retryable
/// client input errors.
///
/// This prevents wasting work on statuses where:
/// - failover is always preferred (e.g. 402/404), or
/// - we already have dedicated retry/backoff logic (e.g. 408/429), or
/// - the body is too large to scan without reading the entire response into memory.
pub(super) fn should_attempt_non_retryable_match(
    status: reqwest::StatusCode,
    content_length: Option<u64>,
) -> bool {
    if !status.is_client_error() {
        return false;
    }

    match status.as_u16() {
        401 | 402 | 403 | 404 | 408 | 429 => return false,
        _ => {}
    }

    if let Some(len) = content_length {
        if len > MAX_SCAN_BYTES as u64 {
            return false;
        }
    }

    true
}

struct Rule {
    id: &'static str,
    any_of: &'static [&'static str],
    all_of: &'static [&'static str],
}

fn matches_rule(haystack_lower: &str, rule: &Rule) -> bool {
    if !rule
        .all_of
        .iter()
        .all(|needle| haystack_lower.contains(needle))
    {
        return false;
    }
    if rule.any_of.is_empty() {
        return true;
    }
    rule.any_of
        .iter()
        .any(|needle| haystack_lower.contains(needle))
}

// NOTE: These rules are a high-signal subset derived from claude-code-hub defaults.
//       We intentionally keep this list small and conservative to avoid false positives.
const NON_RETRYABLE_RULES: &[Rule] = &[
    Rule {
        id: "parameter_alt_sse",
        any_of: &["alt=sse"],
        all_of: &[],
    },
    Rule {
        id: "prompt_limit",
        any_of: &["prompt is too long", "prompt too long"],
        all_of: &[],
    },
    Rule {
        id: "input_limit",
        any_of: &["input is too long", "content_length_exceeds_threshold"],
        all_of: &[],
    },
    Rule {
        id: "token_limit",
        any_of: &["max_tokens", "maximum tokens", "max tokens"],
        all_of: &["exceed"],
    },
    Rule {
        id: "context_limit",
        any_of: &[
            "context window",
            "context length",
            "pricing plan does not include long context",
        ],
        all_of: &[],
    },
    Rule {
        id: "content_filter",
        any_of: &["content filter", "blocked by content filter"],
        all_of: &[],
    },
    Rule {
        id: "validation_exception",
        any_of: &["validationexception"],
        all_of: &[],
    },
    Rule {
        id: "validation_tool_use_ids_unique",
        any_of: &["tool_use", "tool names must be unique"],
        all_of: &["must be unique"],
    },
    Rule {
        id: "validation_message_non_empty",
        any_of: &["all messages must have non-empty content"],
        all_of: &[],
    },
    Rule {
        id: "validation_server_tool_use_id",
        any_of: &["server_tool_use", "srvtoolu_"],
        all_of: &["match pattern"],
    },
    Rule {
        id: "validation_tool_use_id_in_tool_result",
        any_of: &["tool_use_id", "tool_result"],
        all_of: &["unexpected"],
    },
    Rule {
        id: "validation_tool_result_missing_tool_use",
        any_of: &["tool_result"],
        all_of: &["tool_use", "corresponding"],
    },
    Rule {
        id: "validation_tool_use_missing_tool_result",
        any_of: &["tool_use"],
        all_of: &["tool_result", "next message"],
    },
    Rule {
        id: "parameter_missing_model",
        any_of: &["model is required"],
        all_of: &[],
    },
    Rule {
        id: "parameter_missing_or_extra",
        any_of: &[
            "missing required parameter",
            "extra inputs",
            "not permitted",
        ],
        all_of: &[],
    },
    Rule {
        id: "signature_field_required",
        any_of: &["field required"],
        all_of: &["signature"],
    },
    Rule {
        id: "pdf_limit",
        any_of: &["pdf has too many pages"],
        all_of: &[],
    },
    Rule {
        id: "media_limit",
        any_of: &["too much media"],
        all_of: &[],
    },
    Rule {
        id: "thinking_error_missing_block_prefix",
        any_of: &["must start with a thinking block"],
        all_of: &[],
    },
    Rule {
        id: "thinking_error_expected_block",
        any_of: &["expected"],
        all_of: &["thinking", "tool_use"],
    },
    Rule {
        id: "cache_limit",
        any_of: &["cache_control"],
        all_of: &["block", "limit"],
    },
    Rule {
        id: "image_size_limit",
        any_of: &["image exceeds"],
        all_of: &["maximum", "bytes"],
    },
    Rule {
        id: "thinking_error_reasoning_effort",
        any_of: &["unsupported value"],
        all_of: &["supported values", "model"],
    },
];

/// Returns a matched rule id if the upstream error should be treated as a non-retryable client
/// input error (abort, no provider switch).
///
/// This is used to avoid wasting failover attempts and to prevent circuit/cooldown pollution
/// on errors caused by invalid client requests (prompt/token limits, content filter, schema
/// validation, etc.).
pub(super) fn match_non_retryable_client_error(
    _cli_key: &str,
    status: reqwest::StatusCode,
    body: &[u8],
) -> Option<&'static str> {
    if !status.is_client_error() {
        return None;
    }
    if matches!(status.as_u16(), 401 | 402 | 403 | 404 | 408 | 429) {
        return None;
    }
    if body.is_empty() {
        return None;
    }

    let scan = if body.len() > MAX_SCAN_BYTES {
        &body[..MAX_SCAN_BYTES]
    } else {
        body
    };

    let haystack_lower = String::from_utf8_lossy(scan).to_ascii_lowercase();
    for rule in NON_RETRYABLE_RULES {
        if matches_rule(&haystack_lower, rule) {
            return Some(rule.id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::match_non_retryable_client_error;

    #[test]
    fn matches_prompt_limit() {
        let body = b"{\"error\":{\"message\":\"Prompt is too long. Maximum tokens exceeded\"}}";
        assert_eq!(
            match_non_retryable_client_error("claude", reqwest::StatusCode::BAD_REQUEST, body),
            Some("prompt_limit")
        );
    }

    #[test]
    fn does_not_match_subscription_402_message() {
        let body = b"No available asset for API access, please purchase a subscription";
        assert_eq!(
            match_non_retryable_client_error("claude", reqwest::StatusCode::PAYMENT_REQUIRED, body),
            None
        );
    }

    #[test]
    fn does_not_match_on_402_even_if_body_contains_limit_text() {
        let body = b"Prompt is too long. Maximum tokens exceeded";
        assert_eq!(
            match_non_retryable_client_error("claude", reqwest::StatusCode::PAYMENT_REQUIRED, body),
            None
        );
    }
}
