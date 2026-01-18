import { invokeTauriOrNull } from "./tauriInvoke";

export type ClaudeModelValidationRunRow = {
  id: number;
  provider_id: number;
  created_at: number;
  request_json: string;
  result_json: string;
};

export async function claudeValidationHistoryList(input: { provider_id: number; limit?: number }) {
  return invokeTauriOrNull<ClaudeModelValidationRunRow[]>("claude_validation_history_list", {
    providerId: input.provider_id,
    limit: input.limit,
  });
}

export async function claudeValidationHistoryClearProvider(input: { provider_id: number }) {
  return invokeTauriOrNull<boolean>("claude_validation_history_clear_provider", {
    providerId: input.provider_id,
  });
}
