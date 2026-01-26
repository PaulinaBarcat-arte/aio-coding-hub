use super::*;

#[test]
fn claude_models_no_config_keeps_original() {
    let models = ClaudeModels::default();
    assert_eq!(
        models.map_model("claude-sonnet-4", false),
        "claude-sonnet-4"
    );
}

#[test]
fn claude_models_thinking_prefers_reasoning_model() {
    let models = ClaudeModels {
        main_model: Some("glm-main".to_string()),
        reasoning_model: Some("glm-thinking".to_string()),
        haiku_model: Some("glm-haiku".to_string()),
        sonnet_model: Some("glm-sonnet".to_string()),
        opus_model: Some("glm-opus".to_string()),
    }
    .normalized();

    assert_eq!(models.map_model("claude-sonnet-4", true), "glm-thinking");
}

#[test]
fn claude_models_type_slot_selected_by_substring() {
    let models = ClaudeModels {
        main_model: Some("glm-main".to_string()),
        haiku_model: Some("glm-haiku".to_string()),
        sonnet_model: Some("glm-sonnet".to_string()),
        opus_model: Some("glm-opus".to_string()),
        ..Default::default()
    }
    .normalized();

    assert_eq!(models.map_model("claude-haiku-4", false), "glm-haiku");
    assert_eq!(models.map_model("claude-sonnet-4", false), "glm-sonnet");
    assert_eq!(models.map_model("claude-opus-4", false), "glm-opus");
}

#[test]
fn claude_models_falls_back_to_main_model() {
    let models = ClaudeModels {
        main_model: Some("glm-main".to_string()),
        ..Default::default()
    }
    .normalized();

    assert_eq!(models.map_model("some-unknown-model", false), "glm-main");
}
