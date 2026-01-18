import { hasTauriRuntime, invokeTauriOrNull } from "./tauriInvoke";

export type UpdaterCheckUpdate = {
  rid: number;
  version?: string;
  currentVersion?: string;
  date?: string;
  body?: string;
};

export type UpdaterCheckResult = UpdaterCheckUpdate | null;

export type UpdaterDownloadEvent =
  | { event: "started"; data?: { contentLength?: number } }
  | { event: "progress"; data?: { chunkLength?: number } }
  | { event: "finished"; data?: unknown };

function asOptionalString(value: unknown) {
  return typeof value === "string" ? value : undefined;
}

function asOptionalNumber(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

export function parseUpdaterCheckResult(value: unknown): UpdaterCheckResult {
  if (value == null || value === false) return null;
  if (typeof value !== "object") return null;

  const obj = value as Record<string, unknown>;
  const rid = asOptionalNumber(obj.rid);
  if (rid == null) return null;

  return {
    rid,
    version: asOptionalString(obj.version),
    currentVersion: asOptionalString(obj.currentVersion),
    date: asOptionalString(obj.date),
    body: asOptionalString(obj.body),
  };
}

export async function updaterCheck(): Promise<UpdaterCheckResult> {
  const raw = await invokeTauriOrNull<unknown>("plugin:updater|check");
  return parseUpdaterCheckResult(raw);
}

function parseUpdaterDownloadEvent(value: unknown): UpdaterDownloadEvent | null {
  if (value == null || typeof value !== "object") return null;
  const obj = value as Record<string, unknown>;
  const event = obj.event;
  if (event !== "started" && event !== "progress" && event !== "finished") return null;
  const data = obj.data;
  if (event === "started") {
    const startedData =
      data && typeof data === "object"
        ? { contentLength: asOptionalNumber((data as Record<string, unknown>).contentLength) }
        : undefined;
    return { event, data: startedData };
  }
  if (event === "progress") {
    const progressData =
      data && typeof data === "object"
        ? { chunkLength: asOptionalNumber((data as Record<string, unknown>).chunkLength) }
        : undefined;
    return { event, data: progressData };
  }
  return { event, data };
}

export async function updaterDownloadAndInstall(options: {
  rid: number;
  onEvent?: (event: UpdaterDownloadEvent) => void;
  timeoutMs?: number;
}): Promise<boolean | null> {
  if (!hasTauriRuntime()) return null;

  const { invoke, Channel } = await import("@tauri-apps/api/core");
  const onEvent = typeof options.onEvent === "function" ? options.onEvent : undefined;

  const channel = new Channel<unknown>((message) => {
    const evt = parseUpdaterDownloadEvent(message);
    if (!evt) return;
    onEvent?.(evt);
  });

  const args: Record<string, unknown> = {
    rid: options.rid,
    onEvent: channel,
  };
  if (typeof options.timeoutMs === "number" && Number.isFinite(options.timeoutMs)) {
    args.timeout = options.timeoutMs;
  }

  await invoke("plugin:updater|download_and_install", args);
  return true;
}
