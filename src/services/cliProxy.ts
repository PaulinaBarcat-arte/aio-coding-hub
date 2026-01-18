import { invokeTauriOrNull } from "./tauriInvoke";
import type { CliKey } from "./providers";

export type CliProxyStatus = {
  cli_key: CliKey;
  enabled: boolean;
  base_origin: string | null;
};

export type CliProxyResult = {
  trace_id: string;
  cli_key: CliKey;
  enabled: boolean;
  ok: boolean;
  error_code: string | null;
  message: string;
  base_origin: string | null;
};

export async function cliProxyStatusAll() {
  return invokeTauriOrNull<CliProxyStatus[]>("cli_proxy_status_all");
}

export async function cliProxySetEnabled(input: { cli_key: CliKey; enabled: boolean }) {
  return invokeTauriOrNull<CliProxyResult>("cli_proxy_set_enabled", {
    cliKey: input.cli_key,
    enabled: input.enabled,
  });
}

export async function cliProxySyncEnabled(base_origin: string) {
  return invokeTauriOrNull<CliProxyResult[]>("cli_proxy_sync_enabled", {
    baseOrigin: base_origin,
  });
}
