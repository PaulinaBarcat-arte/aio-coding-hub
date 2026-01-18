import { useSyncExternalStore } from "react";
import { toast } from "sonner";
import { logToConsole } from "../services/consoleLog";
import { appAboutGet, type AppAboutInfo } from "../services/appAbout";
import {
  updaterCheck,
  updaterDownloadAndInstall,
  type UpdaterCheckUpdate,
  type UpdaterDownloadEvent,
} from "../services/updater";
import { hasTauriRuntime } from "../services/tauriInvoke";

const STORAGE_KEY_LAST_CHECKED_AT_MS = "updater.lastCheckedAtMs";
const AUTO_CHECK_DELAY_MS = 2000;
const AUTO_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
const AUTO_CHECK_TICK_MS = 60 * 60 * 1000;

export type UpdateMeta = {
  about: AppAboutInfo | null;
  updateCandidate: UpdaterCheckUpdate | null;
  checkingUpdate: boolean;
  dialogOpen: boolean;

  installingUpdate: boolean;
  installError: string | null;
  installTotalBytes: number | null;
  installDownloadedBytes: number;
};

type Listener = () => void;

let snapshot: UpdateMeta = {
  about: null,
  updateCandidate: null,
  checkingUpdate: false,
  dialogOpen: false,

  installingUpdate: false,
  installError: null,
  installTotalBytes: null,
  installDownloadedBytes: 0,
};

const listeners = new Set<Listener>();

let started = false;
let starting: Promise<void> | null = null;
let autoCheckScheduled = false;
let sessionChecked = false;
let lastCheckError: string | null = null;
let checkingPromise: Promise<UpdaterCheckUpdate | null> | null = null;
let installingPromise: Promise<boolean | null> | null = null;

function emit() {
  for (const listener of listeners) listener();
}

function setSnapshot(patch: Partial<UpdateMeta>) {
  snapshot = { ...snapshot, ...patch };
  emit();
}

function readLastCheckedAtMs() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_LAST_CHECKED_AT_MS);
    if (!raw) return null;
    const v = Number(raw);
    return Number.isFinite(v) ? v : null;
  } catch {
    return null;
  }
}

function writeLastCheckedAtMs(ms: number) {
  try {
    localStorage.setItem(STORAGE_KEY_LAST_CHECKED_AT_MS, String(ms));
  } catch {}
}

async function ensureStarted() {
  if (started) return;
  if (starting) return starting;

  starting = (async () => {
    if (!hasTauriRuntime()) {
      started = true;
      starting = null;
      return;
    }

    try {
      const about = await appAboutGet();
      setSnapshot({ about });
    } catch {
      setSnapshot({ about: null });
    }

    scheduleAutoCheck();

    started = true;
    starting = null;
  })();

  return starting;
}

async function autoCheckIfDue() {
  const last = readLastCheckedAtMs();
  const now = Date.now();
  if (last != null && now - last < AUTO_CHECK_INTERVAL_MS) return;
  await updateCheckNow({ silent: true, openDialogIfUpdate: false });
}

async function autoCheckOnStartup() {
  if (sessionChecked) {
    logToConsole("info", "初始化：跳过自动检查更新", { reason: "already_checked_this_session" });
    return;
  }

  logToConsole("info", "初始化：自动检查更新", { delay_ms: AUTO_CHECK_DELAY_MS });

  const update = await updateCheckNow({ silent: true, openDialogIfUpdate: false });

  if (lastCheckError) {
    logToConsole("warn", "初始化：自动检查更新失败", { error: lastCheckError });
    return;
  }

  if (update) {
    logToConsole("info", "初始化：发现新版本", {
      version: update.version,
      current_version: update.currentVersion,
      date: update.date,
      rid: update.rid,
    });
    return;
  }

  logToConsole("info", "初始化：已是最新版本");
}

function scheduleAutoCheck() {
  if (autoCheckScheduled) return;
  autoCheckScheduled = true;

  window.setTimeout(() => {
    autoCheckOnStartup().catch(() => {});
  }, AUTO_CHECK_DELAY_MS);

  window.setInterval(() => {
    autoCheckIfDue().catch(() => {});
  }, AUTO_CHECK_TICK_MS);
}

export async function updateCheckNow(options: {
  silent: boolean;
  openDialogIfUpdate: boolean;
}): Promise<UpdaterCheckUpdate | null> {
  await ensureStarted();

  sessionChecked = true;

  if (snapshot.checkingUpdate) {
    return checkingPromise ?? snapshot.updateCandidate;
  }

  checkingPromise = (async () => {
    lastCheckError = null;
    setSnapshot({ checkingUpdate: true });
    try {
      const update = await updaterCheck();
      writeLastCheckedAtMs(Date.now());

      // Keep existing updateCandidate when check fails; but if check succeeds with no update, clear it.
      setSnapshot({ updateCandidate: update });

      if (update && options.openDialogIfUpdate) {
        setSnapshot({
          dialogOpen: true,
          installError: null,
          installDownloadedBytes: 0,
          installTotalBytes: null,
          installingUpdate: false,
        });
      }

      if (!update && !options.silent) {
        toast("已是最新版本");
      }

      return update;
    } catch (err) {
      const message = String(err);
      lastCheckError = message;
      logToConsole("error", "检查更新失败", { error: message });
      writeLastCheckedAtMs(Date.now());
      if (!options.silent) toast(`检查更新失败：${message}`);
      return null;
    } finally {
      setSnapshot({ checkingUpdate: false });
      checkingPromise = null;
    }
  })();

  return checkingPromise;
}

function onUpdaterDownloadEvent(evt: UpdaterDownloadEvent) {
  if (evt.event === "started") {
    const total = evt.data?.contentLength;
    setSnapshot({ installTotalBytes: typeof total === "number" ? total : null });
    return;
  }
  if (evt.event === "progress") {
    const chunk = evt.data?.chunkLength;
    if (typeof chunk === "number" && Number.isFinite(chunk) && chunk > 0) {
      setSnapshot({ installDownloadedBytes: snapshot.installDownloadedBytes + chunk });
    }
  }
}

export async function updateDownloadAndInstall(): Promise<boolean | null> {
  await ensureStarted();

  if (!snapshot.updateCandidate) return null;
  if (snapshot.installingUpdate) return installingPromise ?? true;

  setSnapshot({
    installError: null,
    installDownloadedBytes: 0,
    installTotalBytes: null,
    installingUpdate: true,
  });

  installingPromise = (async () => {
    try {
      const ok = await updaterDownloadAndInstall({
        rid: snapshot.updateCandidate!.rid,
        onEvent: onUpdaterDownloadEvent,
      });
      return ok;
    } catch (err) {
      const message = String(err);
      setSnapshot({ installError: message });
      logToConsole("error", "安装更新失败", { error: message });
      toast("安装更新失败：请稍后重试");
      return false;
    } finally {
      setSnapshot({ installingUpdate: false });
      installingPromise = null;
    }
  })();

  return installingPromise;
}

export function updateDialogSetOpen(open: boolean) {
  if (!open && snapshot.installingUpdate) return;

  setSnapshot({ dialogOpen: open });
  if (!open) {
    setSnapshot({
      installError: null,
      installDownloadedBytes: 0,
      installTotalBytes: null,
      installingUpdate: false,
    });
  }
}

export function useUpdateMeta(): UpdateMeta {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      void ensureStarted();
      return () => listeners.delete(listener);
    },
    () => snapshot,
    () => snapshot
  );
}
