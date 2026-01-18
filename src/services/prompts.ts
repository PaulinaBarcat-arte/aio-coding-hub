import { invokeTauriOrNull } from "./tauriInvoke";
import type { CliKey } from "./providers";

export type PromptSummary = {
  id: number;
  cli_key: CliKey;
  name: string;
  content: string;
  enabled: boolean;
  created_at: number;
  updated_at: number;
};

export type DefaultPromptSyncItem = {
  cli_key: CliKey;
  action: "created" | "updated" | "unchanged" | "skipped" | "error";
  message: string | null;
};

export type DefaultPromptSyncReport = {
  items: DefaultPromptSyncItem[];
};

export async function promptsList(cliKey: CliKey) {
  return invokeTauriOrNull<PromptSummary[]>("prompts_list", { cliKey });
}

export async function promptsDefaultSyncFromFiles() {
  return invokeTauriOrNull<DefaultPromptSyncReport>("prompts_default_sync_from_files");
}

export async function promptUpsert(input: {
  prompt_id?: number | null;
  cli_key: CliKey;
  name: string;
  content: string;
  enabled: boolean;
}) {
  return invokeTauriOrNull<PromptSummary>("prompt_upsert", {
    promptId: input.prompt_id ?? null,
    cliKey: input.cli_key,
    name: input.name,
    content: input.content,
    enabled: input.enabled,
  });
}

export async function promptSetEnabled(promptId: number, enabled: boolean) {
  return invokeTauriOrNull<PromptSummary>("prompt_set_enabled", {
    promptId,
    enabled,
  });
}

export async function promptDelete(promptId: number) {
  return invokeTauriOrNull<boolean>("prompt_delete", { promptId });
}
