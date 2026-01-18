import { NavLink } from "react-router-dom";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AIO_RELEASES_URL, AIO_REPO_URL } from "../constants/urls";
import { useGatewayMeta } from "../hooks/useGatewayMeta";
import { updateDialogSetOpen, useUpdateMeta } from "../hooks/useUpdateMeta";
import { cn } from "../utils/cn";

type NavItem = {
  to: string;
  label: string;
};

const NAV: NavItem[] = [
  { to: "/", label: "首页" },
  { to: "/providers", label: "供应商" },
  { to: "/prompts", label: "提示词" },
  { to: "/mcp", label: "MCP" },
  { to: "/skills", label: "Skill" },
  { to: "/usage", label: "用量" },
  { to: "/console", label: "控制台" },
  { to: "/cli-manager", label: "CLI 管理" },
  { to: "/settings", label: "设置" },
];

export function Sidebar() {
  const { gatewayAvailable, gateway, preferredPort } = useGatewayMeta();
  const updateMeta = useUpdateMeta();
  const hasUpdate = !!updateMeta.updateCandidate;
  const isPortable = updateMeta.about?.run_mode === "portable";

  const statusText =
    gatewayAvailable === "checking"
      ? "检查中"
      : gatewayAvailable === "unavailable"
        ? "不可用"
        : gateway == null
          ? "未知"
          : gateway.running
            ? "运行中"
            : "已停止";

  const statusTone =
    gatewayAvailable === "available" && gateway?.running
      ? "bg-emerald-50 text-emerald-700"
      : "bg-slate-100 text-slate-600";

  const portText = gatewayAvailable === "available" ? String(gateway?.port ?? preferredPort) : "—";

  async function openReleases() {
    try {
      await openUrl(AIO_RELEASES_URL);
    } catch {
      try {
        window.open(AIO_RELEASES_URL, "_blank", "noopener,noreferrer");
      } catch {}
    }
  }

  return (
    <aside className="sticky top-0 h-screen w-64 shrink-0 border-r border-slate-200 bg-white/70 backdrop-blur">
      <div className="flex h-full flex-col">
        <div className="px-4 py-5">
          <div className="flex items-center justify-between">
            <div className="text-sm font-semibold">AIO Coding Hub</div>
            {hasUpdate ? (
              <button
                type="button"
                className={cn(
                  "flex items-center gap-1 rounded-lg px-2 py-1 transition",
                  "bg-emerald-50 text-emerald-700 ring-1 ring-emerald-200 hover:bg-emerald-100"
                )}
                title={isPortable ? "发现新版本（portable：打开下载页）" : "发现新版本（点击更新）"}
                onClick={() => {
                  if (isPortable) {
                    openReleases().catch(() => {});
                    return;
                  }
                  updateDialogSetOpen(true);
                }}
              >
                <svg className="h-5 w-5" fill="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
                </svg>
                <span className="text-[10px] font-bold leading-none tracking-wide">NEW</span>
              </button>
            ) : (
              <a
                href={AIO_REPO_URL}
                target="_blank"
                rel="noopener noreferrer"
                className="text-slate-500 transition hover:text-slate-900"
              >
                <svg className="h-6 w-6" fill="currentColor" viewBox="0 0 24 24">
                  <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
                </svg>
              </a>
            )}
          </div>
        </div>

        <nav className="flex-1 space-y-1 px-3">
          {NAV.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                cn(
                  "group flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition",
                  isActive
                    ? "bg-slate-900 text-white shadow-sm"
                    : "text-slate-700 hover:bg-slate-100"
                )
              }
              end={item.to === "/"}
            >
              {({ isActive }) => (
                <>
                  <span
                    className={cn(
                      "h-1.5 w-1.5 rounded-full bg-current transition-opacity",
                      isActive ? "opacity-100" : "opacity-40 group-hover:opacity-60"
                    )}
                  />
                  <span className="truncate">{item.label}</span>
                </>
              )}
            </NavLink>
          ))}
        </nav>

        <div className="border-t border-slate-200 px-4 py-4 text-xs text-slate-500">
          <div className="grid grid-cols-[1fr_auto] items-center gap-3">
            <span>网关</span>
            <div className="flex w-24 justify-center">
              <span className={cn("rounded-full px-2 py-1 font-medium", statusTone)}>
                {statusText}
              </span>
            </div>
          </div>
          <div className="mt-2 grid grid-cols-[1fr_auto] items-center gap-3">
            <span>端口</span>
            <div className="flex w-24 justify-center">
              <span className="font-mono text-slate-700">{portText}</span>
            </div>
          </div>
        </div>
      </div>
    </aside>
  );
}
