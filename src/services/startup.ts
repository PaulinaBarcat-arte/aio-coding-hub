import { logToConsole } from "./consoleLog";
import { modelPricesSyncBasellm, notifyModelPricesUpdated } from "./modelPrices";
import { promptsDefaultSyncFromFiles } from "./prompts";

const STORAGE_KEY_MODEL_PRICES_SYNCED = "startup.modelPrices.basellmSyncedAt";

let modelPricesSyncStarted = false;
let defaultPromptsSyncPromise: Promise<void> | null = null;

function hasSyncedModelPricesOnce(): boolean {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_MODEL_PRICES_SYNCED);
    return !!raw;
  } catch {
    return false;
  }
}

function markModelPricesSyncedOnce() {
  try {
    localStorage.setItem(STORAGE_KEY_MODEL_PRICES_SYNCED, String(Date.now()));
  } catch {}
}

export async function startupSyncModelPricesOnce(): Promise<void> {
  if (modelPricesSyncStarted) return;
  modelPricesSyncStarted = true;

  if (hasSyncedModelPricesOnce()) return;

  try {
    const report = await modelPricesSyncBasellm(false);
    if (!report) return;

    markModelPricesSyncedOnce();
    notifyModelPricesUpdated();
    logToConsole("info", "初始化：模型定价同步完成", {
      status: report.status,
      inserted: report.inserted,
      updated: report.updated,
      skipped: report.skipped,
      total: report.total,
    });
  } catch (err) {
    logToConsole("error", "初始化：模型定价同步失败", { error: String(err) });
  }
}

function summarizeDefaultPromptSyncActions(items: { action: string }[]) {
  const summary: Record<string, number> = {};
  for (const item of items) {
    const key = String(item.action || "unknown");
    summary[key] = (summary[key] ?? 0) + 1;
  }
  return summary;
}

export function startupSyncDefaultPromptsFromFilesOncePerSession(): Promise<void> {
  if (defaultPromptsSyncPromise) return defaultPromptsSyncPromise;

  defaultPromptsSyncPromise = (async () => {
    try {
      const report = await promptsDefaultSyncFromFiles();
      if (!report) return;

      const summary = summarizeDefaultPromptSyncActions(report.items);
      const hasError = report.items.some((it) => it.action === "error");

      logToConsole(hasError ? "error" : "info", "初始化：default 提示词与本机文件同步完成", {
        summary,
        items: report.items,
      });
    } catch (err) {
      logToConsole("error", "初始化：default 提示词与本机文件同步失败", {
        error: String(err),
      });
    }
  })();

  return defaultPromptsSyncPromise;
}
