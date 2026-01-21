import type { AppAboutInfo } from "../../services/appAbout";
import type { ModelPricesSyncReport } from "../../services/modelPrices";
import { Button } from "../../ui/Button";
import { Card } from "../../ui/Card";
import { SettingsRow } from "../../ui/SettingsRow";

type AvailableStatus = "checking" | "available" | "unavailable";

export function SettingsDataSyncCard({
  about,
  modelPricesAvailable,
  modelPricesCount,
  lastModelPricesSyncError,
  lastModelPricesSyncReport,
  openModelPriceAliasesDialog,
  todayRequestsAvailable,
  todayRequestsTotal,
  syncingModelPrices,
  syncModelPrices,
}: {
  about: AppAboutInfo | null;
  modelPricesAvailable: AvailableStatus;
  modelPricesCount: number | null;
  lastModelPricesSyncError: string | null;
  lastModelPricesSyncReport: ModelPricesSyncReport | null;
  openModelPriceAliasesDialog: () => void;
  todayRequestsAvailable: AvailableStatus;
  todayRequestsTotal: number | null;
  syncingModelPrices: boolean;
  syncModelPrices: (force: boolean) => Promise<void>;
}) {
  return (
    <Card>
      <div className="mb-4 font-semibold text-slate-900">数据与同步</div>
      <div className="divide-y divide-slate-100">
        <SettingsRow label="模型定价">
          <span className="font-mono text-sm text-slate-900">
            {modelPricesAvailable === "checking"
              ? "加载中…"
              : modelPricesAvailable === "unavailable"
                ? "—"
                : modelPricesCount === 0
                  ? "未同步"
                  : `${modelPricesCount} 条`}
          </span>
          {lastModelPricesSyncError ? (
            <span className="text-xs text-rose-600">失败</span>
          ) : lastModelPricesSyncReport ? (
            <span className="text-xs text-slate-500">
              {lastModelPricesSyncReport.status === "not_modified"
                ? "最新"
                : `+${lastModelPricesSyncReport.inserted} / ~${lastModelPricesSyncReport.updated}`}
            </span>
          ) : null}
        </SettingsRow>
        <SettingsRow label="定价匹配">
          <span className="text-xs text-slate-500">prefix / wildcard / exact</span>
          <Button
            onClick={openModelPriceAliasesDialog}
            variant="secondary"
            size="sm"
            disabled={!about}
          >
            配置
          </Button>
        </SettingsRow>
        <SettingsRow label="今日请求">
          <span className="font-mono text-sm text-slate-900">
            {todayRequestsAvailable === "checking"
              ? "加载中…"
              : todayRequestsAvailable === "unavailable"
                ? "—"
                : String(todayRequestsTotal ?? 0)}
          </span>
        </SettingsRow>
        <SettingsRow label="同步定价">
          <div className="flex gap-2">
            <Button
              onClick={() => syncModelPrices(false)}
              variant="secondary"
              size="sm"
              disabled={syncingModelPrices}
            >
              {syncingModelPrices ? "同步中" : "同步"}
            </Button>
            <Button
              onClick={() => syncModelPrices(true)}
              variant="secondary"
              size="sm"
              disabled={syncingModelPrices}
            >
              强制
            </Button>
          </div>
        </SettingsRow>
      </div>
    </Card>
  );
}
