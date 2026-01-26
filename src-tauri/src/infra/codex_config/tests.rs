use super::*;

#[test]
fn patch_creates_features_table_and_preserves_other_tables() {
    let input = r#"# header

[mcp_servers.exa]
type = "stdio"

"#;

    let out = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: None,
            features_unified_exec: None,
            features_shell_snapshot: Some(true),
            features_apply_patch_freeform: None,
            features_web_search_request: Some(true),
            features_shell_tool: None,
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out).expect("utf8");
    assert!(s.contains("[mcp_servers.exa]"), "{s}");
    assert!(s.contains("[features]"), "{s}");
    assert!(s.contains("shell_snapshot = true"), "{s}");
    assert!(s.contains("web_search_request = true"), "{s}");
}

#[test]
fn patch_updates_dotted_tui_keys_without_creating_table() {
    let input = r#"tui.animations = true
tui.show_tooltips = true

[other]
foo = "bar"
"#;

    let out = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: Some(false),
            tui_alternate_screen: None,
            tui_show_tooltips: Some(false),
            tui_scroll_invert: None,
            features_unified_exec: None,
            features_shell_snapshot: None,
            features_apply_patch_freeform: None,
            features_web_search_request: None,
            features_shell_tool: None,
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out).expect("utf8");
    assert!(s.contains("tui.animations = false"), "{s}");
    assert!(s.contains("tui.show_tooltips = false"), "{s}");
    assert!(!s.contains("[tui]"), "{s}");
}

#[test]
fn patch_preserves_existing_features_when_setting_another() {
    let input = r#"[features]
web_search_request = true
"#;

    let out = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: None,
            features_unified_exec: None,
            features_shell_snapshot: Some(true),
            features_apply_patch_freeform: None,
            features_web_search_request: None,
            features_shell_tool: None,
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out).expect("utf8");
    assert!(s.contains("web_search_request = true"), "{s}");
    assert!(s.contains("shell_snapshot = true"), "{s}");
}

#[test]
fn patch_deletes_default_false_feature_when_disabled() {
    let input = r#"[features]
shell_snapshot = true
"#;

    let out = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: None,
            features_unified_exec: None,
            features_shell_snapshot: Some(false),
            features_apply_patch_freeform: None,
            features_web_search_request: None,
            features_shell_tool: None,
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out).expect("utf8");
    assert!(!s.contains("shell_snapshot ="), "{s}");
}

#[test]
fn patch_writes_true_when_feature_enabled() {
    let input = r#"[features]
shell_tool = false
"#;

    let out = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: None,
            features_unified_exec: None,
            features_shell_snapshot: None,
            features_apply_patch_freeform: None,
            features_web_search_request: None,
            features_shell_tool: Some(true),
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out).expect("utf8");
    assert!(!s.contains("shell_tool = false"), "{s}");
    assert!(s.contains("shell_tool = true"), "{s}");
}

#[test]
fn patch_deletes_feature_when_disabled() {
    let input = r#"[features]
shell_tool = true
"#;

    let out = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: None,
            features_unified_exec: None,
            features_shell_snapshot: None,
            features_apply_patch_freeform: None,
            features_web_search_request: None,
            features_shell_tool: Some(false),
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out).expect("utf8");
    assert!(!s.contains("shell_tool ="), "{s}");
}

#[test]
fn patch_preserves_other_tui_keys_when_updating_one() {
    let input = r#"[tui]
animations = true
show_tooltips = true
scroll_invert = false
"#;

    let out = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: Some(true),
            features_unified_exec: None,
            features_shell_snapshot: None,
            features_apply_patch_freeform: None,
            features_web_search_request: None,
            features_shell_tool: None,
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out).expect("utf8");
    assert!(s.contains("animations = true"), "{s}");
    assert!(s.contains("show_tooltips = true"), "{s}");
    assert!(s.contains("scroll_invert = true"), "{s}");
}

#[test]
fn patch_preserves_other_history_keys_when_updating_one() {
    let input = r#"[history]
persistence = "save-all"
max_bytes = 123
"#;

    let out = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: Some(456),
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: None,
            features_unified_exec: None,
            features_shell_snapshot: None,
            features_apply_patch_freeform: None,
            features_web_search_request: None,
            features_shell_tool: None,
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out).expect("utf8");
    assert!(s.contains("persistence = \"save-all\""), "{s}");
    assert!(s.contains("max_bytes = 456"), "{s}");
}

#[test]
fn patch_compacts_blank_lines_in_features_table() {
    let input = r#"[features]

shell_snapshot = true

web_search_request = true



[other]
foo = "bar"
"#;

    let out1 = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: None,
            features_unified_exec: None,
            features_shell_snapshot: None,
            features_apply_patch_freeform: None,
            features_web_search_request: None,
            features_shell_tool: Some(true),
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let out2 = patch_config_toml(
        Some(out1),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: None,
            features_unified_exec: Some(true),
            features_shell_snapshot: None,
            features_apply_patch_freeform: None,
            features_web_search_request: None,
            features_shell_tool: None,
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out2).expect("utf8");
    assert!(
        s.contains(
            "[features]\n\
shell_snapshot = true\n\
web_search_request = true\n\
unified_exec = true\n\
shell_tool = true\n\n\
[other]\n"
        ),
        "{s}"
    );
    assert!(!s.contains("[features]\n\n"), "{s}");
    assert!(!s.contains("true\n\nweb_search_request"), "{s}");
    assert!(!s.contains("true\n\nshell_tool"), "{s}");
    assert!(!s.contains("true\n\nunified_exec"), "{s}");
}

#[test]
fn patch_compacts_blank_lines_across_entire_file() {
    let input = r#"approval_policy = "never"


preferred_auth_method = "apikey"


[features]


shell_snapshot = true


[mcp_servers.exa]
type = "stdio"
"#;

    let out = patch_config_toml(
        Some(input.as_bytes().to_vec()),
        CodexConfigPatch {
            model: None,
            approval_policy: None,
            sandbox_mode: None,
            model_reasoning_effort: None,
            file_opener: None,
            hide_agent_reasoning: None,
            show_raw_agent_reasoning: None,
            history_persistence: None,
            history_max_bytes: None,
            sandbox_workspace_write_network_access: None,
            tui_animations: None,
            tui_alternate_screen: None,
            tui_show_tooltips: None,
            tui_scroll_invert: None,
            features_unified_exec: None,
            features_shell_snapshot: None,
            features_apply_patch_freeform: None,
            features_web_search_request: Some(true),
            features_shell_tool: None,
            features_exec_policy: None,
            features_experimental_windows_sandbox: None,
            features_elevated_windows_sandbox: None,
            features_remote_compaction: None,
            features_remote_models: None,
            features_powershell_utf8: None,
            features_child_agents_md: None,
        },
    )
    .expect("patch_config_toml");

    let s = String::from_utf8(out).expect("utf8");
    assert!(
        s.contains(
            "approval_policy = \"never\"\n\
preferred_auth_method = \"apikey\"\n\n\
[features]\n\
shell_snapshot = true\n\
web_search_request = true\n\n\
[mcp_servers.exa]\n\
type = \"stdio\"\n"
        ),
        "{s}"
    );
    assert!(!s.contains("\n\n\n"), "{s}");
}
