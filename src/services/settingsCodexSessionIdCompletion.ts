import { invokeTauriOrNull } from "./tauriInvoke";
import type { AppSettings } from "./settings";

export async function settingsCodexSessionIdCompletionSet(enable: boolean) {
  return invokeTauriOrNull<AppSettings>("settings_codex_session_id_completion_set", {
    enableCodexSessionIdCompletion: enable,
  });
}
