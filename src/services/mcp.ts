import { invokeTauriOrNull } from "./tauriInvoke";
import type { CliKey } from "./providers";

export type McpTransport = "stdio" | "http";

export type McpServerSummary = {
  id: number;
  server_key: string;
  name: string;
  transport: McpTransport;
  command: string | null;
  args: string[];
  env: Record<string, string>;
  cwd: string | null;
  url: string | null;
  headers: Record<string, string>;
  enabled_claude: boolean;
  enabled_codex: boolean;
  enabled_gemini: boolean;
  created_at: number;
  updated_at: number;
};

export type McpImportServer = {
  server_key: string;
  name: string;
  transport: McpTransport;
  command: string | null;
  args: string[];
  env: Record<string, string>;
  cwd: string | null;
  url: string | null;
  headers: Record<string, string>;
  enabled_claude: boolean;
  enabled_codex: boolean;
  enabled_gemini: boolean;
};

export type McpParseResult = {
  servers: McpImportServer[];
};

export type McpImportReport = {
  inserted: number;
  updated: number;
};

export async function mcpServersList() {
  return invokeTauriOrNull<McpServerSummary[]>("mcp_servers_list");
}

export async function mcpServerUpsert(input: {
  server_id?: number | null;
  server_key: string;
  name: string;
  transport: McpTransport;
  command?: string | null;
  args?: string[];
  env?: Record<string, string>;
  cwd?: string | null;
  url?: string | null;
  headers?: Record<string, string>;
  enabled_claude: boolean;
  enabled_codex: boolean;
  enabled_gemini: boolean;
}) {
  return invokeTauriOrNull<McpServerSummary>("mcp_server_upsert", {
    serverId: input.server_id ?? null,
    serverKey: input.server_key,
    name: input.name,
    transport: input.transport,
    command: input.command ?? null,
    args: input.args ?? [],
    env: input.env ?? {},
    cwd: input.cwd ?? null,
    url: input.url ?? null,
    headers: input.headers ?? {},
    enabledClaude: input.enabled_claude,
    enabledCodex: input.enabled_codex,
    enabledGemini: input.enabled_gemini,
  });
}

export async function mcpServerSetEnabled(input: {
  server_id: number;
  cli_key: CliKey;
  enabled: boolean;
}) {
  return invokeTauriOrNull<McpServerSummary>("mcp_server_set_enabled", {
    serverId: input.server_id,
    cliKey: input.cli_key,
    enabled: input.enabled,
  });
}

export async function mcpServerDelete(serverId: number) {
  return invokeTauriOrNull<boolean>("mcp_server_delete", { serverId });
}

export async function mcpParseJson(jsonText: string) {
  return invokeTauriOrNull<McpParseResult>("mcp_parse_json", { jsonText });
}

export async function mcpImportServers(servers: McpImportServer[]) {
  return invokeTauriOrNull<McpImportReport>("mcp_import_servers", { servers });
}
