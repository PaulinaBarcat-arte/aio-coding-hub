import { invokeTauriOrNull } from "./tauriInvoke";
import type { AppSettings } from "./settings";

export async function settingsCircuitBreakerNoticeSet(enable: boolean) {
  return invokeTauriOrNull<AppSettings>("settings_circuit_breaker_notice_set", {
    enableCircuitBreakerNotice: enable,
  });
}
