import { invokeTauriOrNull } from "./tauriInvoke";
import type { CliKey } from "./providers";

type Listener = () => void;

const listeners = new Set<Listener>();

function emitUpdated() {
  for (const listener of listeners) listener();
}

export function subscribeModelPricesUpdated(listener: Listener) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function notifyModelPricesUpdated() {
  emitUpdated();
}

export type ModelPricesSyncReport = {
  status: "updated" | "not_modified" | string;
  inserted: number;
  updated: number;
  skipped: number;
  total: number;
};

export type ModelPriceSummary = {
  id: number;
  cli_key: CliKey;
  model: string;
  currency: string;
  created_at: number;
  updated_at: number;
};

export async function modelPricesList(cliKey: CliKey) {
  return invokeTauriOrNull<ModelPriceSummary[]>("model_prices_list", { cliKey });
}

export async function modelPricesSyncBasellm(force = false) {
  return invokeTauriOrNull<ModelPricesSyncReport>("model_prices_sync_basellm", {
    force,
  });
}
