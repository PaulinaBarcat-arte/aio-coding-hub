import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import type { UpdateMeta } from "../../hooks/useUpdateMeta";
import { updateCheckNow } from "../../hooks/useUpdateMeta";
import { AIO_RELEASES_URL } from "../../constants/urls";
import { logToConsole } from "../../services/consoleLog";
import {
  modelPricesList,
  modelPricesSyncBasellm,
  subscribeModelPricesUpdated,
  type ModelPricesSyncReport,
} from "../../services/modelPrices";
import { usageSummary } from "../../services/usage";
import {
  appDataDirGet,
  appDataReset,
  appExit,
  dbDiskUsageGet,
  requestLogsClearAll,
  type DbDiskUsage,
} from "../../services/dataManagement";
import { SettingsAboutCard } from "./SettingsAboutCard";
import { SettingsDataManagementCard } from "./SettingsDataManagementCard";
import { SettingsDataSyncCard } from "./SettingsDataSyncCard";
import { SettingsDialogs } from "./SettingsDialogs";
import { SettingsUpdateCard } from "./SettingsUpdateCard";

type AvailableStatus = "checking" | "available" | "unavailable";

export type SettingsSidebarProps = {
  updateMeta: UpdateMeta;
};

export function SettingsSidebar({ updateMeta }: SettingsSidebarProps) {
  const about = updateMeta.about;

  const [syncingModelPrices, setSyncingModelPrices] = useState(false);
  const [lastModelPricesSyncReport, setLastModelPricesSyncReport] =
    useState<ModelPricesSyncReport | null>(null);
  const [lastModelPricesSyncError, setLastModelPricesSyncError] = useState<string | null>(null);
  const [modelPriceAliasesDialogOpen, setModelPriceAliasesDialogOpen] = useState(false);
  const [modelPricesAvailable, setModelPricesAvailable] = useState<AvailableStatus>("checking");
  const [modelPricesCount, setModelPricesCount] = useState<number | null>(null);

  const [todayRequestsAvailable, setTodayRequestsAvailable] = useState<AvailableStatus>("checking");
  const [todayRequestsTotal, setTodayRequestsTotal] = useState<number | null>(null);

  const [dbDiskUsageAvailable, setDbDiskUsageAvailable] = useState<AvailableStatus>("checking");
  const [dbDiskUsage, setDbDiskUsage] = useState<DbDiskUsage | null>(null);

  const [clearRequestLogsDialogOpen, setClearRequestLogsDialogOpen] = useState(false);
  const [clearingRequestLogs, setClearingRequestLogs] = useState(false);
  const [resetAllDialogOpen, setResetAllDialogOpen] = useState(false);
  const [resettingAll, setResettingAll] = useState(false);

  async function openUpdateLog() {
    const url = AIO_RELEASES_URL;

    try {
      await openUrl(url);
    } catch (err) {
      logToConsole("error", "打开更新日志失败", { error: String(err), url });
      toast("打开更新日志失败");
    }
  }

  async function checkUpdate() {
    try {
      if (!about) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }

      if (about.run_mode === "portable") {
        toast("portable 模式请手动下载");
        await openUpdateLog();
        return;
      }

      await updateCheckNow({ silent: false, openDialogIfUpdate: true });
    } catch {
      // noop: errors/toasts are handled in updateCheckNow
    }
  }

  async function openAppDataDir() {
    try {
      const dir = await appDataDirGet();
      if (!dir) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }
      await openPath(dir);
    } catch (err) {
      logToConsole("error", "打开数据目录失败", { error: String(err) });
      toast("打开数据目录失败：请查看控制台日志");
    }
  }

  const refreshModelPricesCount = useCallback(async () => {
    setModelPricesAvailable("checking");
    try {
      const [codex, claude, gemini] = await Promise.all([
        modelPricesList("codex"),
        modelPricesList("claude"),
        modelPricesList("gemini"),
      ]);

      if (!codex || !claude || !gemini) {
        setModelPricesAvailable("unavailable");
        setModelPricesCount(null);
        return;
      }

      setModelPricesAvailable("available");
      setModelPricesCount(codex.length + claude.length + gemini.length);
    } catch {
      setModelPricesAvailable("unavailable");
      setModelPricesCount(null);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    setTodayRequestsAvailable("checking");
    usageSummary("today")
      .then((summary) => {
        if (cancelled) return;
        if (!summary) {
          setTodayRequestsAvailable("unavailable");
          setTodayRequestsTotal(null);
          return;
        }
        setTodayRequestsAvailable("available");
        setTodayRequestsTotal(summary.requests_total);
      })
      .catch(() => {
        if (cancelled) return;
        setTodayRequestsAvailable("unavailable");
        setTodayRequestsTotal(null);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    refreshModelPricesCount().catch(() => {});
  }, [refreshModelPricesCount]);

  const refreshDbDiskUsage = useCallback(async () => {
    setDbDiskUsageAvailable("checking");
    try {
      const usage = await dbDiskUsageGet();
      if (!usage) {
        setDbDiskUsageAvailable("unavailable");
        setDbDiskUsage(null);
        return;
      }
      setDbDiskUsageAvailable("available");
      setDbDiskUsage(usage);
    } catch {
      setDbDiskUsageAvailable("unavailable");
      setDbDiskUsage(null);
    }
  }, []);

  useEffect(() => {
    refreshDbDiskUsage().catch(() => {});
  }, [refreshDbDiskUsage]);

  async function clearRequestLogs() {
    if (clearingRequestLogs) return;
    setClearingRequestLogs(true);

    try {
      const result = await requestLogsClearAll();
      if (!result) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }

      toast(
        `已清理请求日志：request_logs ${result.request_logs_deleted} 条，request_attempt_logs ${result.request_attempt_logs_deleted} 条`
      );
      logToConsole("info", "清理请求日志", result);
      setClearRequestLogsDialogOpen(false);
      refreshDbDiskUsage().catch(() => {});
    } catch (err) {
      logToConsole("error", "清理请求日志失败", { error: String(err) });
      toast("清理请求日志失败：请稍后重试");
    } finally {
      setClearingRequestLogs(false);
    }
  }

  async function resetAllData() {
    if (resettingAll) return;
    setResettingAll(true);

    try {
      const ok = await appDataReset();
      if (!ok) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }

      logToConsole("info", "清理全部信息", { ok: true });
      toast("已清理全部信息：应用即将退出，请重新打开");
      setResetAllDialogOpen(false);

      window.setTimeout(() => {
        appExit().catch(() => {});
      }, 1000);
    } catch (err) {
      logToConsole("error", "清理全部信息失败", { error: String(err) });
      toast("清理全部信息失败：请稍后重试");
    } finally {
      setResettingAll(false);
    }
  }

  useEffect(() => {
    return subscribeModelPricesUpdated(() => {
      refreshModelPricesCount().catch(() => {});
    });
  }, [refreshModelPricesCount]);

  async function syncModelPrices(force: boolean) {
    if (syncingModelPrices) return;
    setSyncingModelPrices(true);
    setLastModelPricesSyncError(null);

    try {
      const report = await modelPricesSyncBasellm(force);
      if (!report) {
        toast("仅在 Tauri Desktop 环境可用");
        return;
      }

      setLastModelPricesSyncReport(report);
      if (report.status !== "not_modified") {
        await refreshModelPricesCount();
      }

      if (report.status === "not_modified") {
        toast("模型定价已是最新（无变更）");
        return;
      }

      toast(`同步完成：新增 ${report.inserted}，更新 ${report.updated}，跳过 ${report.skipped}`);
    } catch (err) {
      logToConsole("error", "同步模型定价失败", { error: String(err) });
      toast("同步模型定价失败：请稍后重试");
      setLastModelPricesSyncError(String(err));
    } finally {
      setSyncingModelPrices(false);
    }
  }

  return (
    <>
      <div className="space-y-6 lg:col-span-4">
        <SettingsAboutCard about={about} />

        <SettingsUpdateCard
          about={about}
          checkingUpdate={updateMeta.checkingUpdate}
          checkUpdate={checkUpdate}
        />

        <SettingsDataManagementCard
          about={about}
          dbDiskUsageAvailable={dbDiskUsageAvailable}
          dbDiskUsage={dbDiskUsage}
          refreshDbDiskUsage={refreshDbDiskUsage}
          openAppDataDir={openAppDataDir}
          openClearRequestLogsDialog={() => setClearRequestLogsDialogOpen(true)}
          openResetAllDialog={() => setResetAllDialogOpen(true)}
        />

        <SettingsDataSyncCard
          about={about}
          modelPricesAvailable={modelPricesAvailable}
          modelPricesCount={modelPricesCount}
          lastModelPricesSyncError={lastModelPricesSyncError}
          lastModelPricesSyncReport={lastModelPricesSyncReport}
          openModelPriceAliasesDialog={() => setModelPriceAliasesDialogOpen(true)}
          todayRequestsAvailable={todayRequestsAvailable}
          todayRequestsTotal={todayRequestsTotal}
          syncingModelPrices={syncingModelPrices}
          syncModelPrices={syncModelPrices}
        />
      </div>

      <SettingsDialogs
        modelPriceAliasesDialogOpen={modelPriceAliasesDialogOpen}
        setModelPriceAliasesDialogOpen={setModelPriceAliasesDialogOpen}
        clearRequestLogsDialogOpen={clearRequestLogsDialogOpen}
        setClearRequestLogsDialogOpen={setClearRequestLogsDialogOpen}
        clearingRequestLogs={clearingRequestLogs}
        setClearingRequestLogs={setClearingRequestLogs}
        clearRequestLogs={clearRequestLogs}
        resetAllDialogOpen={resetAllDialogOpen}
        setResetAllDialogOpen={setResetAllDialogOpen}
        resettingAll={resettingAll}
        setResettingAll={setResettingAll}
        resetAllData={resetAllData}
      />
    </>
  );
}
