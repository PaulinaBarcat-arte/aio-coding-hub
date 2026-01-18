// Usage: Main page for managing providers and sort modes (renders sub-views under `src/pages/providers/*`).

import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { logToConsole } from "../services/consoleLog";
import { providersList, type CliKey, type ProviderSummary } from "../services/providers";
import { Button } from "../ui/Button";
import { cn } from "../utils/cn";
import { ProvidersView } from "./providers/ProvidersView";
import { SortModesView } from "./providers/SortModesView";

export function ProvidersPage() {
  const [view, setView] = useState<"providers" | "sortModes">("providers");

  const [activeCli, setActiveCli] = useState<CliKey>("claude");
  const activeCliRef = useRef(activeCli);
  useEffect(() => {
    activeCliRef.current = activeCli;
  }, [activeCli]);

  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [providersLoading, setProvidersLoading] = useState(false);

  async function refreshProviders(cliKey: CliKey) {
    setProvidersLoading(true);
    try {
      const items = await providersList(cliKey);
      if (activeCliRef.current !== cliKey) return;
      if (!items) {
        setProviders([]);
        return;
      }
      setProviders(items);
    } catch (err) {
      if (activeCliRef.current !== cliKey) return;
      logToConsole("error", "读取供应商失败", {
        cli: cliKey,
        error: String(err),
      });
      toast("读取供应商失败：请查看控制台日志");
    } finally {
      if (activeCliRef.current === cliKey) {
        setProvidersLoading(false);
      }
    }
  }

  useEffect(() => {
    void refreshProviders(activeCli);
  }, [activeCli]);

  return (
    <div className="flex flex-col gap-5 lg:h-[calc(100vh-40px)] lg:overflow-hidden">
      <div className="flex items-end justify-between gap-3">
        <div className="min-w-0">
          <h1 className="text-2xl font-semibold tracking-tight">
            {view === "providers" ? "供应商" : "排序模板"}
          </h1>
        </div>

        <div className="flex items-center gap-2">
          <div
            role="tablist"
            aria-label="视图切换"
            className="flex items-center rounded-lg border border-slate-200 bg-white p-1"
          >
            <Button
              onClick={() => setView("providers")}
              variant={view === "providers" ? "primary" : "ghost"}
              size="sm"
              role="tab"
              aria-selected={view === "providers"}
              className="h-auto w-40 px-3 py-2 shadow-none"
            >
              <span className="flex flex-col items-start leading-tight">
                <span className="text-sm font-semibold">供应商</span>
                <span
                  className={cn(
                    "mt-0.5 text-[11px]",
                    view === "providers" ? "text-white/80" : "text-slate-500"
                  )}
                >
                  配置 Provider · 默认顺序
                </span>
              </span>
            </Button>

            <Button
              onClick={() => setView("sortModes")}
              variant={view === "sortModes" ? "primary" : "ghost"}
              size="sm"
              role="tab"
              aria-selected={view === "sortModes"}
              className="h-auto w-40 px-3 py-2 shadow-none"
            >
              <span className="flex flex-col items-start leading-tight">
                <span className="text-sm font-semibold">排序模板</span>
                <span
                  className={cn(
                    "mt-0.5 text-[11px]",
                    view === "sortModes" ? "text-white/80" : "text-slate-500"
                  )}
                >
                  公司/生活场景切换
                </span>
              </span>
            </Button>
          </div>
        </div>
      </div>

      {view === "providers" ? (
        <ProvidersView
          activeCli={activeCli}
          setActiveCli={setActiveCli}
          providers={providers}
          setProviders={setProviders}
          providersLoading={providersLoading}
          refreshProviders={refreshProviders}
        />
      ) : (
        <SortModesView
          activeCli={activeCli}
          setActiveCli={setActiveCli}
          providers={providers}
          providersLoading={providersLoading}
        />
      )}
    </div>
  );
}
