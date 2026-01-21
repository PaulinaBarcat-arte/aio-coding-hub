import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { ModelPriceAliasesDialog } from "../../components/settings/ModelPriceAliasesDialog";

export function SettingsDialogs({
  modelPriceAliasesDialogOpen,
  setModelPriceAliasesDialogOpen,

  clearRequestLogsDialogOpen,
  setClearRequestLogsDialogOpen,
  clearingRequestLogs,
  setClearingRequestLogs,
  clearRequestLogs,

  resetAllDialogOpen,
  setResetAllDialogOpen,
  resettingAll,
  setResettingAll,
  resetAllData,
}: {
  modelPriceAliasesDialogOpen: boolean;
  setModelPriceAliasesDialogOpen: (open: boolean) => void;

  clearRequestLogsDialogOpen: boolean;
  setClearRequestLogsDialogOpen: (open: boolean) => void;
  clearingRequestLogs: boolean;
  setClearingRequestLogs: (next: boolean) => void;
  clearRequestLogs: () => Promise<void>;

  resetAllDialogOpen: boolean;
  setResetAllDialogOpen: (open: boolean) => void;
  resettingAll: boolean;
  setResettingAll: (next: boolean) => void;
  resetAllData: () => Promise<void>;
}) {
  return (
    <>
      <ModelPriceAliasesDialog
        open={modelPriceAliasesDialogOpen}
        onOpenChange={setModelPriceAliasesDialogOpen}
      />

      <Dialog
        open={clearRequestLogsDialogOpen}
        onOpenChange={(open) => {
          if (!open && clearingRequestLogs) return;
          setClearRequestLogsDialogOpen(open);
          if (!open) setClearingRequestLogs(false);
        }}
        title="确认清理请求日志"
        description="将清空 request_logs 与 request_attempt_logs。此操作不可撤销。"
        className="max-w-lg"
      >
        <div className="space-y-4">
          <div className="text-sm text-slate-700">
            说明：仅影响请求日志与明细，不会影响 Providers、Prompts、MCP 等配置。
          </div>
          <div className="flex flex-wrap items-center justify-end gap-2 border-t border-slate-100 pt-3">
            <Button
              onClick={() => setClearRequestLogsDialogOpen(false)}
              variant="secondary"
              disabled={clearingRequestLogs}
            >
              取消
            </Button>
            <Button
              onClick={() => void clearRequestLogs()}
              variant="warning"
              disabled={clearingRequestLogs}
            >
              {clearingRequestLogs ? "清理中…" : "确认清理"}
            </Button>
          </div>
        </div>
      </Dialog>

      <Dialog
        open={resetAllDialogOpen}
        onOpenChange={(open) => {
          if (!open && resettingAll) return;
          setResetAllDialogOpen(open);
          if (!open) setResettingAll(false);
        }}
        title="确认清理全部信息"
        description="将删除本地数据库与 settings.json，并在完成后退出应用。下次启动会以默认配置重新初始化。此操作不可撤销。"
        className="max-w-lg"
      >
        <div className="space-y-4">
          <div className="rounded-lg border border-rose-200 bg-rose-50 p-3 text-sm text-rose-800">
            注意：此操作会清空所有本地数据与配置。完成后应用会自动退出，需要手动重新打开。
          </div>
          <div className="flex flex-wrap items-center justify-end gap-2 border-t border-slate-100 pt-3">
            <Button
              onClick={() => setResetAllDialogOpen(false)}
              variant="secondary"
              disabled={resettingAll}
            >
              取消
            </Button>
            <Button onClick={() => void resetAllData()} variant="danger" disabled={resettingAll}>
              {resettingAll ? "清理中…" : "确认清理并退出"}
            </Button>
          </div>
        </div>
      </Dialog>
    </>
  );
}
