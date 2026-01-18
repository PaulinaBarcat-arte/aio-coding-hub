import { invokeTauriOrNull } from "./tauriInvoke";

export type DbDiskUsage = {
  db_bytes: number;
  wal_bytes: number;
  shm_bytes: number;
  total_bytes: number;
};

export type ClearRequestLogsResult = {
  request_logs_deleted: number;
  request_attempt_logs_deleted: number;
};

export async function dbDiskUsageGet() {
  return invokeTauriOrNull<DbDiskUsage>("db_disk_usage_get");
}

export async function requestLogsClearAll() {
  return invokeTauriOrNull<ClearRequestLogsResult>("request_logs_clear_all");
}

export async function appDataReset() {
  return invokeTauriOrNull<boolean>("app_data_reset");
}

export async function appDataDirGet() {
  return invokeTauriOrNull<string>("app_data_dir_get");
}

export async function appExit() {
  return invokeTauriOrNull<boolean>("app_exit");
}

export async function appRestart() {
  return invokeTauriOrNull<boolean>("app_restart");
}
