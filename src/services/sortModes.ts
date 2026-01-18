import { invokeTauriOrNull } from "./tauriInvoke";
import type { CliKey } from "./providers";

export type SortModeSummary = {
  id: number;
  name: string;
  created_at: number;
  updated_at: number;
};

export type SortModeActiveRow = {
  cli_key: CliKey;
  mode_id: number | null;
  updated_at: number;
};

export async function sortModesList() {
  return invokeTauriOrNull<SortModeSummary[]>("sort_modes_list");
}

export async function sortModeCreate(input: { name: string }) {
  return invokeTauriOrNull<SortModeSummary>("sort_mode_create", {
    name: input.name,
  });
}

export async function sortModeRename(input: { mode_id: number; name: string }) {
  return invokeTauriOrNull<SortModeSummary>("sort_mode_rename", {
    modeId: input.mode_id,
    name: input.name,
  });
}

export async function sortModeDelete(input: { mode_id: number }) {
  return invokeTauriOrNull<boolean>("sort_mode_delete", {
    modeId: input.mode_id,
  });
}

export async function sortModeActiveList() {
  return invokeTauriOrNull<SortModeActiveRow[]>("sort_mode_active_list");
}

export async function sortModeActiveSet(input: { cli_key: CliKey; mode_id: number | null }) {
  return invokeTauriOrNull<SortModeActiveRow>("sort_mode_active_set", {
    cliKey: input.cli_key,
    modeId: input.mode_id,
  });
}

export async function sortModeProvidersList(input: { mode_id: number; cli_key: CliKey }) {
  return invokeTauriOrNull<number[]>("sort_mode_providers_list", {
    modeId: input.mode_id,
    cliKey: input.cli_key,
  });
}

export async function sortModeProvidersSetOrder(input: {
  mode_id: number;
  cli_key: CliKey;
  ordered_provider_ids: number[];
}) {
  return invokeTauriOrNull<number[]>("sort_mode_providers_set_order", {
    modeId: input.mode_id,
    cliKey: input.cli_key,
    orderedProviderIds: input.ordered_provider_ids,
  });
}
