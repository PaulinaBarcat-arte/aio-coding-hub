import type { AppAboutInfo } from "../../services/appAbout";
import type { DbDiskUsage } from "../../services/dataManagement";
import { Button } from "../../ui/Button";
import { Card } from "../../ui/Card";
import { SettingsRow } from "../../ui/SettingsRow";
import { formatBytes } from "../../utils/formatters";

type AvailableStatus = "checking" | "available" | "unavailable";

export function SettingsDataManagementCard({
  about,
  dbDiskUsageAvailable,
  dbDiskUsage,
  refreshDbDiskUsage,
  openAppDataDir,
  openClearRequestLogsDialog,
  openResetAllDialog,
}: {
  about: AppAboutInfo | null;
  dbDiskUsageAvailable: AvailableStatus;
  dbDiskUsage: DbDiskUsage | null;
  refreshDbDiskUsage: () => Promise<void>;
  openAppDataDir: () => Promise<void>;
  openClearRequestLogsDialog: () => void;
  openResetAllDialog: () => void;
}) {
  return (
    <Card>
      <div className="mb-4 flex items-center justify-between gap-2">
        <div className="font-semibold text-slate-900">数据管理</div>
        <Button
          onClick={() => void openAppDataDir()}
          variant="secondary"
          size="sm"
          disabled={!about}
        >
          打开目录
        </Button>
      </div>
      <div className="divide-y divide-slate-100">
        <SettingsRow label="数据磁盘占用">
          <span className="font-mono text-sm text-slate-900">
            {dbDiskUsageAvailable === "checking"
              ? "加载中…"
              : dbDiskUsageAvailable === "unavailable"
                ? "—"
                : formatBytes(dbDiskUsage?.total_bytes ?? 0)}
          </span>
          <Button
            onClick={() => refreshDbDiskUsage().catch(() => {})}
            variant="secondary"
            size="sm"
            disabled={!about || dbDiskUsageAvailable === "checking"}
          >
            刷新
          </Button>
        </SettingsRow>
        <SettingsRow label="清理请求日志">
          <span className="text-xs text-slate-500">不可撤销</span>
          <Button
            onClick={openClearRequestLogsDialog}
            variant="warning"
            size="sm"
            disabled={!about}
          >
            清理
          </Button>
        </SettingsRow>
        <SettingsRow label="清理全部信息">
          <span className="text-xs text-rose-700">不可撤销</span>
          <Button onClick={openResetAllDialog} variant="danger" size="sm" disabled={!about}>
            清理
          </Button>
        </SettingsRow>
      </div>
    </Card>
  );
}
