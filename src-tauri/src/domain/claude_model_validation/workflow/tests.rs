use super::build_preserved_assistant_message;

#[test]
fn preserved_assistant_message_contains_thinking_signature_and_optional_text() {
    let msg = build_preserved_assistant_message("  THINK  ", "  SIG  ", "  hello  ");
    assert_eq!(msg.get("role").and_then(|v| v.as_str()), Some("assistant"));

    let content = msg.get("content").and_then(|v| v.as_array()).unwrap();
    assert_eq!(content.len(), 2);

    let thinking = content[0].as_object().unwrap();
    assert_eq!(
        thinking.get("type").and_then(|v| v.as_str()),
        Some("thinking")
    );
    assert_eq!(
        thinking.get("thinking").and_then(|v| v.as_str()),
        Some("THINK")
    );
    assert_eq!(
        thinking.get("signature").and_then(|v| v.as_str()),
        Some("SIG")
    );

    let text = content[1].as_object().unwrap();
    assert_eq!(text.get("type").and_then(|v| v.as_str()), Some("text"));
    assert_eq!(text.get("text").and_then(|v| v.as_str()), Some("hello"));
}

#[test]
fn preserved_assistant_message_omits_text_block_when_empty() {
    let msg = build_preserved_assistant_message("THINK", "SIG", "   ");
    let content = msg.get("content").and_then(|v| v.as_array()).unwrap();
    assert_eq!(content.len(), 1);
}
