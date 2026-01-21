import type { AppAboutInfo } from "../../services/appAbout";
import { Card } from "../../ui/Card";

export function SettingsAboutCard({ about }: { about: AppAboutInfo | null }) {
  return (
    <Card>
      <div className="mb-4 font-semibold text-slate-900">关于应用</div>
      {about ? (
        <div className="grid gap-2 text-sm text-slate-700">
          <div className="flex items-center justify-between gap-4">
            <span className="text-slate-500">版本</span>
            <span className="font-mono">{about.app_version}</span>
          </div>
          <div className="flex items-center justify-between gap-4">
            <span className="text-slate-500">构建</span>
            <span className="font-mono">{about.profile}</span>
          </div>
          <div className="flex items-center justify-between gap-4">
            <span className="text-slate-500">平台</span>
            <span className="font-mono">
              {about.os}/{about.arch}
            </span>
          </div>
          <div className="flex items-center justify-between gap-4">
            <span className="text-slate-500">Bundle</span>
            <span className="font-mono">{about.bundle_type ?? "—"}</span>
          </div>
          <div className="flex items-center justify-between gap-4">
            <span className="text-slate-500">运行模式</span>
            <span className="font-mono">{about.run_mode}</span>
          </div>
        </div>
      ) : (
        <div className="text-sm text-slate-600">仅在 Tauri Desktop 环境可用。</div>
      )}
    </Card>
  );
}
