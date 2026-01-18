import { invokeTauriOrNull } from "./tauriInvoke";
import type { AppSettings } from "./settings";

export type GatewayRectifierSettingsPatch = {
  intercept_anthropic_warmup_requests: boolean;
  enable_thinking_signature_rectifier: boolean;
  enable_response_fixer: boolean;
  response_fixer_fix_encoding: boolean;
  response_fixer_fix_sse_format: boolean;
  response_fixer_fix_truncated_json: boolean;
};

export async function settingsGatewayRectifierSet(input: GatewayRectifierSettingsPatch) {
  return invokeTauriOrNull<AppSettings>("settings_gateway_rectifier_set", {
    interceptAnthropicWarmupRequests: input.intercept_anthropic_warmup_requests,
    enableThinkingSignatureRectifier: input.enable_thinking_signature_rectifier,
    enableResponseFixer: input.enable_response_fixer,
    responseFixerFixEncoding: input.response_fixer_fix_encoding,
    responseFixerFixSseFormat: input.response_fixer_fix_sse_format,
    responseFixerFixTruncatedJson: input.response_fixer_fix_truncated_json,
  });
}
