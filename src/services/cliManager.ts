import { invokeTauriOrNull } from "./tauriInvoke";

export type ClaudeCliInfo = {
  found: boolean;
  executable_path: string | null;
  version: string | null;
  error: string | null;
  shell: string | null;
  resolved_via: string;
  config_dir: string;
  settings_path: string;
  mcp_timeout_ms: number | null;
  disable_error_reporting: boolean;
};

export type SimpleCliInfo = {
  found: boolean;
  executable_path: string | null;
  version: string | null;
  error: string | null;
  shell: string | null;
  resolved_via: string;
};

export type ClaudeEnvState = {
  config_dir: string;
  settings_path: string;
  mcp_timeout_ms: number | null;
  disable_error_reporting: boolean;
};

export type ClaudeSettingsState = {
  config_dir: string;
  settings_path: string;
  exists: boolean;

  model: string | null;
  output_style: string | null;
  language: string | null;
  always_thinking_enabled: boolean | null;

  show_turn_duration: boolean | null;
  spinner_tips_enabled: boolean | null;
  terminal_progress_bar_enabled: boolean | null;
  respect_gitignore: boolean | null;

  disable_all_hooks: boolean | null;

  permissions_allow: string[];
  permissions_ask: string[];
  permissions_deny: string[];

  env_mcp_timeout_ms: number | null;
  env_mcp_tool_timeout_ms: number | null;
  env_disable_error_reporting: boolean;
  env_disable_telemetry: boolean;
  env_disable_background_tasks: boolean;
  env_disable_terminal_title: boolean;
  env_claude_bash_no_login: boolean;
  env_claude_code_attribution_header_disabled: boolean;
  env_claude_code_blocking_limit_override: number | null;
  env_claude_autocompact_pct_override: number | null;
  env_claude_code_max_output_tokens: number | null;
};

export type ClaudeSettingsPatch = Partial<{
  model: string;
  output_style: string;
  language: string;
  always_thinking_enabled: boolean;

  show_turn_duration: boolean;
  spinner_tips_enabled: boolean;
  terminal_progress_bar_enabled: boolean;
  respect_gitignore: boolean;

  disable_all_hooks: boolean;

  permissions_allow: string[];
  permissions_ask: string[];
  permissions_deny: string[];

  env_mcp_timeout_ms: number;
  env_mcp_tool_timeout_ms: number;
  env_disable_error_reporting: boolean;
  env_disable_telemetry: boolean;
  env_disable_background_tasks: boolean;
  env_disable_terminal_title: boolean;
  env_claude_bash_no_login: boolean;
  env_claude_code_attribution_header_disabled: boolean;
  env_claude_code_blocking_limit_override: number;
  env_claude_autocompact_pct_override: number;
  env_claude_code_max_output_tokens: number;
}>;

export type CodexConfigState = {
  config_dir: string;
  config_path: string;
  can_open_config_dir: boolean;
  exists: boolean;

  model: string | null;
  approval_policy: string | null;
  sandbox_mode: string | null;
  model_reasoning_effort: string | null;
  file_opener: string | null;
  hide_agent_reasoning: boolean | null;
  show_raw_agent_reasoning: boolean | null;

  history_persistence: string | null;
  history_max_bytes: number | null;

  sandbox_workspace_write_network_access: boolean | null;

  tui_animations: boolean | null;
  tui_alternate_screen: string | null;
  tui_show_tooltips: boolean | null;
  tui_scroll_invert: boolean | null;

  features_unified_exec: boolean | null;
  features_shell_snapshot: boolean | null;
  features_apply_patch_freeform: boolean | null;
  features_web_search_request: boolean | null;
  features_shell_tool: boolean | null;
  features_exec_policy: boolean | null;
  features_experimental_windows_sandbox: boolean | null;
  features_elevated_windows_sandbox: boolean | null;
  features_remote_compaction: boolean | null;
  features_remote_models: boolean | null;
  features_powershell_utf8: boolean | null;
  features_child_agents_md: boolean | null;
};

export type CodexConfigPatch = Partial<{
  model: string;
  approval_policy: string;
  sandbox_mode: string;
  model_reasoning_effort: string;
  file_opener: string;
  hide_agent_reasoning: boolean;
  show_raw_agent_reasoning: boolean;

  history_persistence: string;
  history_max_bytes: number;

  sandbox_workspace_write_network_access: boolean;

  tui_animations: boolean;
  tui_alternate_screen: string;
  tui_show_tooltips: boolean;
  tui_scroll_invert: boolean;

  features_unified_exec: boolean;
  features_shell_snapshot: boolean;
  features_apply_patch_freeform: boolean;
  features_web_search_request: boolean;
  features_shell_tool: boolean;
  features_exec_policy: boolean;
  features_experimental_windows_sandbox: boolean;
  features_elevated_windows_sandbox: boolean;
  features_remote_compaction: boolean;
  features_remote_models: boolean;
  features_powershell_utf8: boolean;
  features_child_agents_md: boolean;
}>;

export async function cliManagerClaudeInfoGet() {
  return invokeTauriOrNull<ClaudeCliInfo>("cli_manager_claude_info_get");
}

export async function cliManagerCodexInfoGet() {
  return invokeTauriOrNull<SimpleCliInfo>("cli_manager_codex_info_get");
}

export async function cliManagerCodexConfigGet() {
  return invokeTauriOrNull<CodexConfigState>("cli_manager_codex_config_get");
}

export async function cliManagerCodexConfigSet(patch: CodexConfigPatch) {
  return invokeTauriOrNull<CodexConfigState>("cli_manager_codex_config_set", { patch });
}

export async function cliManagerGeminiInfoGet() {
  return invokeTauriOrNull<SimpleCliInfo>("cli_manager_gemini_info_get");
}

export async function cliManagerClaudeEnvSet(input: {
  mcp_timeout_ms: number | null;
  disable_error_reporting: boolean;
}) {
  return invokeTauriOrNull<ClaudeEnvState>("cli_manager_claude_env_set", {
    mcpTimeoutMs: input.mcp_timeout_ms,
    disableErrorReporting: input.disable_error_reporting,
  });
}

export async function cliManagerClaudeSettingsGet() {
  return invokeTauriOrNull<ClaudeSettingsState>("cli_manager_claude_settings_get");
}

export async function cliManagerClaudeSettingsSet(patch: ClaudeSettingsPatch) {
  return invokeTauriOrNull<ClaudeSettingsState>("cli_manager_claude_settings_set", { patch });
}
