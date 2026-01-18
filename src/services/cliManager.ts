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

export async function cliManagerClaudeInfoGet() {
  return invokeTauriOrNull<ClaudeCliInfo>("cli_manager_claude_info_get");
}

export async function cliManagerCodexInfoGet() {
  return invokeTauriOrNull<SimpleCliInfo>("cli_manager_codex_info_get");
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
